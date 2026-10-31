//! The window's side of `status`, `push` and `pull`, on the library's
//! steps: the server's position for each book, sending the device's
//! positions, and writing the server's into a Tolino. A step that needs an
//! answer (`confirm`) comes back as a [`Question`] carrying what to send or
//! write, so the window asks and nothing is decided twice.

use std::collections::HashMap;
use std::path::PathBuf;

use nickelinksync::db::Write;
use nickelinksync::history::{self, Entry};
use nickelinksync::kosync::{self, Client, Progress, Update};
use nickelinksync::session::{
  self, Action, KindleSession, Options, PullOutcome, PushOutcome, Server, Session, Written,
};
use nickelinksync::sync::{format_timestamp, outlook, Confirm, Local, Outlook};

use crate::devices::Kind;

/// A position stored on the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Remote {
  pub percent: f64,
  /// The device that stored it.
  pub device: String,
  pub time: String,
  pub xpointer: String,
}

impl From<&Progress> for Remote {
  fn from(p: &Progress) -> Self {
    Remote {
      percent: p.percentage,
      device: p.device.clone(),
      time: format_timestamp(p.timestamp),
      xpointer: p.progress.clone(),
    }
  }
}

/// One book against the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Compared {
  /// `None`: nothing stored.
  pub remote: Option<Remote>,
  /// `None` if the device position could not be translated.
  pub outlook: Option<Outlook>,
}

/// The device's books against the server, by row id.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
  /// No configuration with a server (`config.ini`).
  NotConfigured,
  Failed(String),
  Compared(HashMap<String, Compared>),
}

/// What a yes to a [`Question`] does.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
  /// Send the device's position.
  Send(Update),
  /// Write the server's position into the device, and note it so.
  Write { write: Write, entry: Entry },
}

/// A book whose push or pull needs an answer, with what would be sent or
/// written.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
  pub title: String,
  pub local_percent: f64,
  pub local_xpointer: String,
  pub local_time: Option<String>,
  pub remote: Remote,
  /// The position would move back in the book: for a push the device's
  /// lies before the server's, for a pull after it.
  pub backwards: bool,
  pub answer: Answer,
}

impl Question {
  pub fn pulls(&self) -> bool {
    matches!(self.answer, Answer::Write { .. })
  }
}

/// What happened to one book.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
  Sent,
  /// The server's position was written into the device.
  Written,
  InSync,
  /// The server's position is newer: kept.
  ServerNewer,
  /// The device's position is newer: not taken.
  DeviceNewer,
  /// The server holds no position for the book.
  NothingStored,
  /// The plugin could not bring the book in line on the device.
  NotInLine,
  Skipped(String),
  Failed(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
  /// Title and outcome, in the order of the books.
  pub books: Vec<(String, Outcome)>,
  pub questions: Vec<Question>,
  /// The copy of the device database made before writing to it.
  pub backup: Option<PathBuf>,
  /// Nickel's cached pages dropped so it opens at the written place; then
  /// the device must be restarted, not only ejected.
  pub pages_dropped: usize,
}

/// What [`Server::open`] gives, as the window needs it.
fn server(options: &Options) -> Result<Option<Server>, String> {
  Server::open(options).map_err(|e| e.to_string())
}

fn compared(local: Option<&Local>, remote: Option<&Progress>, confirm: Confirm) -> Compared {
  Compared {
    remote: remote.map(Remote::from),
    outlook: local.map(|l| outlook(l, remote, confirm)),
  }
}

/// The server's position for each book on the device in `options`.
pub fn compare(options: &Options, kind: Kind) -> ServerState {
  match compare_books(options, kind) {
    Ok(Some(found)) => ServerState::Compared(found),
    Ok(None) => ServerState::NotConfigured,
    Err(e) => ServerState::Failed(e),
  }
}

fn compare_books(
  options: &Options,
  kind: Kind,
) -> Result<Option<HashMap<String, Compared>>, String> {
  let Some(server) = server(options)? else {
    return Ok(None);
  };
  let confirm = server.config.confirm;
  let ask = |document: &str| server.client.progress(document).map_err(|e| e.to_string());
  let mut found = HashMap::new();
  match kind {
    Kind::Nickel => {
      let session = Session::open(options).map_err(|e| e.to_string())?;
      for book in &session.books {
        let identity = session.identity(book);
        let Some(document) = identity.document() else {
          continue;
        };
        let remote = ask(document)?;
        let local = session
          .local(book, &identity)
          .ok()
          .map(|(l, _)| session.against(book, &identity, l, remote.as_ref()));
        found.insert(
          book.content_id.clone(),
          compared(local.as_ref(), remote.as_ref(), confirm),
        );
      }
    }
    Kind::Kindle => {
      let session = KindleSession::open(options).map_err(|e| e.to_string())?;
      for book in &session.books {
        let Ok(mapping) = session.mapping(book) else {
          continue;
        };
        let remote = ask(&mapping.epub.hash)?;
        let local = session.local(book, mapping).ok().map(|(l, _, _)| l);
        found.insert(
          book.content_id.clone(),
          compared(local.as_ref(), remote.as_ref(), confirm),
        );
      }
    }
  }
  Ok(Some(found))
}

/// The server's position for each of `documents` (absent: nothing
/// stored); `None` without a server in the configuration.
pub fn remotes(
  options: &Options,
  documents: &[String],
) -> Result<Option<HashMap<String, Remote>>, String> {
  let Some(server) = server(options)? else {
    return Ok(None);
  };
  let mut found = HashMap::new();
  for document in documents {
    if let Some(p) = server
      .client
      .progress(document)
      .map_err(|e| e.to_string())?
    {
      found.insert(document.clone(), Remote::from(&p));
    }
  }
  Ok(Some(found))
}

/// One push step's outcome: sent at once, a question, or a report.
fn settle(
  client: &Client,
  title: String,
  document: &str,
  outcome: PushOutcome,
  (device, device_id): (&str, &str),
  report: &mut Report,
  sent: &mut Vec<Entry>,
) -> Result<(), kosync::Error> {
  let (local, remote, action) = match outcome {
    PushOutcome::Skip { message, .. } => {
      report.books.push((title, Outcome::Skipped(message)));
      return Ok(());
    }
    PushOutcome::NotInLine { .. } => {
      report.books.push((title, Outcome::NotInLine));
      return Ok(());
    }
    PushOutcome::Failed(e) => {
      report.books.push((title, Outcome::Failed(e.to_string())));
      return Err(e);
    }
    PushOutcome::Ready {
      local,
      remote,
      action,
      ..
    } => (local, remote, action),
  };
  let update = session::update(document, &local, device, device_id);
  match (action, remote) {
    (Action::InSync, _) => report.books.push((title, Outcome::InSync)),
    (Action::Keep, _) => report.books.push((title, Outcome::ServerNewer)),
    (Action::Ask { backwards }, Some(remote)) => report.questions.push(Question {
      title,
      local_percent: local.percentage,
      local_xpointer: local.xpointer.clone(),
      local_time: local.last_read.map(format_timestamp),
      remote: Remote::from(&remote),
      backwards,
      answer: Answer::Send(update),
    }),
    (Action::Apply | Action::Ask { .. }, _) => {
      deliver(client, title, update, report, sent)?;
    }
  }
  Ok(())
}

/// Sends one update and notes it for the report and the history.
fn deliver(
  client: &Client,
  title: String,
  update: Update,
  report: &mut Report,
  sent: &mut Vec<Entry>,
) -> Result<(), kosync::Error> {
  match client.update(&update) {
    Ok(time) => {
      sent.push(Entry {
        time: time.unwrap_or_else(history::now),
        direction: history::Direction::Push,
        device: update.device.clone(),
        title: Some(title.clone()),
        document: update.document.clone(),
        position: update.progress.clone(),
        percentage: update.percentage,
        by: "gui".into(),
      });
      report.books.push((title, Outcome::Sent));
      Ok(())
    }
    Err(e) => {
      report.books.push((title, Outcome::Failed(e.to_string())));
      Err(e)
    }
  }
}

/// Keeps the sent positions in the history; a failure only shows in the
/// report.
fn record(sent: &[Entry], report: &mut Report) {
  if let Err(e) = history::record(sent) {
    report
      .books
      .push(("history.jsonl".into(), Outcome::Failed(e.to_string())));
  }
}

/// Pushes the books with `ids` on the device in `options`: what needs no
/// answer is sent at once, the rest comes back as questions.
pub fn push(options: &Options, kind: Kind, ids: &[String]) -> Result<Report, String> {
  let server = server(options)?.ok_or("no server set up in config.ini")?;
  server.client.authorize().map_err(|e| e.to_string())?;
  let confirm = server.config.confirm;
  let mut report = Report::default();
  let mut sent = Vec::new();
  match kind {
    Kind::Nickel => {
      let session = Session::open(options).map_err(|e| e.to_string())?;
      let (device, device_id) = server.config.identity(&session.model);
      for book in session.books.iter().filter(|b| ids.contains(&b.content_id)) {
        let step = session.push_step(book, &server.client, confirm, false);
        let title = book
          .title
          .clone()
          .unwrap_or_else(|| book.content_id.clone());
        let document = step.identity.document().unwrap_or_default().to_string();
        let to = (device.as_str(), device_id.as_str());
        if let Err(kosync::Error::Unauthorized) = settle(
          &server.client,
          title,
          &document,
          step.outcome,
          to,
          &mut report,
          &mut sent,
        ) {
          break;
        }
      }
    }
    Kind::Kindle => {
      let session = KindleSession::open(options).map_err(|e| e.to_string())?;
      let (device, device_id) = server.config.identity("Kindle");
      for book in session.books.iter().filter(|b| ids.contains(&b.content_id)) {
        let step = session.push_step(book, &server.client, confirm, false);
        let title = book
          .title
          .clone()
          .unwrap_or_else(|| book.content_id.clone());
        let document = step.mapping.map(|m| m.epub.hash).unwrap_or_default();
        let to = (device.as_str(), device_id.as_str());
        if let Err(kosync::Error::Unauthorized) = settle(
          &server.client,
          title,
          &document,
          step.outcome,
          to,
          &mut report,
          &mut sent,
        ) {
          break;
        }
      }
    }
  }
  record(&sent, &mut report);
  Ok(report)
}

/// Opens the device for writing: a Tolino. On a Kobo the position is not
/// written back yet (P2), only sent.
fn tolino(options: &Options) -> Result<Session, String> {
  let session = Session::open(options).map_err(|e| e.to_string())?;
  if session.family != "Tolino" {
    return Err("only a Tolino takes positions from the server for now".into());
  }
  Ok(session)
}

/// Writes `writes` into the device after a backup, and keeps `taken` in the
/// history.
fn write(
  session: &mut Session,
  writes: Vec<(String, Write, Entry)>,
  report: &mut Report,
) -> Result<(), String> {
  if writes.is_empty() {
    return Ok(());
  }
  let only: Vec<Write> = writes.iter().map(|(_, w, _)| w.clone()).collect();
  let (backup, pages_dropped) = match session.write(&only).map_err(|e| e.to_string())? {
    Written::Mounted {
      backup,
      pages_dropped,
    } => (Some(backup), pages_dropped),
    Written::Live {
      backup,
      pages_dropped,
    } => (backup, pages_dropped),
  };
  report.backup = backup;
  report.pages_dropped += pages_dropped;
  let mut taken = Vec::new();
  for (title, _, entry) in writes {
    report.books.push((title, Outcome::Written));
    taken.push(Entry {
      time: history::now(),
      ..entry
    });
  }
  record(&taken, report);
  Ok(())
}

/// Takes the server's positions for the books with `ids` into a Tolino:
/// what needs no answer is written at once (after a backup of the
/// database), the rest comes back as questions.
pub fn pull(options: &Options, ids: &[String]) -> Result<Report, String> {
  let server = server(options)?.ok_or("no server set up in config.ini")?;
  let mut session = tolino(options)?;
  let confirm = server.config.confirm;
  let mut report = Report::default();
  let mut writes = Vec::new();
  for book in session.books.iter().filter(|b| ids.contains(&b.content_id)) {
    let title = book
      .title
      .clone()
      .unwrap_or_else(|| book.content_id.clone());
    let step = session.pull_step(book, &server.client, confirm, false);
    let (remote, local, action) = match step.outcome {
      PullOutcome::Skip { message, .. } => {
        report.books.push((title, Outcome::Skipped(message)));
        continue;
      }
      PullOutcome::NothingStored => {
        report.books.push((title, Outcome::NothingStored));
        continue;
      }
      PullOutcome::Failed(e) => {
        let unauthorized = matches!(e, kosync::Error::Unauthorized);
        report.books.push((title, Outcome::Failed(e.to_string())));
        if unauthorized {
          break;
        }
        continue;
      }
      PullOutcome::Ready {
        remote,
        local,
        action,
        ..
      } => (remote, local, action),
    };
    match action {
      Action::InSync => {
        report.books.push((title, Outcome::InSync));
        continue;
      }
      Action::Keep => {
        report.books.push((title, Outcome::DeviceNewer));
        continue;
      }
      Action::Apply | Action::Ask { .. } => {}
    }
    let take = match session.take(book, &step.identity, &remote, local.as_ref()) {
      Ok(take) => take,
      Err(e) => {
        report.books.push((title, Outcome::Skipped(e)));
        continue;
      }
    };
    let entry = Entry {
      time: 0,
      direction: history::Direction::Pull,
      device: session.model.clone(),
      title: book.title.clone(),
      document: step.identity.document().unwrap_or_default().to_string(),
      position: take.write.position.clone(),
      percentage: remote.percentage,
      by: "gui".into(),
    };
    match (action, local) {
      (Action::Ask { backwards }, Some(local)) => report.questions.push(Question {
        title,
        local_percent: local.percentage,
        local_xpointer: local.xpointer.clone(),
        local_time: local.last_read.map(format_timestamp),
        remote: Remote::from(&remote),
        backwards,
        answer: Answer::Write {
          write: take.write,
          entry,
        },
      }),
      _ => writes.push((title, take.write, entry)),
    }
  }
  write(&mut session, writes, &mut report)?;
  Ok(report)
}

/// Does what the questions answered with yes ask for: sends the device's
/// positions, or writes the server's into the device.
pub fn answer(options: &Options, questions: Vec<Question>) -> Result<Report, String> {
  let mut report = Report::default();
  let (pulls, pushes): (Vec<Question>, Vec<Question>) =
    questions.into_iter().partition(Question::pulls);
  if !pushes.is_empty() {
    let server = server(options)?.ok_or("no server set up in config.ini")?;
    let mut sent = Vec::new();
    for q in pushes {
      let Answer::Send(update) = q.answer else {
        unreachable!("partitioned")
      };
      if let Err(kosync::Error::Unauthorized) =
        deliver(&server.client, q.title, update, &mut report, &mut sent)
      {
        break;
      }
    }
    record(&sent, &mut report);
  }
  if !pulls.is_empty() {
    let mut session = tolino(options)?;
    let writes = pulls
      .into_iter()
      .map(|q| match q.answer {
        Answer::Write { write, entry } => (q.title, write, entry),
        Answer::Send(_) => unreachable!("partitioned"),
      })
      .collect();
    write(&mut session, writes, &mut report)?;
  }
  Ok(report)
}

/// The last `n` entries of the history.
pub fn history(n: usize) -> Result<Vec<Entry>, String> {
  match history::default_path() {
    Some(path) => history::read(&path, n).map_err(|e| e.to_string()),
    None => Ok(Vec::new()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::path::PathBuf;
  use std::time::Duration;

  use nickelinksync::hash::partial_md5;

  use crate::common::{fake_device, fixtures, MockServer, KOBO_VERSION, PASSWORD, TOLINO_VERSION};
  use crate::test_data::{isolate, DataDir};

  /// Where book-a.epub is on the fake Tolino, as KOReader's XPointer.
  const XPOINTER: &str = "/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0";
  /// `DateLastRead` of that book.
  const LAST_READ: i64 = 1_767_441_600;
  /// Further on than the device.
  const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
  /// How the Tolino stores that place.
  const AHEAD_ON_DEVICE: &str = "OEBPS/xhtml/xhtml-0-89.xhtml#point(/1/2/1/1:0)";
  /// Further back than the device.
  const BEHIND: &str = "/body[1]/DocFragment[40]/body[1]/p[1]/text()[1].0";
  const BOOK_A: &str = "file:///mnt/onboard/Books/book-a.epub";

  fn book_hash() -> String {
    partial_md5(&fixtures().join("epub/book-a.epub")).unwrap()
  }

  struct Setup {
    root: PathBuf,
    server: MockServer,
    options: Options,
    _data: DataDir,
  }

  impl Setup {
    fn new(name: &str) -> Self {
      let data = isolate();
      let root = fake_device(
        &format!("gui-{name}"),
        "tolino-shinecolor-5.18-placeholder-books.sql",
        TOLINO_VERSION,
      );
      let server = MockServer::start();
      let config = server.config(&root, PASSWORD);
      let options = Options {
        device: root.clone(),
        config: Some(config),
        book: None,
        content_id: None,
        live: false,
        timeout: Duration::from_secs(5),
        library: None,
      };
      Setup {
        root,
        server,
        options,
        _data: data,
      }
    }
  }

  impl Drop for Setup {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.root);
    }
  }

  #[test]
  fn push_sends_and_compare_shows_it() {
    let s = Setup::new("push");
    let ServerState::Compared(before) = compare(&s.options, Kind::Nickel) else {
      panic!("not compared")
    };
    assert_eq!(before[BOOK_A].remote, None);
    assert_eq!(before[BOOK_A].outlook, Some(Outlook::Push));

    let report = push(&s.options, Kind::Nickel, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Sent)]);
    assert!(report.questions.is_empty());
    assert_eq!(s.server.puts().len(), 1);
    assert_eq!(
      s.server.document(&book_hash()).unwrap()["progress"],
      XPOINTER
    );
    let history = history(200).unwrap();
    let entry = history
      .iter()
      .find(|e| e.document == book_hash() && e.by == "gui")
      .unwrap();
    assert_eq!(entry.position, XPOINTER);
    assert_eq!(entry.device, "Tolino Shine Color");

    let ServerState::Compared(after) = compare(&s.options, Kind::Nickel) else {
      panic!("not compared")
    };
    assert_eq!(after[BOOK_A].outlook, Some(Outlook::InSync));
    assert_eq!(after[BOOK_A].remote.as_ref().unwrap().xpointer, XPOINTER);
    // Pushing again sends nothing.
    let report = push(&s.options, Kind::Nickel, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::InSync)]);
    assert_eq!(s.server.puts().len(), 1);
  }

  #[test]
  fn a_question_is_sent_only_after_the_answer() {
    // KOReader got further, then the book was only opened on the device.
    let s = Setup::new("ask");
    s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ - 60);
    let mut report = push(&s.options, Kind::Nickel, &[BOOK_A.to_string()]).unwrap();
    assert!(report.books.is_empty(), "{report:?}");
    assert_eq!(report.questions.len(), 1);
    let q = &report.questions[0];
    assert!(q.backwards);
    assert_eq!(q.remote.xpointer, AHEAD);
    assert_eq!(q.local_xpointer, XPOINTER);
    assert!(s.server.puts().is_empty());

    let report = answer(&s.options, std::mem::take(&mut report.questions)).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Sent)]);
    assert_eq!(
      s.server.document(&book_hash()).unwrap()["progress"],
      XPOINTER
    );
  }

  /// `ChapterIDBookmarked` and `___PercentRead` of book-a.epub.
  fn row(root: &std::path::Path) -> (String, i64) {
    let session = Session::open(&Options {
      device: root.to_path_buf(),
      config: None,
      book: None,
      content_id: Some(BOOK_A.into()),
      live: false,
      timeout: Duration::from_secs(5),
      library: None,
    })
    .unwrap();
    let book = &session.books[0];
    (book.position.clone().unwrap(), book.percent_read)
  }

  #[test]
  fn pull_writes_after_a_backup_and_records_it() {
    let s = Setup::new("pull");
    s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ + 60);
    let report = pull(&s.options, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Written)]);
    assert!(report.questions.is_empty());
    assert!(
      report.backup.as_ref().is_some_and(|b| b.exists()),
      "{report:?}"
    );
    assert_eq!(row(&s.root), (AHEAD_ON_DEVICE.into(), 90));
    let entry = history(200)
      .unwrap()
      .into_iter()
      .find(|e| e.position == AHEAD_ON_DEVICE)
      .unwrap();
    assert_eq!(entry.direction, history::Direction::Pull);
    assert_eq!(entry.by, "gui");

    assert_eq!(report.pages_dropped, 0);
    let text = crate::i18n::report(crate::i18n::Lang::En, &report);
    assert!(text.contains("Eject the device before you open"), "{text}");

    // In line now: nothing more to write, and no new backup.
    let report = pull(&s.options, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::InSync)]);
    assert_eq!(report.backup, None);
  }

  /// Nickel's cached pages of the chapter hide the written place: they are
  /// dropped, and the report asks for a restart instead of only ejecting.
  #[test]
  fn a_pull_that_drops_cached_pages_asks_for_a_restart() {
    let s = Setup::new("pull-pages");
    let chapter = "OEBPS/xhtml/xhtml-0-89.xhtml";
    let conn = rusqlite::Connection::open(s.root.join(".kobo/KoboReader.sqlite")).unwrap();
    conn
      .execute_batch(
        "CREATE TABLE CachePersistenceRenderedPageData (id TEXT PRIMARY KEY,
           pageStartLocation TEXT NOT NULL, pageEndLocation TEXT NOT NULL, contentId TEXT NOT NULL);
         CREATE TABLE CachePersistenceRenderedPageChapterData (id INTEGER PRIMARY KEY AUTOINCREMENT,
           cachePersistenceRenderedPageDataId TEXT NOT NULL, contentId TEXT NOT NULL);",
      )
      .unwrap();
    for (id, start, end) in [
      ("a", "/1/2/5/1:0", "/1/2/9/1:0"),
      ("b", "/1/2/9/1:1", "/1/2/13/1:0"),
    ] {
      conn
        .execute(
          "INSERT INTO CachePersistenceRenderedPageData VALUES (?1, ?2, ?3, ?4)",
          [
            id,
            &format!("{chapter}#point({start})"),
            &format!("{chapter}#point({end})"),
            BOOK_A,
          ],
        )
        .unwrap();
      conn
        .execute(
          "INSERT INTO CachePersistenceRenderedPageChapterData
             (cachePersistenceRenderedPageDataId, contentId) VALUES (?1, ?2)",
          [id, BOOK_A],
        )
        .unwrap();
    }
    drop(conn);
    s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ + 60);
    let report = pull(&s.options, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Written)]);
    assert_eq!(report.pages_dropped, 2);
    let text = crate::i18n::report(crate::i18n::Lang::En, &report);
    assert!(
      text.contains("restart it before you open the book"),
      "{text}"
    );
    assert!(!text.contains("Eject the device before you open"), "{text}");
    let text = crate::i18n::report(crate::i18n::Lang::De, &report);
    assert!(text.contains("neu starten"), "{text}");
  }

  #[test]
  fn after_a_pull_inside_a_paragraph_the_server_column_says_same() {
    let s = Setup::new("pull-inside");
    let inside = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].20";
    s.server.store(&book_hash(), inside, 0.9, LAST_READ + 60);
    let report = pull(&s.options, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Written)]);
    let ServerState::Compared(after) = compare(&s.options, Kind::Nickel) else {
      panic!("not compared")
    };
    assert_eq!(after[BOOK_A].outlook, Some(Outlook::InSync));
    let report = push(&s.options, Kind::Nickel, &[BOOK_A.to_string()]).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::InSync)]);
    assert!(s.server.puts().is_empty());
  }

  #[test]
  fn a_pull_back_is_written_only_after_the_answer() {
    // The server is newer but further back than the device.
    let s = Setup::new("pull-ask");
    s.server.store(&book_hash(), BEHIND, 0.5, LAST_READ + 60);
    let before = row(&s.root);
    let mut report = pull(&s.options, &[BOOK_A.to_string()]).unwrap();
    assert!(report.books.is_empty(), "{report:?}");
    assert_eq!(report.backup, None);
    let q = &report.questions[0];
    assert!(q.pulls() && q.backwards);
    assert_eq!(row(&s.root), before);

    let report = answer(&s.options, std::mem::take(&mut report.questions)).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Outcome::Written)]);
    assert!(report.backup.is_some());
    assert_ne!(row(&s.root), before);
    assert_eq!(row(&s.root).1, 50);
  }

  #[test]
  fn a_kobo_is_not_written() {
    let _data = isolate();
    let root = fake_device(
      "gui-pull-kobo",
      "kobo-clarabw-4.46-placeholder-books.sql",
      KOBO_VERSION,
    );
    let server = MockServer::start();
    let options = Options {
      device: root.clone(),
      config: Some(server.config(&root, PASSWORD)),
      book: None,
      content_id: None,
      live: false,
      timeout: Duration::from_secs(5),
      library: None,
    };
    let before = fs::read(root.join(".kobo/KoboReader.sqlite")).unwrap();
    assert!(pull(&options, &[BOOK_A.to_string()]).is_err());
    assert_eq!(
      fs::read(root.join(".kobo/KoboReader.sqlite")).unwrap(),
      before
    );
    let _ = fs::remove_dir_all(&root);
  }

  #[test]
  fn the_overview_puts_one_book_on_two_devices_in_one_line() {
    let s = Setup::new("overview");
    let kobo = fake_device(
      "gui-overview-kobo",
      "kobo-clarabw-4.46-placeholder-books.sql",
      KOBO_VERSION,
    );
    s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ + 60);
    let found = |root: &PathBuf| crate::devices::Found {
      root: root.clone(),
      kind: Kind::Nickel,
      name: "eReader".into(),
    };
    let overview = crate::overview::build(&[found(&s.root), found(&kobo)]);
    let names: Vec<&str> = overview.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Tolino Shine Color", "Kobo Clara BW"]);
    let line = overview
      .lines
      .iter()
      .find(|l| l.document == book_hash())
      .unwrap();
    assert_eq!(line.title, "Book A");
    assert!(line.cells.iter().all(Option::is_some), "{line:?}");
    // The tests' environment has no configuration: the server is not asked.
    assert_eq!(overview.server, Ok(false));
    assert_eq!(line.server, None);
    // With one, it is.
    let remotes = remotes(&s.options, &[book_hash()]).unwrap().unwrap();
    assert_eq!(remotes[&book_hash()].xpointer, AHEAD);
    let _ = fs::remove_dir_all(&kobo);
  }

  #[test]
  fn without_configuration_nothing_is_compared() {
    let s = Setup::new("no-config");
    let options = Options {
      config: None,
      ..s.options.clone()
    };
    assert_eq!(compare(&options, Kind::Nickel), ServerState::NotConfigured);
    assert!(push(&options, Kind::Nickel, &[BOOK_A.to_string()]).is_err());
  }
}
