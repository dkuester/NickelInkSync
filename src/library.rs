//! The library of original EPUBs: the files KOReader opens, whose checksums
//! the KOSync server knows.
//!
//! A folder with a Calibre `metadata.db` is read as a Calibre library: the
//! database is opened read-only as an immutable SQLite file
//! (`?immutable=1`, so SQLite creates no journal and takes no lock), and
//! titles, authors and EPUB paths come from it. Any other folder is
//! searched for `*.epub` files (not `*.kepub.epub`, which are not
//! originals), with title and authors from each OPF. Nothing is ever written
//! into the library folder.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::epub::Epub;
use crate::hash::partial_md5;

#[derive(Debug)]
pub enum Error {
  /// The folder does not exist or cannot be read.
  Io(PathBuf, io::Error),
  /// `metadata.db` is there but not a Calibre database we can read.
  Calibre(PathBuf, rusqlite::Error),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(path, e) => write!(f, "library {}: {e}", path.display()),
      Error::Calibre(path, e) => write!(f, "Calibre library {}: {e}", path.display()),
    }
  }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  /// A Calibre library (`metadata.db`).
  Calibre,
  /// Any folder, searched for `*.epub`.
  Folder,
}

/// An original EPUB in the library.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryBook {
  pub path: PathBuf,
  pub title: Option<String>,
  pub authors: Vec<String>,
  /// The book's id in Calibre.
  pub calibre_id: Option<i64>,
}

impl LibraryBook {
  /// KOReader's partial MD5 of the file.
  pub fn hash(&self) -> io::Result<String> {
    partial_md5(&self.path)
  }
}

#[derive(Debug, Clone)]
pub struct Library {
  pub root: PathBuf,
  pub kind: Kind,
  pub books: Vec<LibraryBook>,
  /// Books whose EPUB is listed but could not be read, and why.
  pub skipped: Vec<(PathBuf, String)>,
}

impl Library {
  /// Reads the library at `root`.
  pub fn open(root: &Path) -> Result<Self, Error> {
    let meta = fs::metadata(root).map_err(|e| Error::Io(root.to_path_buf(), e))?;
    if !meta.is_dir() {
      return Err(Error::Io(
        root.to_path_buf(),
        io::Error::new(io::ErrorKind::NotADirectory, "not a folder"),
      ));
    }
    if root.join("metadata.db").is_file() {
      calibre(root)
    } else {
      folder(root)
    }
  }
}

/// The URI of an immutable, read-only SQLite file. Backslashes become
/// slashes, and a Windows drive (`C:/…`) gets a leading slash.
fn immutable_uri(path: &Path) -> String {
  let path = path.to_string_lossy().replace('\\', "/");
  let mut uri = String::from("file:");
  if path.as_bytes().get(1) == Some(&b':') {
    uri.push('/');
  }
  for b in path.bytes() {
    match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
        uri.push(b as char)
      }
      _ => uri.push_str(&format!("%{b:02X}")),
    }
  }
  uri.push_str("?mode=ro&immutable=1");
  uri
}

fn calibre(root: &Path) -> Result<Library, Error> {
  let db_path = root.join("metadata.db");
  let err = |e| Error::Calibre(db_path.clone(), e);
  let db = Connection::open_with_flags(
    immutable_uri(&db_path),
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
  )
  .map_err(err)?;
  // One row per book with an EPUB; `data.name` is the file name without
  // the extension, `books.path` the folder below the library.
  let mut stmt = db
    .prepare(
      "SELECT b.id, b.title, b.path, d.name FROM books b \
       JOIN data d ON d.book = b.id AND upper(d.format) = 'EPUB' ORDER BY b.id",
    )
    .map_err(err)?;
  let rows: Vec<(i64, Option<String>, String, String)> = stmt
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)?;
  let mut authors = db
    .prepare(
      "SELECT a.name FROM books_authors_link l JOIN authors a ON a.id = l.author \
       WHERE l.book = ?1 ORDER BY l.id",
    )
    .map_err(err)?;
  let (mut books, mut skipped) = (Vec::new(), Vec::new());
  for (id, title, folder, name) in rows {
    let path = root.join(&folder).join(format!("{name}.epub"));
    if !path.is_file() {
      skipped.push((
        path,
        "listed in metadata.db, but the file is missing".into(),
      ));
      continue;
    }
    let names = authors
      .query_map([id], |r| r.get::<_, String>(0))
      .map_err(err)?
      .collect::<Result<Vec<_>, _>>()
      .map_err(err)?;
    books.push(LibraryBook {
      path,
      title,
      authors: names,
      calibre_id: Some(id),
    });
  }
  Ok(Library {
    root: root.to_path_buf(),
    kind: Kind::Calibre,
    books,
    skipped,
  })
}

fn folder(root: &Path) -> Result<Library, Error> {
  let mut files = Vec::new();
  collect_epubs(root, &mut files).map_err(|e| Error::Io(root.to_path_buf(), e))?;
  files.sort();
  let (mut books, mut skipped) = (Vec::new(), Vec::new());
  for path in files {
    match Epub::open(&path) {
      Ok(epub) => books.push(LibraryBook {
        path,
        title: epub.title,
        authors: epub.authors,
        calibre_id: None,
      }),
      Err(e) => skipped.push((path, e.to_string())),
    }
  }
  Ok(Library {
    root: root.to_path_buf(),
    kind: Kind::Folder,
    books,
    skipped,
  })
}

fn collect_epubs(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let name = entry.file_name();
    let name = name.to_string_lossy().to_lowercase();
    // Hidden files, macOS AppleDouble files (`._x.epub`).
    if name.starts_with('.') {
      continue;
    }
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_epubs(&path, out)?;
    } else if name.ends_with(".epub") && !name.ends_with(".kepub.epub") {
      out.push(path);
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn uri_escapes_the_path() {
    assert_eq!(
      immutable_uri(Path::new("/Users/a/Calibre Library/metadata.db")),
      "file:/Users/a/Calibre%20Library/metadata.db?mode=ro&immutable=1"
    );
    assert_eq!(
      immutable_uri(Path::new("/b/Bücher?#/m.db")),
      "file:/b/B%C3%BCcher%3F%23/m.db?mode=ro&immutable=1"
    );
    assert_eq!(
      immutable_uri(Path::new("C:\\Books\\metadata.db")),
      "file:/C:/Books/metadata.db?mode=ro&immutable=1"
    );
  }
}
