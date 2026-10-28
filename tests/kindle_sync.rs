//! `status`, `push` and `pull` on a Kindle (the Kindle fixtures where
//! Send-to-Kindle puts them, or a flat copy of a book's files) against the
//! local mock server: a KFX syncs only through its mapping, under the
//! EPUB's checksum, and nothing comes back.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fake_kindle, fixtures, MockServer};
use nickelinksync::hash::partial_md5;
use nickelinksync::mapping::{DeviceBook, Mapping, Original, Store};
use serde_json::Value;

/// Book R's `lpr` on the Kindle: "Stuhl Garten Seite" in R3-85.
const R3_85: &str = "/body[1]/DocFragment[4]/body[1]/p[85]/text()[1].187";

struct Setup {
  root: PathBuf,
  data: PathBuf,
  server: MockServer,
  config: PathBuf,
}

impl Setup {
  fn new(name: &str) -> Self {
    let root = fake_kindle(name);
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

  fn run_on(&self, device: &Path, args: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
      .args(args)
      .arg("--device")
      .arg(device)
      .arg("--config")
      .arg(&self.config)
      .env("NICKELINKSYNC_DATA_DIR", &self.data)
      .output()
      .unwrap();
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    (output, out)
  }

  fn run(&self, args: &[&str]) -> (Output, String) {
    self.run_on(&self.root, args)
  }

  fn map_book_r(&self) {
    let epub = fixtures().join("epub/book-r.epub");
    let mut store = Store::load(&self.data.join("mappings.json")).unwrap();
    store.insert(Mapping {
      device: DeviceBook::Kindle {
        content_id: "00000000000000000000000000000003".into(),
        title: Some("book-r".into()),
        author: None,
      },
      epub: Original {
        path: epub.to_str().unwrap().to_string(),
        hash: partial_md5(&epub).unwrap(),
        title: None,
        author: None,
      },
      created: "2026-01-01T12:00:00Z".into(),
    });
    store.save().unwrap();
  }

  fn position(&self, setting: &str) {
    let mut config = fs::read_to_string(&self.config).unwrap();
    config.push_str(&format!("position = {setting}\n"));
    fs::write(&self.config, config).unwrap();
  }
}

fn sent(server: &MockServer) -> Vec<Value> {
  server
    .puts()
    .iter()
    .map(|r| serde_json::from_str(&r.body).unwrap())
    .collect()
}

#[test]
fn push_sends_a_mapped_kfx_under_the_epub_checksum() {
  let s = Setup::new("kindle-push");
  s.map_book_r();
  s.position("exact");
  let (output, out) = s.run(&["push"]);
  assert!(output.status.success(), "{out}");
  // Book P and Book Q are not mapped.
  assert_eq!(out.matches("skipped   not mapped").count(), 2, "{out}");
  let sent = sent(&s.server);
  assert_eq!(sent.len(), 1, "{out}");
  assert_eq!(
    sent[0]["document"],
    partial_md5(&fixtures().join("epub/book-r.epub")).unwrap()
  );
  assert_eq!(sent[0]["progress"], R3_85);
  assert_eq!(sent[0]["device"], "Kindle");
  // pid 126677 of the book's positions: paragraph 485 of 600.
  let percentage = sent[0]["percentage"].as_f64().unwrap();
  assert!((0.78..0.84).contains(&percentage), "{percentage}");

  let (_, out) = s.run(&["status", "--book", "book-r"]);
  assert!(out.contains(&format!("  koreader  {R3_85}\n")), "{out}");
  assert!(
    out.contains("block 513, offset 187 (position 126677 of"),
    "{out}"
  );
  assert!(out.contains("  sync      in sync"), "{out}");
}

#[test]
fn push_sends_the_paragraph_start_by_default() {
  let s = Setup::new("kindle-paragraph");
  s.map_book_r();
  let (output, out) = s.run(&["push", "--book", "book-r"]);
  assert!(output.status.success(), "{out}");
  assert!(out.contains("characters to the paragraph start"), "{out}");
  assert_eq!(
    sent(&s.server)[0]["progress"],
    "/body[1]/DocFragment[4]/body[1]/p[85]/text()[1].0"
  );
}

#[test]
fn a_flat_copy_of_the_books_works_too() {
  // As OpenMTP copies a book: the KFX and its .sdr folder, no documents/.
  let s = Setup::new("kindle-flat");
  s.map_book_r();
  s.position("exact");
  let items = s.root.join("documents/Downloads/Items01");
  let flat = s.root.join("copy");
  fs::create_dir_all(&flat).unwrap();
  let name = "book-r_00000000000000000000000000000003";
  fs::rename(
    items.join(format!("{name}.kfx")),
    flat.join(format!("{name}.kfx")),
  )
  .unwrap();
  fs::rename(
    items.join(format!("{name}.sdr")),
    flat.join(format!("{name}.sdr")),
  )
  .unwrap();
  let (output, out) = s.run_on(&flat, &["status"]);
  assert!(output.status.success(), "{out}");
  assert!(out.starts_with("Kindle, 1 KFX book(s) in "), "{out}");
  assert!(out.contains(&format!("  koreader  {R3_85}\n")), "{out}");
}

#[test]
fn pull_is_refused_and_nothing_is_written() {
  let s = Setup::new("kindle-pull");
  s.map_book_r();
  let before: Vec<_> = fs::read_dir(s.root.join("documents/Downloads/Items01"))
    .unwrap()
    .map(|e| e.unwrap().file_name())
    .collect();
  let (output, _) = s.run(&["pull", "--write"]);
  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("a Kindle is only read"), "{stderr}");
  let after: Vec<_> = fs::read_dir(s.root.join("documents/Downloads/Items01"))
    .unwrap()
    .map(|e| e.unwrap().file_name())
    .collect();
  assert_eq!(before, after);
}
