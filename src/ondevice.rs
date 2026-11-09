//! Mapping a book on the device itself (stage C6), for the plugin's dialog:
//! which books do not sync yet, which library originals fit one of them
//! (from the index `device prepare` left, so by title and author only: the
//! EPUBs are not on the device, and neither are their chapters), and the
//! mapping, written as `added` in `mappings.json`. The computer takes it
//! over and builds the translation table at the next `device prepare`;
//! until then the book is mapped but does not sync
//! ([`Standing::Waits`]).

use std::path::{Path, PathBuf};

use crate::db::{Book, Device};
use crate::hash::partial_md5;
use crate::library::LibraryBook;
use crate::mapping::DeviceBook;
use crate::prepare::{self, Prepared, Standing};
use crate::session::Result;
use crate::suggest::{self, MIN_SCORE};

/// How many suggestions the dialog can step through.
pub const SUGGESTIONS: usize = 5;

/// A sideloaded book that does not sync (yet).
#[derive(Debug, Clone, PartialEq)]
pub struct Unsynced {
  pub content_id: String,
  pub title: Option<String>,
  pub author: Option<String>,
  pub standing: Standing,
}

/// A library original that may be the book.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
  /// Its checksum in the index.
  pub hash: String,
  pub title: Option<String>,
  pub authors: Vec<String>,
  /// Title and author only, 0 to 1.
  pub score: f64,
}

fn prepared(root: &Path) -> Result<Prepared> {
  Prepared::load(root)?.ok_or_else(|| "the device was not prepared".into())
}

fn books_of(device: &Device) -> Result<Vec<Book>> {
  Ok(device.db.sideloaded_books()?)
}

fn kepub(book: &Book) -> bool {
  book.is_kepub() || book.onboard_path().ends_with(".kepub.epub")
}

/// The books on the device that do not sync: not mapped, or mapped and
/// waiting for the computer's table. `None` if the device was not
/// prepared. A book whose file cannot be read is left out.
pub fn unsynced(root: &Path, live: bool) -> Result<Option<Vec<Unsynced>>> {
  let Some(prepared) = Prepared::load(root)? else {
    return Ok(None);
  };
  let device = Device::open(root, live)?;
  let mut found = Vec::new();
  for book in books_of(&device)? {
    let Ok(hash) = partial_md5(&device.book_file(&book)) else {
      continue;
    };
    let standing = prepared.standing(&hash, kepub(&book));
    if standing != Standing::Syncs {
      found.push(Unsynced {
        content_id: book.content_id,
        title: book.title,
        author: book.author,
        standing,
      });
    }
  }
  Ok(Some(found))
}

/// The originals in the index that fit the book `content_id` best, best
/// first, at most [`SUGGESTIONS`], none below [`MIN_SCORE`].
pub fn suggest(root: &Path, content_id: &str, live: bool) -> Result<Vec<Suggestion>> {
  let prepared = prepared(root)?;
  let device = Device::open(root, live)?;
  let book = find(&device, content_id)?;
  // The ranking wants a path; the index has none, so the checksum stands in
  // (it also breaks ties the same way every time).
  let library: Vec<LibraryBook> = prepared
    .index
    .books
    .iter()
    .map(|b| LibraryBook {
      path: PathBuf::from(&b.hash),
      title: b.title.clone(),
      authors: b.authors.clone(),
      calibre_id: None,
    })
    .collect();
  let mut found = Vec::new();
  for c in suggest::rank(&library, book.title.as_deref(), book.author.as_deref()) {
    if c.score() < MIN_SCORE {
      continue;
    }
    found.push(Suggestion {
      hash: c.book.path.to_string_lossy().into_owned(),
      title: c.book.title.clone(),
      authors: c.book.authors.clone(),
      score: c.score(),
    });
    if found.len() == SUGGESTIONS {
      break;
    }
  }
  Ok(found)
}

fn find(device: &Device, content_id: &str) -> Result<Book> {
  books_of(device)?
    .into_iter()
    .find(|b| b.content_id == content_id)
    .ok_or_else(|| format!("no such book on the device: {content_id}").into())
}

/// Maps the book `content_id` to the original with checksum
/// `original_hash` from the index.
pub fn map(root: &Path, content_id: &str, original_hash: &str, live: bool) -> Result<Unsynced> {
  let prepared = prepared(root)?;
  let device = Device::open(root, live)?;
  let book = find(&device, content_id)?;
  let file = device.book_file(&book);
  let hash = partial_md5(&file).map_err(|e| format!("{}: {e}", file.display()))?;
  prepare::add_mapping(
    root,
    DeviceBook::File {
      hash: hash.clone(),
      path: book.onboard_path().to_string(),
      title: book.title.clone(),
      author: book.author.clone(),
    },
    original_hash,
  )?;
  // Reload: the mapping is there now, the table not.
  let standing = Prepared::load(root)?
    .unwrap_or(prepared)
    .standing(&hash, kepub(&book));
  Ok(Unsynced {
    content_id: book.content_id,
    title: book.title,
    author: book.author,
    standing,
  })
}
