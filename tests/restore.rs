//! `restore` on fake devices: a backup that `pull --write` made goes back,
//! after a backup of the database as it is; a backup of another device or
//! from another database version does not.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer, TOLINO_VERSION};
use nickelinksync::hash::partial_md5;
use rusqlite::Connection;

const LAST_READ: i64 = 1_767_441_600;
const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
const BOOK: &str = "file:///mnt/onboard/Books/book-a.epub";

struct Setup {
  root: PathBuf,
  data: PathBuf,
  server: MockServer,
  config: PathBuf,
}

impl Setup {
  fn new(name: &str) -> Self {
    let root = fake_device(
      name,
      "tolino-shinecolor-5.18-placeholder-books.sql",
      TOLINO_VERSION,
    );
    let data = PathBuf::from(format!("{}-data", root.display()));
    let _ = fs::remove_dir_all(&data);
    let server = MockServer::start();
    let config = server.config(&root, common::PASSWORD);
    Setup {
      root,
      data,
      server,
      config,
    }
  }

  fn run(&self, args: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(&self.root)
      .env("NICKELINKSYNC_CONFIG", &self.config)
      .env("NICKELINKSYNC_DATA_DIR", &self.data)
      .env_remove("NICKELINKSYNC_BACKUP_DIR")
      .output()
      .unwrap();
    let mut out = String::from_utf8_lossy(&output.stdout).into_owned();
    out.push_str(&String::from_utf8_lossy(&output.stderr));
    (output, out)
  }

  fn db(&self) -> PathBuf {
    self.root.join(".kobo/KoboReader.sqlite")
  }

  fn position(&self) -> String {
    Connection::open(self.db())
      .unwrap()
      .query_row(
        "SELECT ChapterIDBookmarked FROM content WHERE ContentID = ?1",
        [BOOK],
        |r| r.get(0),
      )
      .unwrap()
  }

  fn backups(&self) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(self.data.join("backups"))
      .map(|d| d.map(|e| e.unwrap().path()).collect())
      .unwrap_or_default();
    found.retain(|p| p.extension().is_some_and(|e| e == "sqlite"));
    found.sort();
    found
  }

  /// Pulls a newer position, which makes the first backup.
  fn pull(&self) -> PathBuf {
    let hash = partial_md5(&fixtures().join("epub/book-a.epub")).unwrap();
    self.server.store(&hash, AHEAD, 0.9, LAST_READ + 60);
    let (output, out) = self.run(&["pull", "--book", "book-a.epub", "--write"]);
    assert!(output.status.success(), "{out}");
    let backups = self.backups();
    assert_eq!(backups.len(), 1, "{backups:?}");
    backups[0].clone()
  }
}

impl Drop for Setup {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
    let _ = fs::remove_dir_all(&self.data);
  }
}

#[test]
fn a_backup_goes_back_after_a_backup_of_the_database() {
  let s = Setup::new("restore");
  let before = s.position();
  let backup = s.pull();
  assert_ne!(s.position(), before);

  let (output, out) = s.run(&["restore"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains(&backup.display().to_string()), "{out}");
  assert!(!out.contains("device unknown"), "{out}");

  let (output, out) = s.run(&["restore", "--backup", backup.to_str().unwrap()]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("restart it before you open a book"), "{out}");
  assert_eq!(s.position(), before);
  // The state before the restore is kept too, named for this device.
  let backups = s.backups();
  assert_eq!(backups.len(), 2, "{backups:?}");
  let newest = backups.iter().find(|b| **b != backup).unwrap();
  let kept: String = Connection::open(newest)
    .unwrap()
    .query_row(
      "SELECT ChapterIDBookmarked FROM content WHERE ContentID = ?1",
      [BOOK],
      |r| r.get(0),
    )
    .unwrap();
  assert_ne!(kept, before);
  assert!(PathBuf::from(format!("{}.device", newest.display())).exists());
  // One file on the device, no journal.
  let left: Vec<_> = fs::read_dir(s.root.join(".kobo"))
    .unwrap()
    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
    .filter(|n| n.starts_with("KoboReader.sqlite-"))
    .collect();
  assert!(left.is_empty(), "{left:?}");
}

#[test]
fn another_devices_backup_is_refused() {
  let s = Setup::new("restore-other");
  let backup = s.pull();
  fs::write(format!("{}.device", backup.display()), "0123").unwrap();
  let (_, out) = s.run(&["restore"]);
  assert!(out.contains("No backups"), "{out}");
  let after_pull = s.position();
  let (output, out) = s.run(&["restore", "--backup", backup.to_str().unwrap()]);
  assert!(!output.status.success(), "{out}");
  assert!(out.contains("another device"), "{out}");
  assert_eq!(s.position(), after_pull);
}

#[test]
fn a_backup_of_another_database_version_is_refused() {
  let s = Setup::new("restore-version");
  let backup = s.pull();
  Connection::open(&backup)
    .unwrap()
    .execute("UPDATE DbVersion SET version = version - 1", [])
    .unwrap();
  // An old backup, without a note of its device.
  fs::remove_file(format!("{}.device", backup.display())).unwrap();
  let (_, out) = s.run(&["restore"]);
  assert!(out.contains("device unknown"), "{out}");
  let after_pull = s.position();
  let (output, out) = s.run(&["restore", "--backup", backup.to_str().unwrap()]);
  assert!(!output.status.success(), "{out}");
  assert!(out.contains("database version"), "{out}");
  assert_eq!(s.position(), after_pull);
  assert_eq!(s.backups().len(), 1);
}
