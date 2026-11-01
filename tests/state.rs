//! `state.json` on a fake Tolino against the local mock server: the base
//! each sync leaves, the three-way rule it allows, the pending list of
//! failed pushes with `sync --pending`, and taking over the plugin's
//! `not-in-line.tsv`.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer, TOLINO_VERSION};
use nickelinksync::hash::partial_md5;
use rusqlite::Connection;
use serde_json::{json, Value};

/// `DateLastRead` of book-a.epub on the fake Tolino, 2026-01-03T12:00:00Z.
const LAST_READ: i64 = 1_767_441_600;
const BOOK: &str = "file:///mnt/onboard/Books/book-a.epub";
/// Where the fake Tolino's book-a.epub stands (spine item 81).
const STORED: &str = "OEBPS/xhtml/xhtml-0-81.xhtml#point(/1/2/1/1:0)";
/// Further on than the device, as KOReader would store it.
const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
const AHEAD_ON_DEVICE: &str = "OEBPS/xhtml/xhtml-0-89.xhtml#point(/1/2/1/1:0)";

/// A place in spine item `n - 1`, as KOReader stores it.
fn at(n: usize) -> String {
  format!("/body[1]/DocFragment[{n}]/body[1]/p[1]/text()[1].0")
}

struct Setup {
  root: PathBuf,
  server: MockServer,
  config: PathBuf,
}

impl Setup {
  fn new(name: &str) -> Self {
    let root = fake_device(
      &format!("state-{name}"),
      "tolino-shinecolor-5.18-placeholder-books.sql",
      TOLINO_VERSION,
    );
    let server = MockServer::start();
    let config = server.config(&root, common::PASSWORD);
    Setup {
      root,
      server,
      config,
    }
  }

  fn run(&self, args: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(&self.root)
      .arg("--config")
      .arg(&self.config)
      .env("NICKELINKSYNC_BACKUP_DIR", self.root.join("backups"))
      .env("NICKELINKSYNC_DATA_DIR", self.root.join("no-data"))
      .env("NICKELINKSYNC_SYSTEM_ROOT", self.root.join("no-system"))
      .output()
      .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    (output, out)
  }

  /// `push --live --emit` for book-a, as the plugin runs it on leaving.
  fn push_live(&self) -> (bool, String) {
    let (output, out) = self.run(&[
      "push",
      "--live",
      "--emit",
      "--timeout",
      "2",
      "--content-id",
      BOOK,
    ]);
    (output.status.success(), out)
  }

  /// `pull --write` for book-a over USB: the server's position as base.
  fn pull_write(&self) -> String {
    let (output, out) = self.run(&["pull", "--write", "--emit", "--content-id", BOOK]);
    assert!(output.status.success(), "{out}");
    out
  }

  /// Nickel moves the book to `position`, read at `last_read`.
  fn read_to(&self, position: &str, last_read: i64) {
    Connection::open(self.root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .execute(
        "UPDATE content SET ChapterIDBookmarked = ?1, DateLastRead = ?2 WHERE ContentID = ?3",
        rusqlite::params![
          position,
          nickelinksync::sync::iso_timestamp(last_read),
          BOOK
        ],
      )
      .unwrap();
  }

  fn position(&self) -> String {
    Connection::open(self.root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .query_row(
        "SELECT ChapterIDBookmarked FROM content WHERE ContentID = ?1",
        [BOOK],
        |r| r.get(0),
      )
      .unwrap()
  }

  fn state_path(&self) -> PathBuf {
    self.root.join(".adds/NickelInkSync/state.json")
  }

  fn state(&self) -> Value {
    serde_json::from_str(&fs::read_to_string(self.state_path()).unwrap()).unwrap()
  }

  /// The server cannot be reached: a configuration for a closed port.
  fn server_down(&self) -> PathBuf {
    let path = self.root.join("down.ini");
    fs::write(
      &path,
      format!(
        "server = http://127.0.0.1:9\nuser = {}\npassword = {}\n",
        common::USER,
        common::PASSWORD
      ),
    )
    .unwrap();
    path
  }

  fn run_with(&self, config: &PathBuf, args: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(&self.root)
      .arg("--config")
      .arg(config)
      .env("NICKELINKSYNC_BACKUP_DIR", self.root.join("backups"))
      .env("NICKELINKSYNC_DATA_DIR", self.root.join("no-data"))
      .env("NICKELINKSYNC_SYSTEM_ROOT", self.root.join("no-system"))
      .output()
      .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    (output, out)
  }
}

impl Drop for Setup {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
  }
}

fn hash() -> String {
  partial_md5(&fixtures().join("epub/book-a.epub")).unwrap()
}

fn lines(out: &str) -> Vec<&str> {
  out
    .lines()
    .filter(|l| l.starts_with("NICKELINKSYNC_"))
    .collect()
}

#[test]
fn pull_write_leaves_the_base() {
  let s = Setup::new("base");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ + 60);
  s.pull_write();
  assert_eq!(
    s.state()["books"][BOOK],
    json!({
      "title": "Book A",
      "base": {
        "document": hash(),
        "device": AHEAD_ON_DEVICE,
        "xpointer": AHEAD,
        "percentage": 0.9,
        "timestamp": LAST_READ + 60,
      }
    })
  );
  let (_, out) = s.run(&["status", "--content-id", BOOK]);
  assert!(
    out.contains(&format!("  base      {AHEAD} (90 %), agreed at ")),
    "{out}"
  );
  assert!(out.contains("  since     neither side moved"), "{out}");
}

/// The case from the review of 2026-10-31: Nickel sets `DateLastRead` on
/// merely opening the book, so the device looked newer than KOReader's
/// position stored before. Since the device did not move from the base,
/// the server wins.
#[test]
fn opening_alone_does_not_count_as_reading() {
  let s = Setup::new("opened");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ + 60);
  s.pull_write();
  // KOReader reads on; later Nickel opens the book without moving.
  s.server.store(&hash(), &at(95), 0.95, LAST_READ + 120);
  s.read_to(AHEAD_ON_DEVICE, LAST_READ + 3600);
  let (ok, out) = s.push_live();
  assert!(ok, "{out}");
  assert!(out.contains("  since     only the server moved"), "{out}");
  assert!(
    lines(&out)[0].starts_with("NICKELINKSYNC_NOT_SENT\tserver-newer\t"),
    "{out}"
  );
  assert!(s.server.puts().is_empty());
  let out = s.pull_write();
  assert!(out.contains("NICKELINKSYNC_POSITION\t95\t"), "{out}");
  assert_eq!(
    s.position(),
    "OEBPS/xhtml/xhtml-0-94.xhtml#point(/1/2/1/1:0)"
  );
}

/// Without a base the same situation keeps the device's newer time (the
/// old rule), here asking because it lies behind the server.
#[test]
fn without_a_base_the_newer_time_still_counts() {
  let s = Setup::new("no-base");
  s.server.store(&hash(), &at(95), 0.95, LAST_READ - 60);
  let (ok, out) = s.push_live();
  assert!(ok, "{out}");
  assert!(!out.contains("  since"), "{out}");
  assert!(out.contains("\nNICKELINKSYNC_UNANSWERED\t1\n"), "{out}");
  let conflict = lines(&out)
    .into_iter()
    .find(|l| l.starts_with("NICKELINKSYNC_CONFLICT\t"))
    .unwrap_or_else(|| panic!("{out}"));
  let f: Vec<&str> = conflict.split('\t').collect();
  assert_eq!(
    &f[1..],
    [
      BOOK,
      "backwards",
      "74",
      &LAST_READ.to_string(),
      STORED,
      "95",
      &(LAST_READ - 60).to_string(),
      "KOReader",
      &at(95),
    ]
  );
}

#[test]
fn the_device_moving_alone_wins_also_backwards() {
  let s = Setup::new("device-moved");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ + 60);
  s.pull_write();
  // Back to an earlier chapter on the device, with an older time than the
  // server's: the old rule would keep the server.
  s.read_to(STORED, LAST_READ);
  let (ok, out) = s.push_live();
  assert!(ok, "{out}");
  assert!(out.contains("  since     only the device moved"), "{out}");
  assert!(lines(&out)[0].starts_with("NICKELINKSYNC_SENT\t"), "{out}");
  assert_eq!(s.server.puts().len(), 1);
  // The sent position is the new base: both agree.
  let base = &s.state()["books"][BOOK]["base"];
  assert_eq!(base["device"], STORED);
  assert_eq!(base["timestamp"], 1_800_000_000);
  let (ok, out) = s.push_live();
  assert!(ok, "{out}");
  assert_eq!(lines(&out), ["NICKELINKSYNC_IN_SYNC"]);
}

#[test]
fn both_moving_is_a_conflict_by_the_old_rule() {
  let s = Setup::new("both-moved");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ + 60);
  s.pull_write();
  s.read_to(STORED, LAST_READ + 90);
  // The server moved back, and later than the device: pull asks.
  s.server.store(&hash(), &at(20), 0.2, LAST_READ + 120);
  let (output, out) = s.run(&["pull", "--write", "--emit", "--content-id", BOOK]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("  since     both moved"), "{out}");
  assert!(out.contains("\nNICKELINKSYNC_UNANSWERED\t1\n"), "{out}");
  assert!(
    out.contains(&format!(
      "\nNICKELINKSYNC_CONFLICT\t{BOOK}\tboth-moved\t90\t"
    )),
    "{out}"
  );
  assert_eq!(s.position(), STORED);
  // Unanswered: the base stays.
  assert_eq!(s.state()["books"][BOOK]["base"]["xpointer"], AHEAD);
}

#[test]
fn a_failed_push_on_the_device_is_kept_and_sent_later() {
  // A refused connection, not a timeout: Windows tries a refused port again
  // for about two seconds before it gives up, so the limit is longer.
  let s = Setup::new("pending");
  let down = s.server_down();
  let args = [
    "push",
    "--live",
    "--emit",
    "--timeout",
    "10",
    "--content-id",
    BOOK,
  ];
  let (output, out) = s.run_with(&down, &args);
  assert!(!output.status.success());
  assert!(out.contains("NICKELINKSYNC_ERROR\tnetwork\t"), "{out}");
  assert!(
    out.contains(&format!(
      "\nNICKELINKSYNC_PENDING\t{BOOK}\tmarked\tnetwork\n"
    )),
    "{out}"
  );
  let entry = &s.state()["books"][BOOK];
  assert_eq!(entry["pending"]["reason"], "network");
  assert_eq!(entry["title"], "Book A");
  // Over USB a failed push is not kept: the user sees it.
  let (_, out) = s.run_with(&down, &["push", "--timeout", "10", "--content-id", BOOK]);
  assert!(!out.contains("pending"), "{out}");
  let (_, out) = s.run(&["status", "--content-id", BOOK]);
  assert!(
    out.contains("  pending   push failed (network) at "),
    "{out}"
  );

  // Still no server: stays pending.
  let sync = ["sync", "--pending", "--live", "--emit", "--timeout", "10"];
  let (output, out) = s.run_with(&down, &sync);
  assert!(!output.status.success());
  assert!(
    out.contains(&format!(
      "\nNICKELINKSYNC_PENDING\t{BOOK}\terror\tnetwork\n"
    )),
    "{out}"
  );
  assert!(out.contains("\nNICKELINKSYNC_PENDING_LEFT\t1\n"), "{out}");

  // The server is back: sent, and nothing pending any more.
  let (output, out) = s.run(&sync);
  assert!(output.status.success(), "{out}");
  assert!(
    out.contains(&format!("\nNICKELINKSYNC_PENDING\t{BOOK}\tsent\t74\n")),
    "{out}"
  );
  assert!(out.contains("\nNICKELINKSYNC_PENDING_LEFT\t0\n"), "{out}");
  assert_eq!(s.server.puts().len(), 1);
  let entry = &s.state()["books"][BOOK];
  assert!(entry.get("pending").is_none(), "{entry}");
  assert_eq!(entry["base"]["device"], STORED);
  let (_, out) = s.run(&sync);
  assert_eq!(lines(&out), ["NICKELINKSYNC_PENDING_LEFT\t0"]);
}

#[test]
fn sync_pending_leaves_a_conflict_pending_and_drops_what_is_settled() {
  let s = Setup::new("pending-conflict");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ + 60);
  s.pull_write();
  // Read back on the device offline; the push fails.
  s.read_to(STORED, LAST_READ + 90);
  let down = s.server_down();
  let (_, out) = s.run_with(
    &down,
    &[
      "push",
      "--live",
      "--emit",
      "--timeout",
      "2",
      "--content-id",
      BOOK,
    ],
  );
  assert!(out.contains("\tmarked\t"), "{out}");
  // Meanwhile KOReader read on, later: both moved, the newer server wins.
  s.server.store(&hash(), &at(95), 0.95, LAST_READ + 120);
  let sync = ["sync", "--pending", "--live", "--emit"];
  let (output, out) = s.run(&sync);
  assert!(output.status.success(), "{out}");
  assert!(
    out.contains(&format!(
      "\nNICKELINKSYNC_PENDING\t{BOOK}\tserver-newer\t95\n"
    )),
    "{out}"
  );
  assert!(s.server.puts().is_empty());
  assert!(s.state()["books"][BOOK].get("pending").is_none());

  // Again pending, and this time the device is newer but behind: a
  // conflict that needs an answer and stays pending.
  s.read_to(STORED, LAST_READ + 200);
  let (_, out) = s.run_with(
    &down,
    &[
      "push",
      "--live",
      "--emit",
      "--timeout",
      "2",
      "--content-id",
      BOOK,
    ],
  );
  assert!(out.contains("\tmarked\t"), "{out}");
  let (output, out) = s.run(&sync);
  assert!(output.status.success(), "{out}");
  assert!(
    out.contains(&format!("\nNICKELINKSYNC_PENDING\t{BOOK}\tconflict\t\n")),
    "{out}"
  );
  assert!(
    out.contains(&format!("\nNICKELINKSYNC_CONFLICT\t{BOOK}\tboth-moved\t")),
    "{out}"
  );
  assert!(out.contains("\nNICKELINKSYNC_PENDING_LEFT\t1\n"), "{out}");
  assert!(s.state()["books"][BOOK].get("pending").is_some());
}

#[test]
fn a_pending_book_no_longer_on_the_device_is_dropped() {
  let s = Setup::new("pending-gone");
  let gone = "file:///mnt/onboard/Books/deleted.epub";
  fs::create_dir_all(s.state_path().parent().unwrap()).unwrap();
  fs::write(
    s.state_path(),
    json!({"version": 1, "books": {gone: {"pending": {"since": 1, "reason": "timeout"}}}})
      .to_string(),
  )
  .unwrap();
  let (output, out) = s.run(&["sync", "--pending", "--live", "--emit"]);
  assert!(output.status.success(), "{out}");
  assert!(
    out.contains(&format!(
      "\nNICKELINKSYNC_PENDING\t{gone}\tgone\tno longer on the device\n"
    )),
    "{out}"
  );
  assert_eq!(s.state()["books"], json!({}));
}

/// The plugin of 0.1.0 still writes `not-in-line.tsv`: the first run that
/// saves takes it over into `state.json`, with the same behaviour.
#[test]
fn not_in_line_tsv_is_taken_over() {
  let s = Setup::new("tsv");
  s.server.store(&hash(), AHEAD, 0.9, LAST_READ - 3600);
  let dir = s.root.join(".adds/NickelInkSync");
  fs::create_dir_all(&dir).unwrap();
  let list = dir.join("not-in-line.tsv");
  fs::write(&list, format!("{BOOK}\t{STORED}\n")).unwrap();
  // status only reads.
  let (_, out) = s.run(&["status", "--content-id", BOOK]);
  assert!(out.contains("  plugin    not in line"), "{out}");
  assert!(list.exists());
  assert!(!s.state_path().exists());
  // Never pushed; the run saves, so the list moves into state.json.
  let (ok, out) = s.push_live();
  assert!(ok, "{out}");
  assert_eq!(
    lines(&out),
    [format!("NICKELINKSYNC_NOT_SENT\tnot-in-line\t{STORED}")]
  );
  assert!(!list.exists());
  assert_eq!(s.state()["books"][BOOK], json!({"not_in_line": STORED}));
  // Again, now from state.json: the same.
  let (_, out) = s.push_live();
  assert!(
    out.contains("NICKELINKSYNC_NOT_SENT\tnot-in-line\t"),
    "{out}"
  );
  // Pull takes the server's position without asking, although the device
  // looks newer, and the book is in line with a base.
  let out = s.pull_write();
  assert!(out.contains("  plugin    Nickel opened here"), "{out}");
  assert_eq!(s.position(), AHEAD_ON_DEVICE);
  let entry = &s.state()["books"][BOOK];
  assert!(entry.get("not_in_line").is_none(), "{entry}");
  assert_eq!(entry["base"]["xpointer"], AHEAD);
  // The plugin adds the book again (Nickel missed the place once more).
  fs::write(&list, format!("{BOOK}\t{AHEAD_ON_DEVICE}\n")).unwrap();
  let (_, out) = s.push_live();
  assert!(
    out.contains("NICKELINKSYNC_NOT_SENT\tnot-in-line\t"),
    "{out}"
  );
  assert!(!list.exists());
  let entry = &s.state()["books"][BOOK];
  assert_eq!(entry["not_in_line"], AHEAD_ON_DEVICE);
}
