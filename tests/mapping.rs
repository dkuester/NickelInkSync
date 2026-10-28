//! `nickelinksync map`: mappings between books on a device and their
//! original EPUB, on fake devices built from the fixtures and a data
//! directory of their own.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fake_device, fixtures};
use nickelinksync::hash::partial_md5;
use nickelinksync::mapping::{DeviceBook, EpubState, Store};

/// A fresh, empty data directory.
fn data_dir(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("nickelinksync-{}-data-{name}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  dir
}

/// A fake Kindle: the Kindle fixtures where Send-to-Kindle puts them.
fn fake_kindle(name: &str) -> PathBuf {
  let root = std::env::temp_dir().join(format!("nickelinksync-{}-{name}", std::process::id()));
  let _ = fs::remove_dir_all(&root);
  let items = root.join("documents/Downloads/Items01");
  fs::create_dir_all(&items).unwrap();
  fs::create_dir_all(root.join("system")).unwrap();
  for entry in fs::read_dir(fixtures().join("kindle")).unwrap() {
    let entry = entry.unwrap();
    let to = items.join(entry.file_name());
    if entry.file_type().unwrap().is_dir() {
      fs::create_dir_all(&to).unwrap();
      for file in fs::read_dir(entry.path()).unwrap() {
        let file = file.unwrap();
        fs::copy(file.path(), to.join(file.file_name())).unwrap();
      }
    } else {
      fs::copy(entry.path(), to).unwrap();
    }
  }
  root
}

fn map(data: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
    .arg("map")
    .args(args)
    .env("NICKELINKSYNC_DATA_DIR", data)
    .output()
    .expect("failed to run nickelinksync")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn epub(name: &str) -> String {
  fixtures()
    .join("epub")
    .join(name)
    .to_string_lossy()
    .into_owned()
}

fn store(data: &Path) -> Store {
  Store::load(&data.join("mappings.json")).unwrap()
}

#[test]
fn maps_a_kepub_to_its_epub() {
  let root = fake_device(
    "map-kobo",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = data_dir("kobo");
  let device = root.to_str().unwrap();
  let out = map(
    &data,
    &[
      "add",
      "--device",
      device,
      "--book",
      "kepub",
      "--epub",
      &epub("book-a.epub"),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  let text = stdout(&out);
  assert!(text.contains("all 109 spine items match"), "{text}");
  assert!(text.contains("Mapped."), "{text}");

  let store = store(&data);
  assert_eq!(store.mappings.len(), 1);
  let m = &store.mappings[0];
  let kepub_hash = partial_md5(&root.join("Books/book-a.kepub.epub")).unwrap();
  assert_eq!(
    m.device,
    DeviceBook::File {
      hash: kepub_hash.clone(),
      path: "Books/book-a.kepub.epub".into(),
      title: Some("Book A".into()),
      author: Some("Author A".into()),
    }
  );
  let epub_path = fixtures().join("epub/book-a.epub").canonicalize().unwrap();
  assert_eq!(m.epub.path, epub_path.to_str().unwrap());
  assert_eq!(m.epub.hash, partial_md5(&epub_path).unwrap());
  assert_eq!(m.epub.title.as_deref(), Some("Book A"));
  assert_eq!(m.check(), EpubState::Ok);
  assert_eq!(store.for_file(&kepub_hash), Some(m));

  // The same again replaces it.
  let again = map(
    &data,
    &[
      "add",
      "--device",
      device,
      "--book",
      "kepub",
      "--epub",
      &epub("book-a.epub"),
    ],
  );
  assert!(stdout(&again).contains("this replaces the mapping"));
  assert_eq!(self::store(&data).mappings.len(), 1);

  let list = stdout(&map(&data, &["list"]));
  assert!(
    list.contains("Books/book-a.kepub.epub (Kobo/Tolino"),
    "{list}"
  );
  assert!(list.contains("state     ok"), "{list}");

  let out = map(&data, &["remove", &kepub_hash[..8]]);
  assert!(out.status.success(), "{out:?}");
  assert!(self::store(&data).mappings.is_empty());
}

#[test]
fn the_same_file_needs_no_mapping() {
  let root = fake_device(
    "map-same",
    "tolino-shinecolor-5.18-placeholder-books.sql",
    common::TOLINO_VERSION,
  );
  let data = data_dir("same");
  let out = map(
    &data,
    &[
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "book-a.epub",
      "--epub",
      &epub("book-a.epub"),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  assert!(
    stdout(&out).contains("no mapping needed"),
    "{}",
    stdout(&out)
  );
  assert!(!data.join("mappings.json").exists());
}

#[test]
fn several_matches_are_listed_not_guessed() {
  let root = fake_device(
    "map-many",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = data_dir("many");
  let out = map(
    &data,
    &[
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "book-a",
      "--epub",
      &epub("book-a.epub"),
    ],
  );
  assert!(!out.status.success());
  let stderr = String::from_utf8_lossy(&out.stderr);
  assert!(stderr.contains("2 books match"), "{stderr}");
  assert!(!data.join("mappings.json").exists());
}

#[test]
fn maps_a_kindle_book_by_content_id() {
  let root = fake_kindle("map-kindle");
  let data = data_dir("kindle");
  let out = map(
    &data,
    &[
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "book-r",
      "--epub",
      &epub("book-r.epub"),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  assert!(
    stdout(&out).contains("as many as spine items"),
    "{}",
    stdout(&out)
  );
  let store = store(&data);
  assert_eq!(
    store.mappings[0].device,
    DeviceBook::Kindle {
      content_id: "00000000000000000000000000000003".into(),
      title: Some("book-r".into()),
      author: Some("Author R".into()),
    }
  );
  assert!(store
    .for_kindle("00000000000000000000000000000003")
    .is_some());

  // By content_id as well.
  let out = map(
    &data,
    &[
      "add",
      "--device",
      root.to_str().unwrap(),
      "--content-id",
      "00000000000000000000000000000001",
      "--epub",
      &epub("book-p.epub"),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  assert_eq!(self::store(&data).mappings.len(), 2);
}

#[test]
fn list_shows_broken_mappings() {
  let root = fake_device(
    "map-broken",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = data_dir("broken");
  // An EPUB of our own, so it can change and go away.
  let library = data.with_extension("library");
  fs::create_dir_all(&library).unwrap();
  let original = library.join("book-a.epub");
  fs::copy(fixtures().join("epub/book-a.epub"), &original).unwrap();
  let out = map(
    &data,
    &[
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "kepub",
      "--epub",
      original.to_str().unwrap(),
    ],
  );
  assert!(out.status.success(), "{out:?}");

  // Another file under the same name.
  fs::copy(fixtures().join("epub/book-c.epub"), &original).unwrap();
  let list = stdout(&map(&data, &["list"]));
  assert!(list.contains("broken: the EPUB has changed"), "{list}");
  assert!(list.contains("1 broken mapping(s)"), "{list}");

  fs::remove_file(&original).unwrap();
  let list = stdout(&map(&data, &["list"]));
  assert!(list.contains("broken: the EPUB is missing"), "{list}");
  fs::remove_dir_all(&library).unwrap();
}

#[test]
fn remove_needs_a_known_key() {
  let data = data_dir("remove");
  let out = map(&data, &["remove", "abcdef"]);
  assert!(!out.status.success());
  assert!(String::from_utf8_lossy(&out.stderr).contains("no mapping for `abcdef`"));
  let list = stdout(&map(&data, &["list"]));
  assert!(list.contains("No mappings yet"), "{list}");
}
