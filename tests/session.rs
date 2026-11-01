//! The library's sync steps (`nickelinksync::session`) without the CLI, as
//! the desktop client uses them: a fake Tolino against the local mock
//! server, with the answer to a question given by the caller.

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use common::{fake_device, fixtures, MockServer, TOLINO_VERSION};
use nickelinksync::hash::partial_md5;
use nickelinksync::session::{
  self, Action, Identity, Options, PullOutcome, PushOutcome, Server, Session, Written,
};
use nickelinksync::sync::Confirm;

/// Where book-a.epub is on the fake Tolino, as KOReader's XPointer.
const XPOINTER: &str = "/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0";
/// `DateLastRead` of that book, 2026-01-03T12:00:00Z.
const LAST_READ: i64 = 1_767_441_600;
/// Further on than the device (spine item 89 against 81).
const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
/// How the Tolino stores that place.
const AHEAD_ON_DEVICE: &str = "OEBPS/xhtml/xhtml-0-89.xhtml#point(/1/2/1/1:0)";

/// Data and backup directories of their own, not the machine's mappings.
fn data_dir() -> &'static PathBuf {
  static DIR: OnceLock<PathBuf> = OnceLock::new();
  DIR.get_or_init(|| {
    let dir =
      std::env::temp_dir().join(format!("nickelinksync-{}-session-data", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    std::env::set_var("NICKELINKSYNC_DATA_DIR", &dir);
    std::env::set_var("NICKELINKSYNC_BACKUP_DIR", dir.join("backups"));
    dir
  })
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
    data_dir();
    let root = fake_device(
      name,
      "tolino-shinecolor-5.18-placeholder-books.sql",
      TOLINO_VERSION,
    );
    let server = MockServer::start();
    let options = Options {
      device: root.clone(),
      config: Some(server.config(&root, common::PASSWORD)),
      book: Some("book-a.epub".into()),
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

  fn open(&self) -> (Session, Server) {
    let session = Session::open(&self.options).unwrap();
    let server = Server::require(&self.options).unwrap();
    (session, server)
  }
}

impl Drop for Setup {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
  }
}

#[test]
fn push_step_sends_what_the_server_lacks() {
  let s = Setup::new("session-push");
  let (session, server) = s.open();
  assert_eq!(session.family, "Tolino");
  assert_eq!(session.books.len(), 1);
  let book = &session.books[0];
  let step = session.push_step(book, &server.client, Confirm::Backwards, false);
  assert!(
    matches!(&step.identity, Identity::Own { hash, .. } if *hash == book_hash()),
    "{step:?}"
  );
  let PushOutcome::Ready {
    local,
    remote: None,
    action: Action::Apply,
    forced: false,
    ..
  } = step.outcome
  else {
    panic!("{step:?}")
  };
  assert_eq!(local.xpointer, XPOINTER);
  let update = session::update(&book_hash(), &local, "Tolino", "ID");
  server.client.update(&update).unwrap();
  assert_eq!(
    s.server.document(&book_hash()).unwrap()["progress"],
    XPOINTER
  );

  // Now both hold it.
  let step = session.push_step(book, &server.client, Confirm::Backwards, false);
  assert!(
    matches!(
      step.outcome,
      PushOutcome::Ready {
        action: Action::InSync,
        ..
      }
    ),
    "{step:?}"
  );
}

#[test]
fn push_step_leaves_the_question_to_the_caller() {
  // KOReader got further, then the book was only opened on the device.
  let s = Setup::new("session-ask");
  s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ - 60);
  let (session, server) = s.open();
  let book = &session.books[0];
  let step = session.push_step(book, &server.client, Confirm::Backwards, false);
  let PushOutcome::Ready {
    remote: Some(remote),
    action: Action::Ask { backwards: true },
    ..
  } = &step.outcome
  else {
    panic!("{step:?}")
  };
  assert_eq!(remote.progress, AHEAD);
  // Asking sends nothing by itself.
  assert!(s.server.puts().is_empty());

  // `never`: the newest wins; `force`: no question.
  for (confirm, force) in [(Confirm::Never, false), (Confirm::Backwards, true)] {
    let step = session.push_step(book, &server.client, confirm, force);
    assert!(
      matches!(
        step.outcome,
        PushOutcome::Ready {
          action: Action::Apply,
          ..
        }
      ),
      "{confirm} {force}: {step:?}"
    );
  }
}

#[test]
fn pull_step_take_and_write() {
  let s = Setup::new("session-pull");
  s.server.store(&book_hash(), AHEAD, 0.9, LAST_READ + 60);
  let (mut session, server) = s.open();
  let book = session.books[0].clone();
  let step = session.pull_step(&book, &server.client, Confirm::Backwards, false);
  let PullOutcome::Ready {
    remote,
    local: Some(local),
    plugin: None,
    moved: None,
    action: Action::Apply,
  } = &step.outcome
  else {
    panic!("{step:?}")
  };
  let take = session
    .take(&book, &step.identity, remote, Some(local))
    .unwrap();
  assert_eq!(take.write.position, AHEAD_ON_DEVICE);
  assert_eq!(take.write.percent, 90);
  assert_eq!(take.note, None);
  let Written::Mounted { backup, .. } = session.write(&[take.write]).unwrap() else {
    panic!("not a mounted device")
  };
  assert!(backup.starts_with(data_dir().join("backups")), "{backup:?}");
  assert!(backup.exists());

  // The device holds the server's position now.
  let (session, server) = s.open();
  let step = session.pull_step(&session.books[0], &server.client, Confirm::Backwards, false);
  assert!(
    matches!(
      step.outcome,
      PullOutcome::Ready {
        action: Action::InSync,
        ..
      }
    ),
    "{step:?}"
  );
}

#[test]
fn pull_step_without_a_server_position() {
  let s = Setup::new("session-empty");
  let (session, server) = s.open();
  let step = session.pull_step(&session.books[0], &server.client, Confirm::Backwards, false);
  assert!(
    matches!(step.outcome, PullOutcome::NothingStored),
    "{step:?}"
  );
}

/// Kobo firmware 6.0 has the `tolino_*` tables too: the model, then the
/// reseller, decide which family a device is.
#[test]
fn a_kobo_with_tolino_tables_stays_a_kobo() {
  data_dir();
  let open = |name: &str, version: &str, affiliate: Option<&str>| {
    let root = fake_device(name, "kobo-clarabw-4.46-placeholder-books.sql", version);
    rusqlite::Connection::open(root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .execute_batch("CREATE TABLE tolino_bookmark (id INTEGER PRIMARY KEY);")
      .unwrap();
    if let Some(a) = affiliate {
      fs::write(
        root.join(".kobo/affiliate.conf"),
        format!("[General]\naffiliate={a}\n"),
      )
      .unwrap();
    }
    let options = Options {
      device: root.clone(),
      config: Some(root.join("no-config.ini")),
      book: None,
      content_id: None,
      live: false,
      timeout: Duration::from_secs(5),
      library: None,
    };
    let family = Session::open(&options).unwrap().family;
    let _ = fs::remove_dir_all(&root);
    family
  };
  // Clara BW (model 391).
  assert_eq!(open("family-model", common::KOBO_VERSION, None), "Kobo");
  // An unknown model: the reseller.
  let unknown =
    "N000000000000,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000999\n";
  assert_eq!(open("family-kobo", unknown, Some("Kobo")), "Kobo");
  assert_eq!(open("family-thalia", unknown, Some("Thalia.de")), "Tolino");
  // Neither: the tables.
  assert_eq!(open("family-tables", unknown, None), "Tolino");
}
