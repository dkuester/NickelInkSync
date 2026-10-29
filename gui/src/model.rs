//! What the window shows of a device, read with the library's
//! [`Session`]/[`KindleSession`]: the books, grouped by how they sync, with
//! the position on the device. Only reads; the server is not asked yet.

use std::path::{Path, PathBuf};
use std::time::Duration;

use nickelinksync::library;
use nickelinksync::session::{Identity, KindleSession, Options, Session};
use nickelinksync::sync::{format_timestamp, parse_timestamp};

use crate::devices::Kind;

/// How a book syncs, if at all.
#[derive(Debug, Clone, PartialEq)]
pub enum Group {
  /// Under its own checksum; `why` (same file as in the library, no
  /// library, the original of a mapping).
  Same { why: String },
  /// Under the checksum of this original EPUB.
  Mapped { original: String },
  /// Neither mapped nor the same file as in the library.
  NotMapped,
  /// Broken mapping or unreadable file.
  Problem { message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
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
}

fn options(root: &Path) -> Options {
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
      Identity::Own { why, .. } => Group::Same {
        why: why.to_string(),
      },
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
    let xpointer = match &identity {
      Identity::Skip { .. } => Err(String::new()),
      _ => session
        .local(book, &identity)
        .map(|(local, _)| local.xpointer),
    };
    rows.push(Row {
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
    let (group, local) = match session.mapping(book) {
      Ok(mapping) => (
        Group::Mapped {
          original: mapping.epub.path.clone(),
        },
        Some(session.local(book, mapping)),
      ),
      Err(_) if session.store.for_kindle(&book.content_id).is_none() => (Group::NotMapped, None),
      Err(message) => (Group::Problem { message }, None),
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
  Ok(Loaded {
    device: "Kindle".into(),
    library: None,
    mappings: session.store.mappings.len(),
    rows,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::sync::OnceLock;

  fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures")
  }

  /// No configuration and no mappings of the machine running the tests.
  fn isolate() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
      let dir = std::env::temp_dir().join(format!("nickelinksync-gui-{}-data", std::process::id()));
      std::env::set_var("NICKELINKSYNC_DATA_DIR", &dir);
      std::env::set_var("NICKELINKSYNC_CONFIG", dir.join("no-config.ini"));
    });
  }

  #[test]
  fn tolino_books_without_library_sync_under_their_own_checksum() {
    isolate();
    let root =
      std::env::temp_dir().join(format!("nickelinksync-gui-{}-tolino", std::process::id()));
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
    assert!(matches!(&a.group, Group::Same { .. }), "{a:?}");
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
    isolate();
    let loaded = load(&fixtures().join("kindle"), Kind::Kindle, &mut |_, _| {}).unwrap();
    assert_eq!(loaded.device, "Kindle");
    assert_eq!(loaded.rows.len(), 3);
    assert!(loaded.rows.iter().all(|r| r.group == Group::NotMapped));
  }
}
