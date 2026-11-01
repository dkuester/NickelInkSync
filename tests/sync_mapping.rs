//! `status`, `push` and `pull` through a book mapping, against the local
//! mock server: a kepub synced under the original EPUB's checksum, a
//! differently converted EPUB on the Tolino, books a library does not know,
//! broken mappings, positions written back into a kepub, and an unmapped
//! kepub that is never synced.

mod common;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fake_device, fixtures, MockServer, KOBO_VERSION, TOLINO_VERSION};
use nickelinksync::epub::Epub;
use nickelinksync::hash::partial_md5;
use nickelinksync::mapping::{DeviceBook, Mapping, Original, Store};
use nickelinksync::position::{from_xpointer, PointFlavor};
use rusqlite::Connection;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// KOReader's position at the heading of xhtml-0-81.xhtml in book-a.epub
/// (`last_xpointer` in koreader/book-a.sdr).
const EPUB_HEADING: &str = "/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0";
/// Further on, spine item 89.
const AHEAD: &str = "/body[1]/DocFragment[90]/body[1]/p[1]/text()[1].0";
const KOBO_6_VERSION: &str =
  "N000000000000,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n";

struct Setup {
  root: PathBuf,
  data: PathBuf,
  server: MockServer,
  config: PathBuf,
}

impl Setup {
  fn new(name: &str, sql: &str, version: &str) -> Self {
    let root = fake_device(name, sql, version);
    let data = root.join("data");
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
      .arg("--config")
      .arg(&self.config)
      .env("NICKELINKSYNC_DATA_DIR", &self.data)
      .env("NICKELINKSYNC_BACKUP_DIR", self.root.join("backups"))
      .output()
      .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    (output, out)
  }

  /// Maps the device file to `original`.
  fn map(&self, file: &str, original: &Path) {
    let mut store = Store::load(&self.data.join("mappings.json")).unwrap();
    store.insert(Mapping {
      device: DeviceBook::File {
        hash: partial_md5(&self.root.join("Books").join(file)).unwrap(),
        path: format!("Books/{file}"),
        title: None,
        author: None,
      },
      epub: Original {
        path: original.to_str().unwrap().to_string(),
        hash: partial_md5(original).unwrap(),
        title: None,
        author: None,
      },
      created: "2026-01-01T12:00:00Z".into(),
    });
    store.save().unwrap();
  }

  /// A library folder with these fixture EPUBs, set in the configuration.
  fn library(&self, files: &[&str]) -> PathBuf {
    let dir = self.root.join("library");
    fs::create_dir_all(&dir).unwrap();
    for file in files {
      fs::copy(fixtures().join("epub").join(file), dir.join(file)).unwrap();
    }
    let mut config = fs::read_to_string(&self.config).unwrap();
    config.push_str(&format!("library = {}\n", dir.display()));
    fs::write(&self.config, config).unwrap();
    dir
  }

  fn stored(&self, content_id: &str) -> String {
    Connection::open(self.root.join(".kobo/KoboReader.sqlite"))
      .unwrap()
      .query_row(
        "SELECT ChapterIDBookmarked FROM content WHERE ContentID = ?1",
        [content_id],
        |r| r.get(0),
      )
      .unwrap()
  }
}

fn put(server: &MockServer) -> Vec<Value> {
  server
    .puts()
    .iter()
    .map(|r| serde_json::from_str(&r.body).unwrap())
    .collect()
}

fn epub_hash() -> String {
  partial_md5(&fixtures().join("epub/book-a.epub")).unwrap()
}

#[test]
fn push_sends_a_kepub_under_the_epub_checksum() {
  let s = Setup::new(
    "map-push-kepub",
    "kobo-clarabw-4.46-placeholder-books.sql",
    KOBO_VERSION,
  );
  s.map("book-a.kepub.epub", &fixtures().join("epub/book-a.epub"));
  let (output, out) = s.run(&["push", "--book", "book-a.kepub"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("  sync as   mapped to "), "{out}");
  let sent = put(&s.server);
  assert_eq!(sent.len(), 1, "{out}");
  // The kepub's span kobo.1.1 at the chapter heading is KOReader's own
  // position in the EPUB.
  assert_eq!(sent[0]["document"], epub_hash());
  assert_eq!(sent[0]["progress"], EPUB_HEADING);

  // status shows the same.
  let (_, out) = s.run(&["status", "--book", "book-a.kepub"]);
  assert!(
    out.contains(&format!("  koreader  {EPUB_HEADING}")),
    "{out}"
  );
  assert!(out.contains("  sync      in sync"), "{out}");
}

#[test]
fn with_a_library_unknown_books_are_not_synced() {
  let s = Setup::new(
    "map-library",
    "tolino-shinecolor-5.18-placeholder-books.sql",
    TOLINO_VERSION,
  );
  s.library(&["book-a.epub"]);
  let (output, out) = s.run(&["push"]);
  assert!(output.status.success(), "{out}");
  // book-a.epub is the library's file; book-a.kepub.epub and book-c.epub
  // are neither in the library nor mapped.
  let sent = put(&s.server);
  assert_eq!(sent.len(), 1, "{out}");
  assert_eq!(sent[0]["document"], epub_hash());
  assert_eq!(out.matches("skipped   not mapped").count(), 2, "{out}");

  let (_, out) = s.run(&["status"]);
  assert!(
    out.contains("  sync as   own checksum (the same file as in the library)"),
    "{out}"
  );
  assert!(out.contains("see `nickelinksync map suggest`"), "{out}");

  // Once the kepub is mapped to book-c.epub (not in the library), book-c.epub
  // on the device is known as that mapping's original.
  s.map("book-a.kepub.epub", &fixtures().join("epub/book-c.epub"));
  let (_, out) = s.run(&["status"]);
  assert!(
    out.contains("  sync as   own checksum (the original EPUB of a mapping)"),
    "{out}"
  );
  assert!(!out.contains("not mapped"), "{out}");
}

#[test]
fn broken_mapping_is_not_synced() {
  let s = Setup::new(
    "map-broken-sync",
    "kobo-clarabw-4.46-placeholder-books.sql",
    KOBO_VERSION,
  );
  let original = s.root.join("original.epub");
  fs::copy(fixtures().join("epub/book-a.epub"), &original).unwrap();
  s.map("book-a.kepub.epub", &original);
  fs::remove_file(&original).unwrap();
  let (output, out) = s.run(&["push", "--book", "book-a.kepub", "--emit"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("the EPUB is missing"), "{out}");
  assert!(
    out.contains("NICKELINKSYNC_NOT_SENT\tmapping-broken\t"),
    "{out}"
  );
  assert!(put(&s.server).is_empty());
}

/// book-a.epub as another conversion would leave it: a cover page in front
/// of the spine, so every chapter's spine index is one higher.
fn with_cover_page(from: &Path, to: &Path) {
  let mut zip = ZipArchive::new(fs::File::open(from).unwrap()).unwrap();
  let mut out = ZipWriter::new(fs::File::create(to).unwrap());
  let options = SimpleFileOptions::default();
  for i in 0..zip.len() {
    let mut entry = zip.by_index(i).unwrap();
    let name = entry.name().unwrap().to_string();
    let mut data = Vec::new();
    entry.read_to_end(&mut data).unwrap();
    if name == "OEBPS/opf.opf" {
      let opf = String::from_utf8(data)
        .unwrap()
        .replacen(
          "<manifest>",
          "<manifest><item id=\"nis-cover\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>",
          1,
        );
      let spine = opf.find("<spine").unwrap();
      let close = spine + opf[spine..].find('>').unwrap() + 1;
      data = format!(
        "{}<itemref idref=\"nis-cover\"/>{}",
        &opf[..close],
        &opf[close..]
      )
      .into_bytes();
    }
    out.start_file(name, options).unwrap();
    out.write_all(&data).unwrap();
  }
  out.start_file("OEBPS/cover.xhtml", options).unwrap();
  out
    .write_all(b"<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body><img src=\"c.jpg\" alt=\"\"/></body></html>")
    .unwrap();
  out.finish().unwrap();
}

#[test]
fn a_differently_converted_epub_on_the_tolino() {
  let s = Setup::new(
    "map-tolino-convert",
    "tolino-shinecolor-5.18-placeholder-books.sql",
    TOLINO_VERSION,
  );
  let device_file = s.root.join("Books/book-a.epub");
  with_cover_page(&fixtures().join("epub/book-a.epub"), &device_file);
  assert_eq!(
    Epub::open(&device_file).unwrap().spine[82],
    "OEBPS/xhtml/xhtml-0-81.xhtml"
  );
  s.map("book-a.epub", &fixtures().join("epub/book-a.epub"));

  // push: the device's point in xhtml-0-81 (spine item 82 there) is
  // DocFragment[82] in the original.
  let (output, out) = s.run(&["push", "--book", "book-a.epub", "--force"]);
  assert!(output.status.success(), "{out}");
  let sent = put(&s.server);
  assert_eq!(sent[0]["document"], epub_hash());
  assert_eq!(sent[0]["progress"], EPUB_HEADING);

  // pull --write: KOReader's position in the original goes to the same
  // chapter file, one spine item later in the device file.
  s.server.store(&epub_hash(), AHEAD, 0.8, 1_900_000_000);
  let (output, out) = s.run(&["pull", "--book", "book-a.epub", "--write"]);
  assert!(output.status.success(), "{out}");
  let expected = from_xpointer(
    &mut Epub::open(&fixtures().join("epub/book-a.epub")).unwrap(),
    AHEAD,
    PointFlavor::AllNodes,
  )
  .unwrap()
  .position
  .to_string();
  assert_eq!(expected, "OEBPS/xhtml/xhtml-0-89.xhtml#point(/1/2/1/1:0)");
  assert_eq!(s.stored("file:///mnt/onboard/Books/book-a.epub"), expected);
}

#[test]
fn pull_write_puts_a_span_into_a_mapped_kepub() {
  let s = Setup::new(
    "map-kepub-pull",
    "kobo-clarabw-4.46-placeholder-books.sql",
    KOBO_6_VERSION,
  );
  let kepub = "file:///mnt/onboard/Books/book-a.kepub.epub";
  // The kepub's row now holds Book R as kepub, mapped to Book R as EPUB.
  fs::copy(
    fixtures().join("epub/book-r.kepub.epub"),
    s.root.join("Books/book-a.kepub.epub"),
  )
  .unwrap();
  let original = fixtures().join("epub/book-r.epub");
  s.map("book-a.kepub.epub", &original);
  // KOReader in R2-63 at "schließen", after an umlaut, so characters, not
  // bytes.
  let r_hash = partial_md5(&original).unwrap();
  s.server.store(
    &r_hash,
    "/body[1]/DocFragment[3]/body[1]/p[63]/text()[1].13",
    0.8,
    1_900_000_000,
  );
  let (output, out) = s.run(&["pull", "--book", "book-a.kepub", "--write", "--force"]);
  assert!(output.status.success(), "{out}");
  // p[63] is span kobo.64.1 in the kepub (the heading is kobo.1.1).
  assert_eq!(s.stored(kepub), "OEBPS/text/chapter2.xhtml#kobo.64.1:13");
  let percent: i64 = Connection::open(s.root.join(".kobo/KoboReader.sqlite"))
    .unwrap()
    .query_row(
      "SELECT ___PercentRead FROM content WHERE ContentID = ?1",
      [kepub],
      |r| r.get(0),
    )
    .unwrap();
  assert_eq!(percent, 80);
  // After the pull both sides agree.
  let (_, out) = s.run(&["status", "--book", "book-a.kepub"]);
  assert!(out.contains("in sync"), "{out}");
}

/// Kobo firmware 4.x stores the span only: the place within it is lost,
/// and after the pull both sides still agree.
#[test]
fn pull_write_puts_a_span_without_offset_into_a_mapped_kepub_on_kobo_4() {
  let s = Setup::new(
    "map-kepub-pull-4",
    "kobo-clarabw-4.46-placeholder-books.sql",
    KOBO_VERSION,
  );
  let kepub = "file:///mnt/onboard/Books/book-a.kepub.epub";
  fs::copy(
    fixtures().join("epub/book-r.kepub.epub"),
    s.root.join("Books/book-a.kepub.epub"),
  )
  .unwrap();
  let original = fixtures().join("epub/book-r.epub");
  s.map("book-a.kepub.epub", &original);
  let r_hash = partial_md5(&original).unwrap();
  s.server.store(
    &r_hash,
    "/body[1]/DocFragment[3]/body[1]/p[63]/text()[1].13",
    0.8,
    1_900_000_000,
  );
  let (output, out) = s.run(&["pull", "--book", "book-a.kepub", "--write", "--force"]);
  assert!(output.status.success(), "{out}");
  assert_eq!(s.stored(kepub), "OEBPS/text/chapter2.xhtml#kobo.64.1");
  let (_, out) = s.run(&["status", "--book", "book-a.kepub"]);
  assert!(out.contains("in sync"), "{out}");
}

#[test]
fn an_unmapped_kepub_is_never_synced() {
  let s = Setup::new(
    "kepub-unmapped",
    "kobo-clarabw-4.46-placeholder-books.sql",
    KOBO_6_VERSION,
  );
  let kepub_hash = partial_md5(&s.root.join("Books/book-a.kepub.epub")).unwrap();
  s.server.store(&kepub_hash, AHEAD, 0.8, 1_900_000_000);
  let before = s.stored("file:///mnt/onboard/Books/book-a.kepub.epub");
  for args in [
    &["push", "--book", "book-a.kepub", "--force"][..],
    &["pull", "--book", "book-a.kepub", "--write", "--force"],
  ] {
    let (output, out) = s.run(args);
    assert!(output.status.success(), "{out}");
    assert!(
      out.contains("not mapped: a kepub syncs only under its original EPUB"),
      "{out}"
    );
  }
  assert!(s.server.puts().is_empty());
  assert_eq!(
    s.stored("file:///mnt/onboard/Books/book-a.kepub.epub"),
    before
  );
}

#[test]
fn push_sends_the_paragraph_start_unless_set_otherwise() {
  let s = Setup::new(
    "push-paragraph",
    "tolino-shinecolor-5.18-placeholder-books.sql",
    TOLINO_VERSION,
  );
  // book-c.epub becomes Book R, read up to byte 204 (character 195) of
  // R2-16.
  fs::copy(
    fixtures().join("epub/book-r.epub"),
    s.root.join("Books/book-c.epub"),
  )
  .unwrap();
  Connection::open(s.root.join(".kobo/KoboReader.sqlite"))
    .unwrap()
    .execute(
      "UPDATE content SET ChapterIDBookmarked = ?1 WHERE ContentID = ?2",
      [
        "OEBPS/text/chapter2.xhtml#point(/1/4/34/1:204)",
        "file:///mnt/onboard/Books/book-c.epub",
      ],
    )
    .unwrap();
  let r = partial_md5(&fixtures().join("epub/book-r.epub")).unwrap();
  let (output, out) = s.run(&["push", "--book", "book-c", "--force"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("characters to the paragraph start"), "{out}");
  let sent = put(&s.server);
  assert_eq!(sent[0]["document"], r);
  assert_eq!(
    sent[0]["progress"],
    "/body[1]/DocFragment[3]/body[1]/p[16]/text()[1].0"
  );

  let mut config = fs::read_to_string(&s.config).unwrap();
  config.push_str("position = exact\n");
  fs::write(&s.config, config).unwrap();
  let (_, out) = s.run(&["status", "--book", "book-c"]);
  assert!(
    out.contains("  koreader  /body[1]/DocFragment[3]/body[1]/p[16]/text()[1].195\n"),
    "{out}"
  );
}

#[test]
fn a_configuration_without_a_server() {
  let s = Setup::new(
    "config-local-only",
    "tolino-shinecolor-5.18-placeholder-books.sql",
    TOLINO_VERSION,
  );
  fs::write(&s.config, "position = sentence\n").unwrap();
  let (output, out) = s.run(&["status", "--book", "book-a.epub"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("Server not configured"), "{out}");
  let (output, _) = s.run(&["push"]);
  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("no server set up in"), "{stderr}");
}
