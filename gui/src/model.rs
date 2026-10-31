//! What the window shows of a device, read with the library's
//! [`Session`]/[`KindleSession`]: the books, grouped by how they sync, with
//! the position on the device; and making or removing a book's mapping
//! (in the data directory, never on the device). The server: `sync`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use nickelinksync::library::{self, Library};
use nickelinksync::map::{self, OnDevice, Outcome};
use nickelinksync::mapping::Store;
use nickelinksync::session::{Identity, KindleSession, Options, Own, Session};
use nickelinksync::sync::{format_timestamp, parse_timestamp};

use crate::devices::Kind;

/// How a book syncs, if at all.
#[derive(Debug, Clone, PartialEq)]
pub enum Group {
  /// Under its own checksum, checked: the same file as in the library, or
  /// the original of a mapping.
  Same { why: Own },
  /// Under its own checksum, unchecked: no library to find its reference
  /// EPUB (the file KOReader opens) and no mapping to one.
  NoReference,
  /// Under the checksum of this original EPUB.
  Mapped { original: String },
  /// Neither mapped nor the same file as in the library.
  NotMapped,
  /// Broken mapping or unreadable file.
  Problem { message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
  /// What selects the book for a mapping: ContentID (Kobo, Tolino) or
  /// `content_id` (Kindle); empty for an unreadable file.
  pub id: String,
  /// The key of the book's mapping, if it has one (also a broken one).
  pub key: Option<String>,
  /// The KOSync document it syncs under; `None` if it does not sync.
  pub document: Option<String>,
  pub title: String,
  pub author: Option<String>,
  /// Path on the device (or in the Kindle copy).
  pub file: String,
  pub group: Group,
  /// 0.0 to 1.0; `None` if never opened.
  pub percent: Option<f64>,
  /// KOReader's XPointer for the position, or why there is none.
  pub xpointer: Result<String, String>,
  pub last_read: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
  /// "Tolino Shine Color, firmware 5.18.264769" or "Kindle".
  pub device: String,
  /// The library of originals: path, Calibre or folder, number of EPUBs.
  pub library: Option<(PathBuf, library::Kind, usize)>,
  pub mappings: usize,
  pub rows: Vec<Row>,
  /// The server's positions can be written into it: a Tolino (on a Kobo
  /// not yet, P2; a Kindle is only read).
  pub writable: bool,
}

pub(crate) fn options(root: &Path) -> Options {
  Options {
    device: root.to_path_buf(),
    config: None,
    book: None,
    content_id: None,
    live: false,
    timeout: Duration::from_secs(30),
    library: None,
  }
}

/// Reads the device at `root`; `progress(done, total)` after each book.
pub fn load(
  root: &Path,
  kind: Kind,
  progress: &mut dyn FnMut(usize, usize),
) -> Result<Loaded, String> {
  match kind {
    Kind::Nickel => load_nickel(&options(root), progress),
    Kind::Kindle => load_kindle(&options(root), progress),
  }
  .map_err(|e| e.to_string())
}

fn load_nickel(
  options: &Options,
  progress: &mut dyn FnMut(usize, usize),
) -> nickelinksync::session::Result<Loaded> {
  let session = Session::open(options)?;
  let device = match &session.device.firmware {
    Some(fw) => format!("{}, firmware {}", session.model, fw.version),
    None => session.model.clone(),
  };
  let total = session.books.len();
  let mut rows = Vec::with_capacity(total);
  for (i, book) in session.books.iter().enumerate() {
    let identity = session.identity(book);
    let group = match &identity {
      // Without a library, the original of a mapping is still known.
      Identity::Own {
        why: Own::NoLibrary,
        hash,
      } if session.store.mappings.iter().any(|m| &m.epub.hash == hash) => Group::Same {
        why: Own::OriginalOfMapping,
      },
      Identity::Own {
        why: Own::NoLibrary,
        ..
      } => Group::NoReference,
      Identity::Own { why, .. } => Group::Same { why: *why },
      Identity::Mapped { mapping, .. } => Group::Mapped {
        original: mapping.epub.path.clone(),
      },
      Identity::Skip {
        kind: "not-mapped", ..
      } => Group::NotMapped,
      Identity::Skip { message, .. } => Group::Problem {
        message: message.clone(),
      },
    };
    let key = match &identity {
      Identity::Mapped { hash, .. } => Some(hash.clone()),
      Identity::Skip {
        kind: "mapping-broken",
        hash,
        ..
      } => hash.clone(),
      _ => None,
    };
    let xpointer = match &identity {
      Identity::Skip { .. } => Err(String::new()),
      _ => session
        .local(book, &identity)
        .map(|(local, _)| local.xpointer),
    };
    rows.push(Row {
      id: book.content_id.clone(),
      key,
      document: identity.document().map(str::to_string),
      title: book
        .title
        .clone()
        .unwrap_or_else(|| book.onboard_path().to_string()),
      author: book.author.clone(),
      file: book.onboard_path().to_string(),
      group,
      percent: book
        .position
        .as_ref()
        .map(|_| book.percent_read as f64 / 100.0),
      xpointer,
      last_read: book
        .last_read
        .as_deref()
        .and_then(parse_timestamp)
        .map(format_timestamp),
    });
    progress(i + 1, total);
  }
  Ok(Loaded {
    device,
    library: session
      .library
      .as_ref()
      .map(|l| (l.root.clone(), l.kind, l.books.len())),
    mappings: session.store.mappings.len(),
    rows,
    writable: session.family == "Tolino",
  })
}

fn load_kindle(
  options: &Options,
  progress: &mut dyn FnMut(usize, usize),
) -> nickelinksync::session::Result<Loaded> {
  let session = KindleSession::open(options)?;
  let total = session.books.len();
  let mut rows = Vec::with_capacity(total);
  for (i, book) in session.books.iter().enumerate() {
    let (group, local, document) = match session.mapping(book) {
      Ok(mapping) => (
        Group::Mapped {
          original: mapping.epub.path.clone(),
        },
        Some(session.local(book, mapping)),
        Some(mapping.epub.hash.clone()),
      ),
      Err(_) if session.store.for_kindle(&book.content_id).is_none() => {
        (Group::NotMapped, None, None)
      }
      Err(message) => (Group::Problem { message }, None, None),
    };
    let (percent, xpointer, last_read) = match local {
      Some(Ok((local, _, _))) => (
        Some(local.percentage),
        Ok(local.xpointer),
        local.last_read.map(format_timestamp),
      ),
      Some(Err(e)) => (None, Err(e), None),
      None => (None, Err(String::new()), None),
    };
    rows.push(Row {
      id: book.content_id.clone(),
      key: session
        .store
        .for_kindle(&book.content_id)
        .map(|m| m.device.key().to_string()),
      document,
      title: book
        .title
        .clone()
        .unwrap_or_else(|| book.content_id.clone()),
      author: book.author.clone(),
      file: book.relative_path(&session.root).display().to_string(),
      group,
      percent,
      xpointer,
      last_read,
    });
    progress(i + 1, total);
  }
  for (path, e) in &session.unreadable {
    rows.push(Row {
      id: String::new(),
      key: None,
      document: None,
      title: path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default(),
      author: None,
      file: path.display().to_string(),
      group: Group::Problem { message: e.clone() },
      percent: None,
      xpointer: Err(String::new()),
      last_read: None,
    });
  }
  let library = map::open_library(None, None)?;
  Ok(Loaded {
    device: "Kindle".into(),
    library: library.map(|l| (l.root, l.kind, l.books.len())),
    mappings: session.store.mappings.len(),
    rows,
    writable: false,
  })
}

/// One library book offered as the original of a device book.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
  pub path: PathBuf,
  pub title: Option<String>,
  pub authors: Vec<String>,
  pub score: f64,
  pub title_score: f64,
  /// `None` if either side has no author.
  pub author_score: Option<f64>,
  /// Matching chapters and how many there are, if compared.
  pub chapters: Option<(usize, usize)>,
}

/// What the library offers for one device book.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestions {
  /// `None`: no library set up, only choosing a file is possible.
  pub library: Option<PathBuf>,
  /// The library book that is the same file: no mapping needed.
  pub same: Option<PathBuf>,
  /// Best first.
  pub candidates: Vec<Suggestion>,
}

/// The one book with `id` on the device at `root`.
fn on_device(root: &Path, id: &str) -> Result<OnDevice, String> {
  let (mut books, _) = map::device_books(root, None, Some(id)).map_err(|e| e.to_string())?;
  match books.len() {
    1 => Ok(books.remove(0)),
    _ => Err(format!("{id}: not on the device")),
  }
}

/// The library's candidates for the book with `id` on the device at
/// `root`.
pub fn suggest(root: &Path, id: &str, library: Option<&Library>) -> Result<Suggestions, String> {
  let on = on_device(root, id)?;
  let Some(library) = library else {
    return Ok(Suggestions {
      library: None,
      same: None,
      candidates: Vec::new(),
    });
  };
  let (candidates, same) = map::candidates(&on, library);
  Ok(Suggestions {
    library: Some(library.root.clone()),
    same: same.map(|b| b.path.clone()),
    candidates: candidates
      .iter()
      .map(|c| Suggestion {
        path: c.book.path.clone(),
        title: c.book.title.clone(),
        authors: c.book.authors.clone(),
        score: c.score(),
        title_score: c.title,
        author_score: c.author,
        chapters: c.chapters,
      })
      .collect(),
  })
}

/// What [`map_book`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
  /// The device file is the EPUB itself: no mapping needed.
  SameFile,
  /// Mapped; the original's path.
  To(String),
}

/// Maps the book with `id` on the device at `root` to the EPUB at `epub`,
/// in the mappings shared with the CLI.
pub fn map_book(root: &Path, id: &str, epub: &Path) -> Result<Mapped, String> {
  let on = on_device(root, id)?;
  let mut store = store()?;
  let added = map::add(&mut store, &on, epub).map_err(|e| e.to_string())?;
  Ok(match added.outcome {
    Outcome::SameFile { .. } => Mapped::SameFile,
    Outcome::Mapped { .. } => Mapped::To(added.original.path),
  })
}

/// Removes the mapping with `key`.
pub fn unmap(key: &str) -> Result<(), String> {
  let mut store = store()?;
  store.remove(key).map_err(|e| e.to_string())?;
  store.save().map_err(|e| e.to_string())
}

fn store() -> Result<Store, String> {
  let path = Store::default_path().ok_or("no data directory (HOME is not set)")?;
  Store::load(&path).map_err(|e| e.to_string())
}

/// The library set up in the configuration file, if any.
pub fn library() -> Result<Option<Library>, String> {
  map::open_library(None, None).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  use crate::test_data::isolate;

  fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures")
  }

  /// A fake Tolino with the placeholder books, in a fresh directory.
  fn fake_tolino(name: &str) -> PathBuf {
    let root =
      std::env::temp_dir().join(format!("nickelinksync-gui-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".kobo")).unwrap();
    fs::create_dir_all(root.join("Books")).unwrap();
    fs::write(
      root.join(".kobo/version"),
      "N000000000000,4.9.77,5.18.264769,4.9.77,4.9.77,00000000-0000-0000-0000-000000000693\n",
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.join(".kobo/KoboReader.sqlite")).unwrap();
    db.execute_batch(
      &fs::read_to_string(fixtures().join("db/tolino-shinecolor-5.18-placeholder-books.sql"))
        .unwrap(),
    )
    .unwrap();
    drop(db);
    for file in ["book-a.epub", "book-a.kepub.epub", "book-c.epub"] {
      fs::copy(
        fixtures().join("epub").join(file),
        root.join("Books").join(file),
      )
      .unwrap();
    }
    root
  }

  #[test]
  fn tolino_books_without_library_sync_under_their_own_checksum() {
    let _data = isolate();
    let root = fake_tolino("tolino");
    let before = fs::read(root.join(".kobo/KoboReader.sqlite")).unwrap();
    let mut seen = Vec::new();
    let loaded = load(&root, Kind::Nickel, &mut |done, total| {
      seen.push((done, total))
    })
    .unwrap();
    assert_eq!(loaded.device, "Tolino Shine Color, firmware 5.18.264769");
    assert_eq!(loaded.library, None);
    assert_eq!(loaded.rows.len(), 3);
    assert_eq!(seen, [(1, 3), (2, 3), (3, 3)]);
    let a = loaded
      .rows
      .iter()
      .find(|r| r.file == "Books/book-a.epub")
      .unwrap();
    assert_eq!(a.group, Group::NoReference);
    assert_eq!(a.percent, Some(0.74));
    assert_eq!(
      a.xpointer.as_deref(),
      Ok("/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0")
    );
    assert_eq!(a.last_read.as_deref(), Some("2026-01-03 12:00:00 UTC"));
    // Only read.
    assert_eq!(
      fs::read(root.join(".kobo/KoboReader.sqlite")).unwrap(),
      before
    );
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn kindle_books_without_mapping_are_not_mapped() {
    let _data = isolate();
    let loaded = load(&fixtures().join("kindle"), Kind::Kindle, &mut |_, _| {}).unwrap();
    assert_eq!(loaded.device, "Kindle");
    assert_eq!(loaded.rows.len(), 3);
    assert!(loaded.rows.iter().all(|r| r.group == Group::NotMapped));
  }
  #[test]
  fn suggest_map_and_unmap_a_kepub() {
    let _data = isolate();
    let root = fake_tolino("map");
    let library_dir = root.with_extension("library");
    let _ = fs::remove_dir_all(&library_dir);
    fs::create_dir_all(&library_dir).unwrap();
    for file in ["book-a.epub", "book-c.epub"] {
      fs::copy(fixtures().join("epub").join(file), library_dir.join(file)).unwrap();
    }
    let library = Library::open(&library_dir).unwrap();
    let kepub = "file:///mnt/onboard/Books/book-a.kepub.epub";

    // Without a library only choosing a file is offered.
    let none = suggest(&root, kepub, None).unwrap();
    assert_eq!(none.library, None);
    assert!(none.candidates.is_empty());

    let s = suggest(&root, kepub, Some(&library)).unwrap();
    assert_eq!(s.same, None);
    let best = &s.candidates[0];
    assert!(best.path.ends_with("book-a.epub"), "{s:?}");
    assert!(best.score >= nickelinksync::suggest::LIKELY, "{best:?}");
    // The plain EPUB on the device is the library's file itself.
    let same = suggest(
      &root,
      "file:///mnt/onboard/Books/book-a.epub",
      Some(&library),
    )
    .unwrap();
    assert!(same.same.is_some_and(|p| p.ends_with("book-a.epub")));

    let mapped = map_book(&root, kepub, &best.path).unwrap();
    let Mapped::To(original) = mapped else {
      panic!("{mapped:?}")
    };
    assert!(original.ends_with("book-a.epub"));
    let loaded = load(&root, Kind::Nickel, &mut |_, _| {}).unwrap();
    let row = loaded.rows.iter().find(|r| r.id == kepub).unwrap();
    assert_eq!(row.group, Group::Mapped { original });
    let key = row.key.clone().unwrap();
    // The plain EPUB on the device is that original: checked, even without
    // a library.
    let plain = loaded
      .rows
      .iter()
      .find(|r| r.file == "Books/book-a.epub")
      .unwrap();
    assert_eq!(
      plain.group,
      Group::Same {
        why: Own::OriginalOfMapping
      }
    );

    // Mapping the plain EPUB to itself needs no mapping.
    assert_eq!(
      map_book(&root, "file:///mnt/onboard/Books/book-a.epub", &best.path).unwrap(),
      Mapped::SameFile
    );

    unmap(&key).unwrap();
    let loaded = load(&root, Kind::Nickel, &mut |_, _| {}).unwrap();
    let row = loaded.rows.iter().find(|r| r.id == kepub).unwrap();
    assert_eq!(row.group, Group::NoReference);
    assert_eq!(row.key, None);
    assert!(unmap(&key).is_err());
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&library_dir).unwrap();
  }
}
