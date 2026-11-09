//! Mapping a book on the device itself (stage C6): `device unsynced`,
//! `device suggest` and `device map-book` as the plugin's dialog runs them,
//! and the mapping taken over by the next `device prepare`.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer};
use nickelinksync::hash::partial_md5;
use rusqlite::Connection;
use serde_json::Value;

const KOBO_6_VERSION: &str =
  "N000000000000,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n";
const KEPUB: &str = "file:///mnt/onboard/Books/book-a.kepub.epub";
const EPUB: &str = "file:///mnt/onboard/Books/book-a.epub";
/// In R2-63 of the kepub, at "schließen".
const READ_TO: &str = "OEBPS/text/chapter2.xhtml#kobo.64.1:13";

struct Setup {
  root: PathBuf,
  data: PathBuf,
  library: PathBuf,
  _server: MockServer,
  config: PathBuf,
}

impl Setup {
  /// A Kobo 6.0 whose kepub row holds Book R as kepub, read to
  /// [`READ_TO`]; a library with book-a.epub and Book R.
  fn new(name: &str) -> Self {
    let root = fake_device(
      &format!("prepare-{name}"),
      "kobo-clarabw-4.46-placeholder-books.sql",
      KOBO_6_VERSION,
    );
    fs::copy(
      fixtures().join("epub/book-r.kepub.epub"),
      root.join("Books/book-a.kepub.epub"),
    )
    .unwrap();
    Connection::open(root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .execute(
        "UPDATE content SET ChapterIDBookmarked = ?1, ___PercentRead = 45 WHERE ContentID = ?2",
        [READ_TO, KEPUB],
      )
      .unwrap();
    let library = root.join("library");
    fs::create_dir_all(&library).unwrap();
    for file in ["book-a.epub", "book-r.epub"] {
      fs::copy(fixtures().join("epub").join(file), library.join(file)).unwrap();
    }
    let server = MockServer::start();
    let config = server.config(&root, common::PASSWORD);
    Setup {
      data: root.join("data"),
      library,
      _server: server,
      config,
      root,
    }
  }

  fn run(&self, args: &[&str]) -> (Output, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nickelinksync"));
    command.args(args).arg("--device").arg(&self.root);
    // The plugin's dialog commands take no configuration file.
    if !matches!(
      args.get(1),
      Some(&("unsynced" | "suggest" | "map-book" | "trigger"))
    ) {
      command.arg("--config").arg(&self.config);
    }
    let output = command
      .env("NICKELINKSYNC_DATA_DIR", &self.data)
      .env("NICKELINKSYNC_BACKUP_DIR", self.root.join("backups"))
      .env("NICKELINKSYNC_SYSTEM_ROOT", self.root.join("no-system"))
      .output()
      .unwrap();
    let mut out = String::from_utf8_lossy(&output.stdout).into_owned();
    out.push_str(&String::from_utf8_lossy(&output.stderr));
    (output, out)
  }

  fn prepare(&self) -> String {
    let library = self.library.to_str().unwrap().to_string();
    let (output, out) = self.run(&["device", "prepare", "--library", &library]);
    assert!(output.status.success(), "{out}");
    out
  }

  fn kepub_hash(&self) -> String {
    partial_md5(&self.root.join("Books/book-a.kepub.epub")).unwrap()
  }

  fn dir(&self) -> PathBuf {
    self.root.join(".adds/NickelInkSync")
  }

  fn device_mappings(&self) -> Value {
    serde_json::from_str(&fs::read_to_string(self.dir().join("mappings.json")).unwrap()).unwrap()
  }
}

fn r_hash() -> String {
  partial_md5(&fixtures().join("epub/book-r.epub")).unwrap()
}

fn lines<'a>(out: &'a str, name: &str) -> Vec<Vec<&'a str>> {
  let prefix = format!("NICKELINKSYNC_{name}\t");
  out
    .lines()
    .filter_map(|l| l.strip_prefix(&prefix))
    .map(|l| l.split('\t').collect())
    .collect()
}

#[test]
fn a_kepub_is_mapped_on_the_device_and_waits_for_the_table() {
  let s = Setup::new("map");
  // Not prepared: the plugin says so and offers nothing.
  let (output, out) = s.run(&["device", "unsynced", "--live"]);
  assert!(!output.status.success(), "{out}");
  assert!(out.contains("NICKELINKSYNC_NO_INDEX\t"), "{out}");

  s.prepare();
  let (output, out) = s.run(&["device", "unsynced", "--live"]);
  assert!(output.status.success(), "{out}");
  let unsynced = lines(&out, "UNSYNCED");
  let kepub = unsynced.iter().find(|l| l[0] == KEPUB).expect(&out);
  assert_eq!(kepub[3], "not-mapped", "{out}");
  assert!(
    unsynced.iter().all(|l| l[0] != EPUB),
    "an original syncs: {out}"
  );

  // The row says "Book A", which the library has too: the owner picks.
  Connection::open(s.root.join(".kobo/KoboReader.sqlite"))
    .unwrap()
    .execute(
      "UPDATE content SET Title = 'Book R', Attribution = 'Author R' WHERE ContentID = ?1",
      [KEPUB],
    )
    .unwrap();
  // Suggestions come from the index by title and author.
  let (output, out) = s.run(&["device", "suggest", "--live", "--content-id", KEPUB]);
  assert!(output.status.success(), "{out}");
  let suggest = lines(&out, "SUGGEST");
  assert_eq!(suggest[0][0], r_hash(), "{out}");
  assert!(suggest.len() <= 5);

  // Map it: only mappings.json changes, as `added`, without a path.
  let before = fs::read_dir(s.dir().join("tables")).map_or(0, |d| d.count());
  let (output, out) = s.run(&[
    "device",
    "map-book",
    "--live",
    "--content-id",
    KEPUB,
    "--original",
    &r_hash(),
  ]);
  assert!(output.status.success(), "{out}");
  assert_eq!(lines(&out, "MAPPED")[0][2], "waits", "{out}");
  let added = &s.device_mappings()["added"];
  assert_eq!(added.as_array().unwrap().len(), 1);
  assert_eq!(added[0]["epub"]["hash"], r_hash());
  assert_eq!(added[0]["epub"]["path"], "");
  assert_eq!(added[0]["device"]["hash"], s.kepub_hash());
  assert_eq!(
    fs::read_dir(s.dir().join("tables")).map_or(0, |d| d.count()),
    before,
    "no table without the original"
  );

  // Mapped, but it does not send yet.
  let (_, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(out.contains("no-table"), "{out}");
  let (_, out) = s.run(&["device", "unsynced", "--live", "--content-id", KEPUB]);
  assert_eq!(lines(&out, "UNSYNCED")[0][3], "waits", "{out}");

  // The computer takes the mapping over and builds the table.
  let library = s.library.to_str().unwrap().to_string();
  let (output, out) = s.run(&["device", "prepare", "--library", &library]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("Imported"), "{out}");
  assert!(s
    .dir()
    .join("tables")
    .join(format!("{}.json", s.kepub_hash()))
    .exists());
  let (_, out) = s.run(&["device", "unsynced", "--live"]);
  assert!(
    lines(&out, "UNSYNCED").iter().all(|l| l[0] != KEPUB),
    "{out}"
  );
}

#[test]
fn an_original_is_not_mapped_to_itself_and_an_unknown_one_is_refused() {
  let s = Setup::new("refuse");
  s.prepare();
  let (output, out) = s.run(&[
    "device",
    "map-book",
    "--live",
    "--content-id",
    EPUB,
    "--original",
    &partial_md5(&s.root.join("Books/book-a.epub")).unwrap(),
  ]);
  assert!(!output.status.success(), "{out}");
  assert!(out.contains("needs no mapping"), "{out}");
  let (output, out) = s.run(&[
    "device",
    "map-book",
    "--live",
    "--content-id",
    KEPUB,
    "--original",
    "00000000000000000000000000000000",
  ]);
  assert!(!output.status.success(), "{out}");
  assert!(out.contains("not in the library index"), "{out}");
  assert!(s.device_mappings()["added"]
    .as_array()
    .is_none_or(|a| a.is_empty()));
}

#[test]
fn kobalt_stops_the_dialogs_commands() {
  let s = Setup::new("kobalt");
  s.prepare();
  let system = s.root.join("no-system");
  fs::create_dir_all(system.join("usr/local/Kobo/imageformats")).unwrap();
  fs::write(
    system.join("usr/local/Kobo/imageformats/libkobalt.so"),
    b"x",
  )
  .unwrap();
  fs::create_dir_all(s.root.join(".adds/kobalt")).unwrap();
  fs::write(s.root.join(".adds/kobalt/uninstall"), b"").unwrap();
  for args in [
    vec!["device", "unsynced", "--live"],
    vec!["device", "suggest", "--live", "--content-id", KEPUB],
    vec![
      "device",
      "map-book",
      "--live",
      "--content-id",
      KEPUB,
      "--original",
      "x",
    ],
  ] {
    let (output, out) = s.run(&args);
    assert!(!output.status.success(), "{out}");
    assert!(out.contains("NICKELINKSYNC_ERROR\tkobalt"), "{out}");
  }
}

#[test]
fn trigger_writes_the_order_for_the_plugin() {
  let s = Setup::new("trigger");
  let (output, out) = s.run(&["device", "trigger", "sync"]);
  assert!(output.status.success(), "{out}");
  assert_eq!(
    fs::read_to_string(s.dir().join("trigger")).unwrap(),
    "sync\n"
  );
  let (output, _) = s.run(&["device", "trigger", "other"]);
  assert!(!output.status.success());
}
