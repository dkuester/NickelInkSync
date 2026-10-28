//! The library of original EPUBs and `nickelinksync map suggest`: a Calibre
//! library whose `metadata.db` the test makes itself (no real library in
//! the repository), a plain folder of EPUBs, and suggestions for the books
//! on fake devices built from the fixtures.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use common::{fake_device, fake_kindle, fixtures};
use nickelinksync::library::{Kind, Library};
use rusqlite::Connection;

fn temp(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("nickelinksync-{}-{name}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  dir
}

/// A Calibre library with Book A and Book R, as Calibre lays it out
/// (`Author/Title (id)/Title - Author.epub`), plus a book whose EPUB is
/// missing. Only the tables and columns NickelInkSync reads.
fn calibre_library(name: &str) -> PathBuf {
  let root = temp(name);
  let db = Connection::open(root.join("metadata.db")).unwrap();
  db.execute_batch(
    "CREATE TABLE books (id INTEGER PRIMARY KEY, title TEXT NOT NULL DEFAULT 'Unknown',
       sort TEXT, path TEXT NOT NULL DEFAULT '');
     CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT NOT NULL, sort TEXT);
     CREATE TABLE books_authors_link (id INTEGER PRIMARY KEY, book INTEGER NOT NULL,
       author INTEGER NOT NULL);
     CREATE TABLE data (id INTEGER PRIMARY KEY, book INTEGER NOT NULL, format TEXT NOT NULL,
       uncompressed_size INTEGER NOT NULL, name TEXT NOT NULL);
     INSERT INTO authors VALUES (1, 'Author A', 'A, Author'), (2, 'Author R', 'R, Author');
     INSERT INTO books VALUES (1, 'Book A', 'Book A', 'Author A/Book A (1)'),
       (2, 'Book R', 'Book R', 'Author R/Book R (2)'),
       (3, 'Book X', 'Book X', 'Author A/Book X (3)');
     INSERT INTO books_authors_link VALUES (1, 1, 1), (2, 2, 2), (3, 3, 1);
     INSERT INTO data VALUES (1, 1, 'EPUB', 0, 'Book A - Author A'),
       (2, 1, 'KEPUB', 0, 'Book A - Author A'),
       (3, 2, 'EPUB', 0, 'Book R - Author R'),
       (4, 3, 'EPUB', 0, 'Book X - Author A');",
  )
  .unwrap();
  drop(db);
  for (folder, file, fixture) in [
    (
      "Author A/Book A (1)",
      "Book A - Author A.epub",
      "book-a.epub",
    ),
    (
      "Author R/Book R (2)",
      "Book R - Author R.epub",
      "book-r.epub",
    ),
  ] {
    fs::create_dir_all(root.join(folder)).unwrap();
    fs::copy(
      fixtures().join("epub").join(fixture),
      root.join(folder).join(file),
    )
    .unwrap();
  }
  root
}

/// Every file below `root` with its size and modification time.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u64, SystemTime)> {
  let mut files = BTreeMap::new();
  let mut dirs = vec![root.to_path_buf()];
  while let Some(dir) = dirs.pop() {
    for entry in fs::read_dir(&dir).unwrap() {
      let entry = entry.unwrap();
      let meta = entry.metadata().unwrap();
      if meta.is_dir() {
        dirs.push(entry.path());
      } else {
        files.insert(entry.path(), (meta.len(), meta.modified().unwrap()));
      }
    }
  }
  files
}

fn run(data: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
    .args(args)
    .env("NICKELINKSYNC_DATA_DIR", data)
    .env("NICKELINKSYNC_CONFIG", data.join("no-config.ini"))
    .output()
    .expect("failed to run nickelinksync")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn reads_a_calibre_library_without_writing() {
  let root = calibre_library("calibre-read");
  let before = snapshot(&root);
  let library = Library::open(&root).unwrap();
  assert_eq!(snapshot(&root), before, "the library was changed");
  assert_eq!(library.kind, Kind::Calibre);
  let books: Vec<_> = library
    .books
    .iter()
    .map(|b| (b.calibre_id, b.title.as_deref(), b.authors.clone()))
    .collect();
  assert_eq!(
    books,
    [
      (Some(1), Some("Book A"), vec!["Author A".to_string()]),
      (Some(2), Some("Book R"), vec!["Author R".to_string()]),
    ]
  );
  assert_eq!(
    library.books[0].path,
    root.join("Author A/Book A (1)/Book A - Author A.epub")
  );
  // Book X is listed in metadata.db, but its file is missing.
  assert_eq!(library.skipped.len(), 1);
  assert!(library.skipped[0].0.ends_with("Book X - Author A.epub"));
}

#[test]
fn reads_a_folder_of_epubs() {
  let root = temp("folder-read");
  fs::create_dir_all(root.join("a/z")).unwrap();
  for (fixture, to) in [
    ("book-a.epub", "a/book-a.epub"),
    ("book-a.kepub.epub", "a/book-a.kepub.epub"),
    ("book-r.epub", "a/z/book-r.epub"),
  ] {
    fs::copy(fixtures().join("epub").join(fixture), root.join(to)).unwrap();
  }
  fs::write(root.join("a/broken.epub"), b"not a zip").unwrap();
  fs::write(root.join("a/._book-a.epub"), b"AppleDouble").unwrap();
  let before = snapshot(&root);
  let library = Library::open(&root).unwrap();
  assert_eq!(snapshot(&root), before);
  assert_eq!(library.kind, Kind::Folder);
  let books: Vec<_> = library
    .books
    .iter()
    .map(|b| (b.title.as_deref(), b.authors.clone()))
    .collect();
  // kepubs are not originals.
  assert_eq!(
    books,
    [
      (Some("Book A"), vec!["Author A".to_string()]),
      (Some("Book R"), vec!["Author R".to_string()]),
    ]
  );
  assert_eq!(library.skipped.len(), 1);
}

#[test]
fn suggests_for_a_kobo() {
  let library = calibre_library("calibre-kobo");
  let root = fake_device(
    "suggest-kobo",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = temp("suggest-kobo-data");
  let before = snapshot(&library);
  let out = run(
    &data,
    &[
      "map",
      "suggest",
      "--device",
      root.to_str().unwrap(),
      "--library",
      library.to_str().unwrap(),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  assert_eq!(snapshot(&library), before, "the library was changed");
  let text = stdout(&out);
  assert!(text.contains("(Calibre library, 2 EPUBs)"), "{text}");
  // The kepub: Book A in the library, with every chapter.
  let kepub = &text[text.find("Books/book-a.kepub.epub").unwrap()..];
  let kepub = &kepub[..kepub.find("\n\n").unwrap()];
  assert!(kepub.contains("Book A - Author A.epub"), "{kepub}");
  assert!(
    kepub.contains("score 100 %: title 100 %, author 100 %, chapters 109/109"),
    "{kepub}"
  );
  assert!(kepub.contains("map add --device '"), "{kepub}");
  assert!(
    kepub.contains("--content-id 'file:///mnt/onboard/Books/book-a.kepub.epub'"),
    "{kepub}"
  );
  // The EPUB is the very file in the library.
  assert!(
    text.contains("same file ") && text.contains("no mapping needed"),
    "{text}"
  );
  // Book C is not in the library: Book A and Book R share only words like
  // "Book" with it, too little to be shown.
  assert!(
    text.contains(
      "3 book(s): 0 mapped, 1 the same file as in the library, 1 not mapped with a \
       suggestion, 1 not mapped without one."
    ),
    "{text}"
  );
  assert!(
    text.contains("not mapped: no candidate in the library"),
    "{text}"
  );
  assert!(
    !data.join("mappings.json").exists(),
    "suggest stored something"
  );
}

#[test]
fn suggests_for_a_kindle_and_shows_mappings() {
  let library = temp("folder-kindle");
  for book in ["book-p", "book-q", "book-r"] {
    fs::copy(
      fixtures().join("epub").join(format!("{book}.epub")),
      library.join(format!("{book}.epub")),
    )
    .unwrap();
  }
  let root = fake_kindle("suggest-kindle");
  let data = temp("suggest-kindle-data");
  let args = |extra: &[&str]| {
    let mut args = vec![
      "map",
      "suggest",
      "--device",
      root.to_str().unwrap(),
      "--library",
      library.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&data, &args)
  };
  let text = stdout(&args(&["--book", "book-r"]));
  assert!(
    text.contains("Kindle, content_id 00000000000000000000000000000003"),
    "{text}"
  );
  assert!(text.contains("book-r.epub"), "{text}");
  assert!(
    text.contains("score 100 %: title 100 %, author 100 %, chapters 5/5"),
    "{text}"
  );

  // Once mapped, suggest says so.
  let out = run(
    &data,
    &[
      "map",
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "book-r",
      "--epub",
      library.join("book-r.epub").to_str().unwrap(),
    ],
  );
  assert!(out.status.success(), "{out:?}");
  let text = stdout(&args(&[]));
  assert!(text.contains("mapped    "), "{text}");
  assert!(text.contains("3 book(s): 1 mapped"), "{text}");
}

#[test]
fn add_without_epub_needs_a_terminal_to_confirm() {
  let library = calibre_library("calibre-add");
  let root = fake_device(
    "add-confirm",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = temp("add-confirm-data");
  let add = |library: Option<&Path>| {
    let mut args = vec![
      "map",
      "add",
      "--device",
      root.to_str().unwrap(),
      "--book",
      "kepub",
    ];
    if let Some(library) = library {
      args.extend(["--library", library.to_str().unwrap()]);
    }
    run(&data, &args)
  };
  let out = add(Some(&library));
  assert!(!out.status.success());
  assert!(stdout(&out).contains("Book A - Author A.epub"));
  let stderr = String::from_utf8_lossy(&out.stderr);
  assert!(stderr.contains("no terminal to confirm"), "{stderr}");
  assert!(!data.join("mappings.json").exists());

  let out = add(None);
  assert!(!out.status.success());
  assert!(String::from_utf8_lossy(&out.stderr).contains("give --epub"));
}

#[test]
fn library_from_the_configuration_file() {
  let library = calibre_library("calibre-config");
  let root = fake_device(
    "suggest-config",
    "kobo-clarabw-4.46-placeholder-books.sql",
    common::KOBO_VERSION,
  );
  let data = temp("suggest-config-data");
  // Only `library`: the mapping needs no server.
  let config = data.join("config.ini");
  fs::write(&config, format!("library = {}\n", library.display())).unwrap();
  let out = run(
    &data,
    &[
      "map",
      "suggest",
      "--device",
      root.to_str().unwrap(),
      "--config",
      config.to_str().unwrap(),
      "--book",
      "kepub",
    ],
  );
  assert!(out.status.success(), "{out:?}");
  assert!(stdout(&out).contains("Calibre library"));

  let out = run(
    &data,
    &["map", "suggest", "--device", root.to_str().unwrap()],
  );
  assert!(!out.status.success());
  assert!(String::from_utf8_lossy(&out.stderr).contains("no library"));
}
