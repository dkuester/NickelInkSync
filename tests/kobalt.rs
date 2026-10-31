//! With Kobalt installed on a fake Tolino, nothing is written into the
//! device database: `pull --write` is refused over USB, and on the device
//! (`--live`) `pull` and `push` stop before the database is opened.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer, TOLINO_VERSION};
use nickelinksync::hash::partial_md5;

/// `DateLastRead` of book-a.epub on the fake Tolino, 2026-01-03T12:00:00Z.
const LAST_READ: i64 = 1_767_441_600;
const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
const BOOK: &str = "file:///mnt/onboard/Books/book-a.epub";

struct Setup {
  root: PathBuf,
  /// The device's root file system, for `--live`.
  system: PathBuf,
  server: MockServer,
  config: PathBuf,
}

impl Setup {
  fn new(name: &str) -> Self {
    let root = fake_device(
      &format!("kobalt-{name}"),
      "tolino-shinecolor-5.18-placeholder-books.sql",
      TOLINO_VERSION,
    );
    let system = PathBuf::from(format!("{}-system", root.display()));
    let _ = fs::remove_dir_all(&system);
    let server = MockServer::start();
    let config = server.config(&root, common::PASSWORD);
    let hash = partial_md5(&fixtures().join("epub/book-a.epub")).unwrap();
    server.store(&hash, AHEAD, 0.9, LAST_READ + 60);
    Setup {
      root,
      system,
      server,
      config,
    }
  }

  /// Kobalt as its package and first start leave it.
  fn install_kobalt(&self) {
    let lib = self.system.join("usr/local/Kobo/imageformats");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("libkobalt.so"), "").unwrap();
    fs::create_dir_all(self.root.join(".adds/kobalt/assets")).unwrap();
    fs::write(self.root.join(".adds/kobalt/uninstall"), "").unwrap();
  }

  fn touch(&self, file: &str) {
    let path = self.root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
  }

  fn run(&self, args: &[&str]) -> (Output, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(&self.root)
      .arg("--config")
      .arg(&self.config)
      .env("NICKELINKSYNC_BACKUP_DIR", self.root.join("backups"))
      .env("NICKELINKSYNC_DATA_DIR", self.root.join("no-data"))
      .env("NICKELINKSYNC_SYSTEM_ROOT", &self.system)
      .output()
      .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, out, err)
  }

  fn position(&self) -> String {
    rusqlite::Connection::open(self.root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .query_row(
        "SELECT ChapterIDBookmarked FROM content WHERE ContentID = ?1",
        [BOOK],
        |r| r.get(0),
      )
      .unwrap()
  }
}

impl Drop for Setup {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
    let _ = fs::remove_dir_all(&self.system);
  }
}

#[test]
fn pull_write_over_usb_is_refused() {
  let s = Setup::new("usb");
  s.install_kobalt();
  let before = s.position();
  let (output, _, err) = s.run(&["pull", "--book", "book-a.epub", "--write"]);
  assert!(!output.status.success());
  assert!(err.contains("Kobalt is installed"), "{err}");
  assert_eq!(s.position(), before);
  assert!(!s.root.join("backups").exists());
}

#[test]
fn pull_without_write_and_push_only_warn() {
  let s = Setup::new("warn");
  s.install_kobalt();
  let (output, out, err) = s.run(&["pull", "--book", "book-a.epub"]);
  assert!(output.status.success(), "{err}");
  assert!(err.contains("warning: Kobalt is installed"), "{err}");
  assert!(out.contains("Dry run"), "{out}");
  let (_, _, err) = s.run(&["push", "--book", "book-a.epub", "--dry-run"]);
  assert!(err.contains("sending anyway"), "{err}");
}

#[test]
fn live_pull_and_push_stop_with_a_kobalt_error() {
  let s = Setup::new("live");
  s.install_kobalt();
  let before = s.position();
  for args in [
    &["pull", "--live", "--write", "--emit", "--content-id", BOOK][..],
    &["push", "--live", "--emit", "--content-id", BOOK][..],
  ] {
    let (output, out, _) = s.run(args);
    assert!(!output.status.success());
    assert!(out.contains("NICKELINKSYNC_ERROR\tkobalt\t"), "{out}");
  }
  assert_eq!(s.position(), before);
  assert!(s.server.puts().is_empty());
}

#[test]
fn a_leftover_folder_blocks_nothing() {
  // Kobalt's own manual removal deletes only `uninstall`.
  let s = Setup::new("leftover");
  s.touch(".adds/kobalt/config.json");
  let (output, out, err) = s.run(&["pull", "--book", "book-a.epub", "--write"]);
  assert!(output.status.success(), "{err}");
  assert!(err.contains("not installed"), "{err}");
  assert!(out.contains("Wrote 1 position"), "{out}");
  assert_ne!(s.position(), "");
}

#[test]
fn ignore_kobalt_lets_it_write() {
  let s = Setup::new("ignored");
  s.install_kobalt();
  s.touch(".adds/NickelInkSync/ignore-kobalt");
  let before = s.position();
  let (output, out, err) = s.run(&["pull", "--book", "book-a.epub", "--write"]);
  assert!(output.status.success(), "{err}");
  assert!(err.contains("syncing anyway"), "{err}");
  assert!(out.contains("Wrote 1 position"), "{out}");
  assert_ne!(s.position(), before);
}
