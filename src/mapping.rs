//! Manual book mappings: a book on a device linked to its original EPUB.
//!
//! KOSync names a book by KOReader's partial MD5 of the file
//! ([`crate::hash`]). A kepub on a Kobo, a differently converted EPUB of the
//! same book on a Tolino and a KFX on a Kindle all have other checksums than
//! the original EPUB that KOReader opens, so the server sees different
//! books. A mapping says: sync this device book under the original EPUB's
//! checksum, and translate its position into that EPUB.
//!
//! The device side is keyed by what identifies the book there:
//!
//! - **Kobo and Tolino: the partial MD5 of the file on the device.** It is
//!   what the device holds and what the sync computes anyway; it does not
//!   depend on the mount point, the folder or the file name, so moving or
//!   renaming the file keeps the mapping, and the same file on two devices
//!   shares one mapping. A re-converted file gets a new checksum and loses
//!   its mapping, which is right: positions in the old file may not fit it.
//!   Path and title are kept for display only.
//! - **Kindle: the `content_id`** from the KFX metadata (32 hex digits, also
//!   in the file name). Nothing in the KFX leads to the EPUB's checksum, the
//!   `title` is the file stem the book was sent under and need not be
//!   unique, and the KFX file itself is rewritten by Amazon; the
//!   `content_id` stays with the document. A different conversion of the
//!   same document shows up anyway: its stored position no longer fits the
//!   KFX ([`crate::kindle::Error::StalePosition`]).
//!
//! The original EPUB is kept as its path and the checksum it had when the
//! mapping was made. If the file is gone or its checksum changed, the
//! mapping is broken ([`Mapping::check`]): syncing under the old checksum
//! would send positions to a book KOReader no longer has, and translating
//! into a changed file could land anywhere.
//!
//! The store is one JSON file in the data directory
//! ([`crate::config::Config::data_dir`]), shared with the desktop client,
//! with a format version, written atomically (a temporary file renamed over
//! the old one). It is never written next to the books.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::hash::partial_md5;

/// Version of the file format; a newer file is refused, not rewritten.
pub const FORMAT: u32 = 1;

/// File name in the data directory.
pub const FILE_NAME: &str = "mappings.json";

#[derive(Debug)]
pub enum Error {
  Io(PathBuf, io::Error),
  /// The file is not a mapping store, or a broken one.
  Parse(PathBuf, String),
  /// Written by a newer version of NickelInkSync.
  NewerFormat(PathBuf, u32),
  /// No mapping, or more than one, matches the given key.
  NotFound(String),
  Ambiguous(String, usize),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
      Error::Parse(path, e) => write!(f, "{}: {e}", path.display()),
      Error::NewerFormat(path, v) => write!(
        f,
        "{}: format {v} is newer than this version understands ({FORMAT}); update NickelInkSync",
        path.display()
      ),
      Error::NotFound(key) => write!(f, "no mapping for `{key}`"),
      Error::Ambiguous(key, n) => write!(f, "`{key}` matches {n} mappings; give more digits"),
    }
  }
}

impl std::error::Error for Error {}

/// The book on the device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DeviceBook {
  /// A file on a Kobo or Tolino (kepub or EPUB).
  File {
    /// KOReader partial MD5 of the file: the key.
    hash: String,
    /// Path on the device storage, e.g. `Books/heat-2.kepub.epub`.
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<String>,
  },
  /// A KFX on a Kindle.
  Kindle {
    /// `content_id` from the KFX metadata: the key.
    content_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<String>,
  },
}

impl DeviceBook {
  /// The key: the file's checksum, or the Kindle `content_id`.
  pub fn key(&self) -> &str {
    match self {
      DeviceBook::File { hash, .. } => hash,
      DeviceBook::Kindle { content_id, .. } => content_id,
    }
  }

  /// Same device book (same kind and key).
  pub fn same_as(&self, other: &DeviceBook) -> bool {
    std::mem::discriminant(self) == std::mem::discriminant(other) && self.key() == other.key()
  }

  pub fn title(&self) -> Option<&str> {
    match self {
      DeviceBook::File { title, .. } | DeviceBook::Kindle { title, .. } => title.as_deref(),
    }
  }

  pub fn author(&self) -> Option<&str> {
    match self {
      DeviceBook::File { author, .. } | DeviceBook::Kindle { author, .. } => author.as_deref(),
    }
  }
}

/// The original EPUB, whose checksum KOSync and KOReader use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Original {
  /// Absolute path.
  pub path: String,
  /// KOReader partial MD5 when the mapping was made.
  pub hash: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
  pub device: DeviceBook,
  pub epub: Original,
  /// When the mapping was made (ISO 8601, UTC).
  pub created: String,
}

/// Whether the original EPUB of a mapping is still the one it was made
/// with.
#[derive(Debug, Clone, PartialEq)]
pub enum EpubState {
  Ok,
  /// The file is gone.
  Missing,
  /// The file is there, with another checksum.
  Changed {
    hash: String,
  },
  /// The file is there but cannot be read.
  Unreadable(String),
}

impl fmt::Display for EpubState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      EpubState::Ok => write!(f, "ok"),
      EpubState::Missing => write!(f, "broken: the EPUB is missing"),
      EpubState::Changed { hash } => {
        write!(f, "broken: the EPUB has changed (checksum now {hash})")
      }
      EpubState::Unreadable(e) => write!(f, "broken: the EPUB cannot be read ({e})"),
    }
  }
}

impl Mapping {
  /// Checks the original EPUB against the checksum stored with the mapping.
  pub fn check(&self) -> EpubState {
    let path = Path::new(&self.epub.path);
    if !path.exists() {
      return EpubState::Missing;
    }
    match partial_md5(path) {
      Ok(hash) if hash == self.epub.hash => EpubState::Ok,
      Ok(hash) => EpubState::Changed { hash },
      Err(e) => EpubState::Unreadable(e.to_string()),
    }
  }
}

#[derive(Serialize, Deserialize)]
struct StoreFile {
  format: u32,
  #[serde(default)]
  mappings: Vec<Mapping>,
}

/// The mappings, loaded from and saved to one file.
#[derive(Debug, Clone, Default)]
pub struct Store {
  pub path: PathBuf,
  pub mappings: Vec<Mapping>,
}

impl Store {
  /// `mappings.json` in [`Config::data_dir`].
  pub fn default_path() -> Option<PathBuf> {
    Some(Config::data_dir()?.join(FILE_NAME))
  }

  /// Loads the store at `path`; a missing file is an empty store.
  pub fn load(path: &Path) -> Result<Self, Error> {
    let text = match fs::read_to_string(path) {
      Ok(text) => text,
      Err(e) if e.kind() == io::ErrorKind::NotFound => {
        return Ok(Store {
          path: path.to_path_buf(),
          mappings: Vec::new(),
        })
      }
      Err(e) => return Err(Error::Io(path.to_path_buf(), e)),
    };
    // The version first, so a newer file is reported as such and not as a
    // parse error.
    #[derive(Deserialize)]
    struct Version {
      format: u32,
    }
    let parse = |e: serde_json::Error| Error::Parse(path.to_path_buf(), e.to_string());
    let version: Version = serde_json::from_str(&text).map_err(parse)?;
    if version.format > FORMAT {
      return Err(Error::NewerFormat(path.to_path_buf(), version.format));
    }
    let file: StoreFile = serde_json::from_str(&text).map_err(parse)?;
    Ok(Store {
      path: path.to_path_buf(),
      mappings: file.mappings,
    })
  }

  /// Writes the store: to a temporary file next to it, flushed to disk,
  /// then renamed over the old file, so a reader sees the old or the new
  /// store, never half of one. Creates the directory if needed.
  pub fn save(&self) -> Result<(), Error> {
    let io = |path: &Path| {
      let path = path.to_path_buf();
      move |e| Error::Io(path, e)
    };
    let dir = self.path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(io(dir))?;
    let file = StoreFile {
      format: FORMAT,
      mappings: self.mappings.clone(),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("mappings serialize");
    text.push('\n');
    let name = self
      .path
      .file_name()
      .map_or("mappings.json".into(), |n| n.to_string_lossy());
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let written = (|| {
      let mut out = fs::File::create(&tmp)?;
      out.write_all(text.as_bytes())?;
      out.sync_all()
    })();
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, &self.path)) {
      let _ = fs::remove_file(&tmp);
      return Err(Error::Io(self.path.clone(), e));
    }
    Ok(())
  }

  /// The mapping for a device book.
  pub fn get(&self, device: &DeviceBook) -> Option<&Mapping> {
    self.mappings.iter().find(|m| m.device.same_as(device))
  }

  /// The mapping for a file on a Kobo or Tolino, by its checksum.
  pub fn for_file(&self, hash: &str) -> Option<&Mapping> {
    self
      .mappings
      .iter()
      .find(|m| matches!(&m.device, DeviceBook::File { hash: h, .. } if h == hash))
  }

  /// The mapping for a Kindle book, by its `content_id`.
  pub fn for_kindle(&self, content_id: &str) -> Option<&Mapping> {
    self.mappings.iter().find(
      |m| matches!(&m.device, DeviceBook::Kindle { content_id: c, .. } if c.eq_ignore_ascii_case(content_id)),
    )
  }

  /// Adds a mapping, replacing one for the same device book; returns the
  /// replaced one.
  pub fn insert(&mut self, mapping: Mapping) -> Option<Mapping> {
    match self
      .mappings
      .iter()
      .position(|m| m.device.same_as(&mapping.device))
    {
      Some(i) => Some(std::mem::replace(&mut self.mappings[i], mapping)),
      None => {
        self.mappings.push(mapping);
        None
      }
    }
  }

  /// Removes the mapping whose device key starts with `key` (case does not
  /// matter); the prefix must match exactly one.
  pub fn remove(&mut self, key: &str) -> Result<Mapping, Error> {
    let key_lower = key.to_ascii_lowercase();
    if key_lower.is_empty() {
      return Err(Error::NotFound(key.to_string()));
    }
    let matches: Vec<usize> = self
      .mappings
      .iter()
      .enumerate()
      .filter(|(_, m)| m.device.key().to_ascii_lowercase().starts_with(&key_lower))
      .map(|(i, _)| i)
      .collect();
    match matches.as_slice() {
      [] => Err(Error::NotFound(key.to_string())),
      [i] => Ok(self.mappings.remove(*i)),
      many => Err(Error::Ambiguous(key.to_string(), many.len())),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn file_book(hash: &str) -> DeviceBook {
    DeviceBook::File {
      hash: hash.into(),
      path: "Books/a.kepub.epub".into(),
      title: Some("Book A".into()),
      author: None,
    }
  }

  fn mapping(device: DeviceBook, epub_hash: &str) -> Mapping {
    Mapping {
      device,
      epub: Original {
        path: "/books/a.epub".into(),
        hash: epub_hash.into(),
        title: None,
        author: None,
      },
      created: "2026-01-01T12:00:00Z".into(),
    }
  }

  #[test]
  fn insert_replaces_the_same_device_book() {
    let dir = std::env::temp_dir().join(format!("nickelinksync-store-{}", std::process::id()));
    let mut store = Store::load(&dir.join("mappings.json")).unwrap();
    assert!(store.mappings.is_empty());
    assert_eq!(store.insert(mapping(file_book("aa11"), "e1")), None);
    let kindle = DeviceBook::Kindle {
      content_id: "AA11".into(),
      title: None,
      author: None,
    };
    // Same key text, other kind: a separate mapping.
    assert_eq!(store.insert(mapping(kindle.clone(), "e2")), None);
    let old = store.insert(mapping(file_book("aa11"), "e3")).unwrap();
    assert_eq!(old.epub.hash, "e1");
    assert_eq!(store.mappings.len(), 2);
    assert_eq!(store.for_file("aa11").unwrap().epub.hash, "e3");
    assert_eq!(store.for_kindle("aa11").unwrap().epub.hash, "e2");
    assert!(store.get(&kindle).is_some());
  }

  #[test]
  fn remove_by_unique_prefix() {
    let mut store = Store {
      path: "mappings.json".into(),
      mappings: vec![
        mapping(file_book("abc1"), "e1"),
        mapping(file_book("abd2"), "e2"),
      ],
    };
    assert!(matches!(store.remove("ab"), Err(Error::Ambiguous(_, 2))));
    assert!(matches!(store.remove("x"), Err(Error::NotFound(_))));
    assert_eq!(store.remove("ABD").unwrap().epub.hash, "e2");
    assert_eq!(store.mappings.len(), 1);
  }

  #[test]
  fn saves_atomically_and_loads_back() {
    let dir = std::env::temp_dir().join(format!("nickelinksync-save-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("sub/mappings.json");
    let mut store = Store::load(&path).unwrap();
    store.insert(mapping(file_book("aa11"), "e1"));
    store.save().unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("{\n  \"format\": 1,"), "{text}");
    assert!(text.contains("\"kind\": \"file\""), "{text}");
    // Nothing else left in the directory.
    let names: Vec<_> = fs::read_dir(dir.join("sub"))
      .unwrap()
      .map(|e| e.unwrap().file_name())
      .collect();
    assert_eq!(names, ["mappings.json"]);
    assert_eq!(Store::load(&path).unwrap().mappings, store.mappings);
    fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn refuses_a_newer_format() {
    let dir = std::env::temp_dir().join(format!("nickelinksync-newer-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mappings.json");
    fs::write(
      &path,
      r#"{"format": 2, "mappings": [{"something": "new"}]}"#,
    )
    .unwrap();
    assert!(matches!(Store::load(&path), Err(Error::NewerFormat(_, 2))));
    fs::write(&path, "not json").unwrap();
    assert!(matches!(Store::load(&path), Err(Error::Parse(..))));
    fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn check_reports_missing_and_changed_epubs() {
    let dir = std::env::temp_dir().join(format!("nickelinksync-check-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let epub = dir.join("a.epub");
    fs::write(&epub, b"first").unwrap();
    let mut m = mapping(file_book("aa11"), &partial_md5(&epub).unwrap());
    m.epub.path = epub.to_string_lossy().into_owned();
    assert_eq!(m.check(), EpubState::Ok);
    fs::write(&epub, b"second").unwrap();
    assert!(matches!(m.check(), EpubState::Changed { .. }));
    fs::remove_file(&epub).unwrap();
    assert_eq!(m.check(), EpubState::Missing);
    fs::remove_dir_all(&dir).unwrap();
  }
}
