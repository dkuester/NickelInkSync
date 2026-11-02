//! `device prepare` and the CLI on the device (`--live`) against the local
//! mock server: the library index, the mappings and the translation tables
//! in `.adds/NickelInkSync`, positions on the device the same as on the
//! desktop with the original EPUB, no push without an index, and mappings
//! made or removed on the device taken over by the desktop.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer};
use nickelinksync::hash::partial_md5;
use nickelinksync::mapping::{DeviceBook, Mapping, Original, Store};
use rusqlite::Connection;
use serde_json::{json, Value};

const KOBO_6_VERSION: &str =
  "N000000000000,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n";
const KEPUB: &str = "file:///mnt/onboard/Books/book-a.kepub.epub";
const EPUB: &str = "file:///mnt/onboard/Books/book-a.epub";
const OTHER: &str = "file:///mnt/onboard/Books/book-c.epub";
/// In R2-63 of the kepub, at "schließen".
const READ_TO: &str = "OEBPS/text/chapter2.xhtml#kobo.64.1:13";

struct Setup {
  root: PathBuf,
  data: PathBuf,
  library: PathBuf,
  server: MockServer,
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
      server,
      config,
      root,
    }
  }

  fn run(&self, args: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(&self.root)
      .arg("--config")
      .arg(&self.config)
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

  fn store(&self) -> Store {
    Store::load(&self.data.join("mappings.json")).unwrap()
  }

  /// Maps the kepub to Book R in the library, on the desktop.
  fn map_kepub(&self) {
    let mut store = self.store();
    store.insert(self.kepub_mapping(self.library.join("book-r.epub").to_str().unwrap()));
    store.save().unwrap();
  }

  fn kepub_mapping(&self, original_path: &str) -> Mapping {
    Mapping {
      device: DeviceBook::File {
        hash: self.kepub_hash(),
        path: "Books/book-a.kepub.epub".into(),
        title: Some("Book R".into()),
        author: None,
      },
      epub: Original {
        path: original_path.into(),
        hash: r_hash(),
        title: None,
        author: None,
      },
      created: "2026-01-01T12:00:00Z".into(),
    }
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

  fn puts(&self) -> Vec<Value> {
    self
      .server
      .puts()
      .iter()
      .map(|r| serde_json::from_str(&r.body).unwrap())
      .collect()
  }
}

fn r_hash() -> String {
  partial_md5(&fixtures().join("epub/book-r.epub")).unwrap()
}

/// The `koreader` line of `status` for the book.
fn koreader(out: &str) -> &str {
  out
    .lines()
    .find_map(|l| l.strip_prefix("  koreader  "))
    .unwrap_or_else(|| panic!("no koreader line: {out}"))
}

fn files_below(dir: &Path) -> Vec<String> {
  let mut out = Vec::new();
  for entry in fs::read_dir(dir).unwrap() {
    let entry = entry.unwrap();
    if entry.file_type().unwrap().is_dir() {
      out.extend(files_below(&entry.path()));
    } else {
      out.push(entry.path().to_string_lossy().into_owned());
    }
  }
  out
}

#[test]
fn the_device_sends_what_the_desktop_sends() {
  let s = Setup::new("push");
  s.map_kepub();
  let out = s.prepare();
  assert!(
    out.contains("Prepared  2 originals in the index, 1 mappings"),
    "{out}"
  );
  assert!(
    out.contains("Tables    1 written, 0 up to date, 0 removed"),
    "{out}"
  );
  // Index, mappings, one table; no EPUB.
  let mut files: Vec<String> = files_below(&s.dir())
    .iter()
    .map(|f| f[s.dir().to_str().unwrap().len() + 1..].to_string())
    .collect();
  files.sort();
  assert_eq!(
    files,
    [
      "library.json".to_string(),
      "mappings.json".into(),
      format!("tables/{}.json", s.kepub_hash()),
    ]
  );
  let index: Value =
    serde_json::from_str(&fs::read_to_string(s.dir().join("library.json")).unwrap()).unwrap();
  let r = index["books"]
    .as_array()
    .unwrap()
    .iter()
    .find(|b| b["hash"] == r_hash())
    .unwrap();
  assert_eq!(r["title"], "Book R");
  assert_eq!(r["authors"], json!(["Author R"]));
  assert_eq!(
    s.device_mappings()["mappings"][0]["device"]["hash"],
    s.kepub_hash()
  );

  // The desktop with the original and the device with the table.
  let library = s.library.to_str().unwrap();
  let (_, desktop) = s.run(&["status", "--content-id", KEPUB, "--library", library]);
  let (_, device) = s.run(&["status", "--live", "--content-id", KEPUB]);
  assert!(
    device.contains("Prepared  2 originals, 1 mappings"),
    "{device}"
  );
  let expected = "/body[1]/DocFragment[3]/body[1]/p[63]/text()[1].0 (moved back 11 characters to the paragraph start)";
  assert_eq!(koreader(&desktop), expected, "{desktop}");
  assert_eq!(koreader(&device), expected, "{device}");

  let (output, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("\nNICKELINKSYNC_SENT\t45\n"), "{out}");
  let sent = s.puts();
  assert_eq!(sent.len(), 1, "{out}");
  assert_eq!(sent[0]["document"], r_hash());
  assert_eq!(
    sent[0]["progress"],
    "/body[1]/DocFragment[3]/body[1]/p[63]/text()[1].0"
  );

  // Again: the table is kept.
  let out = s.prepare();
  assert!(
    out.contains("Tables    0 written, 1 up to date, 0 removed"),
    "{out}"
  );
}

#[test]
fn the_device_takes_what_the_desktop_takes() {
  let s = Setup::new("pull");
  s.map_kepub();
  s.prepare();
  // KOReader in R3-130.
  s.server.store(
    &r_hash(),
    "/body[1]/DocFragment[4]/body[1]/p[130]/text()[1].0",
    0.8,
    1_900_000_000,
  );
  let library = s.library.to_str().unwrap();
  let (output, desktop) = s.run(&[
    "pull",
    "--emit",
    "--force",
    "--content-id",
    KEPUB,
    "--library",
    library,
  ]);
  assert!(output.status.success(), "{desktop}");
  let (output, device) = s.run(&["pull", "--live", "--emit", "--force", "--content-id", KEPUB]);
  assert!(output.status.success(), "{device}");
  let position = "\nNICKELINKSYNC_POSITION\t80\tOEBPS/text/chapter3.xhtml#kobo.131.1\n";
  assert!(desktop.contains(position), "{desktop}");
  assert!(device.contains(position), "{device}");
}

#[test]
fn without_an_index_nothing_is_sent() {
  let s = Setup::new("no-index");
  let (output, out) = s.run(&["push", "--live", "--emit", "--content-id", EPUB]);
  assert!(!output.status.success(), "{out}");
  assert!(
    out.contains("\nNICKELINKSYNC_NO_INDEX\tthe device is not prepared"),
    "{out}"
  );
  assert!(!out.contains("NICKELINKSYNC_SENT"), "{out}");
  assert!(s.server.puts().is_empty());
  // Taking a position stays allowed.
  let hash = partial_md5(&s.root.join("Books/book-a.epub")).unwrap();
  s.server.store(
    &hash,
    "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0",
    0.8,
    1_900_000_000,
  );
  let (output, out) = s.run(&["pull", "--live", "--emit", "--force", "--content-id", EPUB]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("\nNICKELINKSYNC_POSITION\t80\t"), "{out}");
}

#[test]
fn only_originals_and_mapped_books_are_sent() {
  let s = Setup::new("only-known");
  s.prepare();
  // book-a.epub is in the library: sent.
  let (output, out) = s.run(&["push", "--live", "--emit", "--content-id", EPUB]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("\nNICKELINKSYNC_SENT\t"), "{out}");
  // book-c.epub is not; the kepub is not mapped.
  for id in [OTHER, KEPUB] {
    let (_, out) = s.run(&["push", "--live", "--emit", "--content-id", id]);
    assert!(
      out.contains("\nNICKELINKSYNC_NOT_SENT\tnot-mapped\t"),
      "{out}"
    );
  }
  assert_eq!(s.puts().len(), 1);
}

#[test]
fn mappings_made_and_removed_on_the_device_are_taken_over() {
  let s = Setup::new("reconcile");
  s.prepare();
  // On the device (as C6 will do): the kepub mapped to Book R, known only
  // by its checksum, and a book whose original the library lacks.
  let mut added = serde_json::to_value(s.kepub_mapping("")).unwrap();
  let mut stranger = added.clone();
  stranger["device"]["hash"] = json!("00000000000000000000000000000001");
  stranger["device"]["title"] = json!("Stranger");
  stranger["epub"]["hash"] = json!("00000000000000000000000000000002");
  added["created"] = json!("2026-02-01T12:00:00Z");
  let mut device = s.device_mappings();
  device["added"] = json!([added, stranger]);
  fs::write(s.dir().join("mappings.json"), device.to_string()).unwrap();

  // Mapped on the device, but no table yet: not sent.
  let (_, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(
    out.contains("\nNICKELINKSYNC_NOT_SENT\tno-table\t"),
    "{out}"
  );

  let out = s.prepare();
  assert!(out.contains("Imported  Book R → "), "{out}");
  assert!(out.contains("Kept      Stranger: its original"), "{out}");
  let store = s.store();
  let m = store.for_file(&s.kepub_hash()).unwrap();
  assert_eq!(m.epub.path, s.library.join("book-r.epub").to_str().unwrap());
  assert_eq!(m.created, "2026-02-01T12:00:00Z");
  let device = s.device_mappings();
  assert_eq!(device["mappings"].as_array().unwrap().len(), 1);
  assert_eq!(device["added"].as_array().unwrap().len(), 1);
  assert_eq!(device["added"][0]["device"]["title"], "Stranger");
  let table = s.dir().join(format!("tables/{}.json", s.kepub_hash()));
  assert!(table.is_file());
  let (output, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(output.status.success(), "{out}");
  assert_eq!(s.puts()[0]["document"], r_hash());

  // Removed on the device: gone from the store, its table too.
  let mut device = s.device_mappings();
  device["removed"] = json!([s.kepub_hash()]);
  fs::write(s.dir().join("mappings.json"), device.to_string()).unwrap();
  let (_, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(
    out.contains("\nNICKELINKSYNC_NOT_SENT\tnot-mapped\t"),
    "{out}"
  );
  let out = s.prepare();
  assert!(
    out.contains("Removed   Book R (mapping removed on the device)"),
    "{out}"
  );
  assert!(
    out.contains("Tables    0 written, 0 up to date, 1 removed"),
    "{out}"
  );
  assert!(s.store().for_file(&s.kepub_hash()).is_none());
  assert!(!table.exists());
  assert!(s.device_mappings()["mappings"]
    .as_array()
    .unwrap()
    .is_empty());
}

#[test]
fn a_kindle_is_not_prepared() {
  let root = common::fake_kindle("prepare-kindle");
  let library = fixtures().join("epub");
  let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
    .args(["device", "prepare", "--device"])
    .arg(&root)
    .arg("--library")
    .arg(&library)
    .env("NICKELINKSYNC_DATA_DIR", root.join("data"))
    .output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("only a Kobo or Tolino can be prepared"),
    "{stderr}"
  );
}

#[test]
fn a_mapping_put_onto_a_prepared_device_syncs_at_once() {
  let s = Setup::new("put");
  let mapping = s.kepub_mapping(s.library.join("book-r.epub").to_str().unwrap());
  // Not prepared: nothing written.
  nickelinksync::prepare::put_mapping(&s.root, &mapping).unwrap();
  assert!(!s.dir().exists());
  s.prepare();
  nickelinksync::prepare::put_mapping(&s.root, &mapping).unwrap();
  assert_eq!(
    s.device_mappings()["mappings"][0]["device"]["hash"],
    s.kepub_hash()
  );
  let (output, out) = s.run(&["push", "--live", "--emit", "--content-id", KEPUB]);
  assert!(output.status.success(), "{out}");
  assert_eq!(s.puts()[0]["document"], r_hash());
}
