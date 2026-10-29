//! Making a book mapping, shared by the CLI (`map suggest`, `map add`) and
//! the desktop client: the books on a device as the mapping sees them,
//! the library's candidates for one of them, and the mapping itself, saved
//! in the [`Store`]. Nothing here prints or asks.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::db::Device;
use crate::epub::Epub;
use crate::hash::partial_md5;
use crate::kindle_device::{self, Unreadable};
use crate::library::{Library, LibraryBook};
use crate::mapping::{DeviceBook, Mapping, Original, Store};
use crate::session::{kindle_matches, select, Result};
use crate::suggest::{self, Candidate};
use crate::sync::iso_timestamp;

/// A book on the device, as the mapping sees it.
#[derive(Debug, Clone)]
pub struct OnDevice {
  pub book: DeviceBook,
  /// What `--content-id` selects it by: the ContentID (Kobo, Tolino) or
  /// the `content_id` (Kindle).
  pub id: String,
  /// Checksum of the file (Kobo, Tolino); a KFX has none that matters.
  pub hash: Option<String>,
  pub chapters: DeviceChapters,
}

/// What the device book's chapters are compared with.
#[derive(Debug, Clone)]
pub enum DeviceChapters {
  /// Spine items of a kepub or EPUB, as paths from the EPUB root.
  Spine(Vec<String>),
  /// Sections of a KFX.
  Sections(usize),
  /// The device file could not be read as an EPUB.
  Unknown(String),
}

impl DeviceChapters {
  /// Matching chapters and how many there are, against the EPUB's spine.
  pub fn overlap(&self, spine: &[String]) -> Option<(usize, usize)> {
    match self {
      DeviceChapters::Spine(own) => Some(suggest::spine_overlap(own, spine)),
      DeviceChapters::Sections(n) => Some(suggest::section_overlap(*n, spine.len())),
      DeviceChapters::Unknown(_) => None,
    }
  }
}

/// The books on a Kobo, Tolino or Kindle, filtered like `--book` and
/// `--content-id` filter `status`, and the KFX files that could not be
/// read.
pub fn device_books(
  root: &Path,
  filter: Option<&str>,
  content_id: Option<&str>,
) -> Result<(Vec<OnDevice>, Vec<Unreadable>)> {
  if kindle_device::is_kindle(root) {
    let (books, unreadable) = kindle_device::books(root)?;
    let books = kindle_matches(root, books, filter, content_id)
      .into_iter()
      .map(|b| OnDevice {
        id: b.content_id.clone(),
        hash: None,
        chapters: DeviceChapters::Sections(b.sections),
        book: DeviceBook::Kindle {
          content_id: b.content_id,
          title: b.title,
          author: b.author,
        },
      })
      .collect();
    return Ok((books, unreadable));
  }
  let device = Device::open(root, false)?;
  let mut found = Vec::new();
  for book in select(device.db.sideloaded_books()?, filter, content_id) {
    let file = device.book_file(&book);
    let hash = partial_md5(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let chapters = match Epub::open(&file) {
      Ok(epub) => DeviceChapters::Spine(epub.spine),
      Err(e) => DeviceChapters::Unknown(e.to_string()),
    };
    found.push(OnDevice {
      id: book.content_id.clone(),
      hash: Some(hash.clone()),
      chapters,
      book: DeviceBook::File {
        hash,
        path: book.onboard_path().to_string(),
        title: book.title,
        author: book.author,
      },
    });
  }
  Ok((found, Vec::new()))
}

/// The library at `library`, else at `library` in the configuration file
/// (`config`, or the default file); `None` if neither is set. A `config`
/// that was given must exist.
pub fn open_library(library: Option<&Path>, config: Option<&Path>) -> Result<Option<Library>> {
  let path = match library {
    Some(path) => Some(path.to_path_buf()),
    None => {
      let config = match config {
        Some(path) if !path.exists() => {
          return Err(format!("{}: no such file", path.display()).into())
        }
        Some(path) => Some(path.to_path_buf()),
        None => Config::default_path(),
      };
      match config {
        Some(config) => Config::library_setting(&config)?,
        None => None,
      }
    }
  };
  Ok(match path {
    Some(path) => Some(Library::open(&path)?),
    None => None,
  })
}

/// How many candidates get their chapters and checksum compared.
pub const COMPARED: usize = 5;

/// The library's best candidates for a device book, with chapters
/// compared, and the library book that is the same file, if any.
pub fn candidates<'a>(
  on: &OnDevice,
  library: &'a Library,
) -> (Vec<Candidate<'a>>, Option<&'a LibraryBook>) {
  let mut ranked = suggest::rank(&library.books, on.book.title(), on.book.author());
  ranked.truncate(COMPARED);
  let mut same = None;
  for c in &mut ranked {
    if on.hash.is_some() && c.book.hash().ok() == on.hash {
      same = Some(c.book);
    }
    if let Ok(epub) = Epub::open(&c.book.path) {
      c.chapters = on.chapters.overlap(&epub.spine);
    }
  }
  ranked.retain(|c| c.score() >= suggest::MIN_SCORE);
  suggest::sort(&mut ranked);
  (ranked, same)
}

/// What [`add`] did.
#[derive(Debug)]
pub enum Outcome {
  /// The device file is the EPUB itself: it syncs under that checksum, no
  /// mapping needed; an old mapping of it was removed.
  SameFile { removed: Option<Mapping> },
  /// Mapped, replacing an older mapping of the book, if any.
  Mapped { replaced: Option<Mapping> },
}

/// A mapping made by [`add`], and what is needed to describe it.
#[derive(Debug)]
pub struct Added {
  pub original: Original,
  /// The EPUB's spine items, for [`DeviceChapters::overlap`].
  pub spine: Vec<String>,
  pub outcome: Outcome,
}

/// The EPUB at `path` as the original of a mapping: absolute path,
/// checksum, title and author, and its spine.
pub fn original(path: &Path) -> Result<(Original, Vec<String>)> {
  let path: PathBuf = path
    .canonicalize()
    .map_err(|e| format!("{}: {e}", path.display()))?;
  let epub = Epub::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  let hash = partial_md5(&path)?;
  let original = Original {
    path: path
      .to_str()
      .ok_or_else(|| format!("{}: path is not UTF-8", path.display()))?
      .to_string(),
    hash,
    title: epub.title.clone(),
    author: (!epub.authors.is_empty()).then(|| epub.authors.join(", ")),
  };
  Ok((original, epub.spine))
}

/// Maps `on` to the EPUB at `epub_path` and saves the store. If the device
/// file is the EPUB itself, no mapping is needed, and an old one is
/// removed.
pub fn add(store: &mut Store, on: &OnDevice, epub_path: &Path) -> Result<Added> {
  let (original, spine) = original(epub_path)?;
  if on.hash.as_deref() == Some(original.hash.as_str()) {
    let removed = match store.get(&on.book).cloned() {
      Some(old) => {
        store.remove(old.device.key())?;
        store.save()?;
        Some(old)
      }
      None => None,
    };
    return Ok(Added {
      original,
      spine,
      outcome: Outcome::SameFile { removed },
    });
  }
  let mapping = Mapping {
    device: on.book.clone(),
    epub: original.clone(),
    created: iso_timestamp(
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64,
    ),
  };
  let replaced = store.insert(mapping);
  store.save()?;
  Ok(Added {
    original,
    spine,
    outcome: Outcome::Mapped { replaced },
  })
}
