//! The window's side of `status` and `push`, on the library's steps: the
//! server's position for each book, and sending the device's positions.
//! A step that needs an answer (`confirm`) comes back as a [`Question`]
//! carrying the update to send, so the window asks and nothing is decided
//! twice.

use std::collections::HashMap;

use nickelinksync::history::{self, Entry};
use nickelinksync::kosync::{self, Client, Progress, Update};
use nickelinksync::session::{self, Action, KindleSession, Options, PushOutcome, Server, Session};
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

/// A book whose push needs an answer, with what would be sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
  pub title: String,
  pub local_percent: f64,
  pub local_xpointer: String,
  pub local_time: Option<String>,
  pub remote: Remote,
  /// The device position lies before the server's.
  pub backwards: bool,
  pub update: Update,
}

/// What happened to one book.
#[derive(Debug, Clone, PartialEq)]
pub enum Pushed {
  Sent,
  InSync,
  /// The server's position is newer: kept.
  ServerNewer,
  /// The plugin could not bring the book in line on the device.
  NotInLine,
  Skipped(String),
  Failed(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
  /// Title and outcome, in the order of the books.
  pub books: Vec<(String, Pushed)>,
  pub questions: Vec<Question>,
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
        let local = session.local(book, &identity).ok().map(|(l, _)| l);
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
      report.books.push((title, Pushed::Skipped(message)));
      return Ok(());
    }
    PushOutcome::NotInLine { .. } => {
      report.books.push((title, Pushed::NotInLine));
      return Ok(());
    }
    PushOutcome::Failed(e) => {
      report.books.push((title, Pushed::Failed(e.to_string())));
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
    (Action::InSync, _) => report.books.push((title, Pushed::InSync)),
    (Action::Keep, _) => report.books.push((title, Pushed::ServerNewer)),
    (Action::Ask { backwards }, Some(remote)) => report.questions.push(Question {
      title,
      local_percent: local.percentage,
      local_xpointer: local.xpointer.clone(),
      local_time: local.last_read.map(format_timestamp),
      remote: Remote::from(&remote),
      backwards,
      update,
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
      report.books.push((title, Pushed::Sent));
      Ok(())
    }
    Err(e) => {
      report.books.push((title, Pushed::Failed(e.to_string())));
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
      .push(("history.jsonl".into(), Pushed::Failed(e.to_string())));
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

/// Sends the updates of the questions answered with "device".
pub fn send_answers(options: &Options, questions: Vec<Question>) -> Result<Report, String> {
  let server = server(options)?.ok_or("no server set up in config.ini")?;
  let mut report = Report::default();
  let mut sent = Vec::new();
  for q in questions {
    if let Err(kosync::Error::Unauthorized) =
      deliver(&server.client, q.title, q.update, &mut report, &mut sent)
    {
      break;
    }
  }
  record(&sent, &mut report);
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
  use std::sync::OnceLock;
  use std::time::Duration;

  use nickelinksync::hash::partial_md5;

  use crate::common::{fake_device, fixtures, MockServer, PASSWORD, TOLINO_VERSION};

  /// Where book-a.epub is on the fake Tolino, as KOReader's XPointer.
  const XPOINTER: &str = "/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0";
  /// `DateLastRead` of that book.
  const LAST_READ: i64 = 1_767_441_600;
  /// Further on than the device.
  const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
  const BOOK_A: &str = "file:///mnt/onboard/Books/book-a.epub";

  /// No configuration and no mappings of the machine running the tests;
  /// the history in a directory of its own.
  fn isolate() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR
      .get_or_init(|| {
        let dir =
          std::env::temp_dir().join(format!("nickelinksync-gui-{}-data", std::process::id()));
        std::env::set_var("NICKELINKSYNC_DATA_DIR", &dir);
        std::env::set_var("NICKELINKSYNC_CONFIG", dir.join("no-config.ini"));
        dir
      })
      .clone()
  }

  fn book_hash() -> String {
    partial_md5(&fixtures().join("epub/book-a.epub")).unwrap()
  }

  struct Setup {
    root: PathBuf,
    server: MockServer,
    options: Options,
  }

  impl Setup {
    fn new(name: &str) -> Self {
      isolate();
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
    assert_eq!(report.books, [("Book A".to_string(), Pushed::Sent)]);
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
    assert_eq!(report.books, [("Book A".to_string(), Pushed::InSync)]);
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

    let report = send_answers(&s.options, std::mem::take(&mut report.questions)).unwrap();
    assert_eq!(report.books, [("Book A".to_string(), Pushed::Sent)]);
    assert_eq!(
      s.server.document(&book_hash()).unwrap()["progress"],
      XPOINTER
    );
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
