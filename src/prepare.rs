//! Preparing a Kobo or Tolino for syncing on its own (`--live`, run by the
//! plugin): what the CLI on the device needs of the desktop's library and
//! mappings, in `<device>/.adds/NickelInkSync/`. Nothing else on the
//! device is written, and the original EPUBs never go onto it.
//!
//! - `library.json`, the **library index**: title, authors and KOReader
//!   partial MD5 of every original EPUB in the library. On the device a
//!   book is sent only if its checksum is in the index (it is an original)
//!   or it has a mapping with a table; without an index, `push --live`
//!   sends nothing.
//! - `mappings.json`, the **mappings** of the books on this device, in the
//!   format of the desktop's [`Store`], plus `added` (mappings made on the
//!   device, whose original the desktop finds by its checksum in the
//!   library) and `removed` (checksums of device files whose mapping was
//!   removed on the device). [`prepare`] takes both over into the desktop's
//!   store and writes the list again from it, so the device, the CLI and
//!   the desktop client have the same mappings.
//! - `tables/<device checksum>.json`, a **translation table**
//!   ([`crate::table`]) for each mapped book on the device.
//!
//! Each file is written atomically (a temporary file renamed over the old
//! one); `library.json` last, so a device with an index has the rest too.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::Device;
use crate::epub::Epub;
use crate::hash::partial_md5;
use crate::kindle_device;
use crate::library::Library;
use crate::mapping::{DeviceBook, EpubState, Mapping, Original, Store};
use crate::session::Result;
use crate::sync::iso_timestamp;
use crate::table::{self, Table};

/// The folder below the device root.
pub const DIR: &str = ".adds/NickelInkSync";
const LIBRARY: &str = "library.json";
const MAPPINGS: &str = "mappings.json";
/// The format of `library.json` and `mappings.json`; a newer one is not
/// read.
pub const FORMAT: u32 = 1;

/// An original EPUB in the index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexBook {
  /// KOReader partial MD5.
  pub hash: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub authors: Vec<String>,
}

/// `library.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Index {
  pub format: u32,
  /// When it was written (ISO 8601, UTC).
  pub created: String,
  pub books: Vec<IndexBook>,
}

/// `mappings.json` on the device.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceMappings {
  pub format: u32,
  /// From the desktop, for the books on the device.
  #[serde(default)]
  pub mappings: Vec<Mapping>,
  /// Made on the device, not yet taken over by the desktop. The original's
  /// `path` may be empty: the desktop finds it by its checksum.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub added: Vec<Mapping>,
  /// Checksums of device files whose mapping was removed on the device.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub removed: Vec<String>,
}

impl DeviceMappings {
  /// The mapping that holds on the device for the file with checksum
  /// `hash`: one added there, else one from the desktop not removed there.
  pub fn for_file(&self, hash: &str) -> Option<&Mapping> {
    let of = |m: &&Mapping| matches!(&m.device, DeviceBook::File { hash: h, .. } if h == hash);
    self.added.iter().rev().find(of).or_else(|| {
      if self.removed.iter().any(|r| r == hash) {
        return None;
      }
      self.mappings.iter().find(of)
    })
  }

  /// Every mapping that holds on the device.
  pub fn holding(&self) -> impl Iterator<Item = &Mapping> {
    let added: HashSet<&str> = self.added.iter().map(|m| m.device.key()).collect();
    self
      .mappings
      .iter()
      .filter(move |m| {
        !added.contains(m.device.key()) && !self.removed.iter().any(|r| r == m.device.key())
      })
      .chain(self.added.iter())
  }
}

/// What the device holds of the desktop's library and mappings.
#[derive(Debug, Clone)]
pub struct Prepared {
  /// `.adds/NickelInkSync` on the device.
  pub dir: PathBuf,
  pub index: Index,
  pub mappings: DeviceMappings,
  hashes: HashSet<String>,
}

impl Prepared {
  /// What [`prepare`] left on the device at `root`; `None` without an
  /// index. A broken or newer file is an error.
  pub fn load(root: &Path) -> io::Result<Option<Self>> {
    let dir = root.join(DIR);
    let Some(index) = read::<Index>(&dir.join(LIBRARY))? else {
      return Ok(None);
    };
    let mappings = read::<DeviceMappings>(&dir.join(MAPPINGS))?.unwrap_or_default();
    let hashes = index.books.iter().map(|b| b.hash.clone()).collect();
    Ok(Some(Prepared {
      dir,
      index,
      mappings,
      hashes,
    }))
  }

  /// The file with checksum `hash` is an original in the library.
  pub fn is_original(&self, hash: &str) -> bool {
    self.hashes.contains(hash)
  }

  /// The table file for the device file with checksum `hash`.
  pub fn table_path(&self, hash: &str) -> PathBuf {
    Table::path(&self.dir, hash)
  }
}

/// Reads a JSON file of format [`FORMAT`] or older; `None` if missing.
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
  let text = match fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
    Err(e) => return Err(e),
  };
  #[derive(Deserialize)]
  struct Version {
    format: u32,
  }
  let bad = |e: serde_json::Error| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      format!("{}: {e}", path.display()),
    )
  };
  let version: Version = serde_json::from_str(&text).map_err(bad)?;
  if version.format > FORMAT {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      format!(
        "{}: format {} is newer than this version understands ({FORMAT}); update NickelInkSync",
        path.display(),
        version.format
      ),
    ));
  }
  serde_json::from_str(&text).map(Some).map_err(bad)
}

/// Writes `text` to `path` through a temporary file next to it.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
  let dir = path.parent().unwrap_or(Path::new("."));
  fs::create_dir_all(dir)?;
  let name = path
    .file_name()
    .map_or("file".into(), |n| n.to_string_lossy());
  let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
  let written = (|| {
    let mut out = fs::File::create(&tmp)?;
    out.write_all(text.as_bytes())?;
    out.sync_all()
  })();
  if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
    let _ = fs::remove_file(&tmp);
    return Err(e);
  }
  Ok(())
}

fn json<T: Serialize>(value: &T, pretty: bool) -> String {
  let mut text = match pretty {
    true => serde_json::to_string_pretty(value),
    false => serde_json::to_string(value),
  }
  .expect("serializes");
  text.push('\n');
  text
}

/// What [`prepare`] did.
#[derive(Debug, Default)]
pub struct Report {
  /// Original EPUBs in the index.
  pub originals: usize,
  /// Mappings now on the device.
  pub mappings: usize,
  /// Mappings made on the device, now in the desktop's store.
  pub imported: Vec<Mapping>,
  /// Mappings removed on the device, now removed from the store too.
  pub removed: Vec<Mapping>,
  /// Tables written, and kept because they were up to date.
  pub tables_written: usize,
  pub tables_kept: usize,
  /// Tables of books no longer mapped, removed.
  pub tables_removed: usize,
  /// Mappings made on the device whose original is not in the library:
  /// the book (title or path), why. They stay on the device.
  pub unresolved: Vec<(String, String)>,
  /// Books or EPUBs that could not be read, and mappings left off the
  /// device: what, why.
  pub failed: Vec<(String, String)>,
}

/// A book on the device, for [`prepare`].
struct OnDevice {
  path: String,
  title: Option<String>,
  file: PathBuf,
}

fn now() -> String {
  iso_timestamp(
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map_or(0, |d| d.as_secs() as i64),
  )
}

/// Prepares the Kobo or Tolino mounted at `root`: takes the mappings made
/// or removed on the device over into `store` (and saves it), writes the
/// mappings of the books on the device, a translation table for each, and
/// the index of `library`. `progress(done, total)` is called as the
/// originals are read and then the tables are built (`total` grows by the
/// tables once their number is known).
pub fn prepare(
  root: &Path,
  library: &Library,
  store: &mut Store,
  progress: &mut dyn FnMut(usize, usize),
) -> Result<Report> {
  if kindle_device::is_kindle(root) {
    return Err("only a Kobo or Tolino can be prepared".into());
  }
  let device = Device::open(root, false)?;
  let dir = root.join(DIR);
  let mut report = Report::default();

  // The books on the device by checksum.
  let mut on_device: HashMap<String, OnDevice> = HashMap::new();
  for book in device.db.sideloaded_books()? {
    let file = device.book_file(&book);
    match partial_md5(&file) {
      Ok(hash) => {
        on_device.insert(
          hash,
          OnDevice {
            path: book.onboard_path().to_string(),
            title: book.title.clone(),
            file,
          },
        );
      }
      Err(e) => report
        .failed
        .push((book.onboard_path().to_string(), e.to_string())),
    }
  }

  // The library by checksum.
  let mut index = Vec::new();
  let mut originals: HashMap<String, usize> = HashMap::new();
  let books = library.books.len();
  for (i, book) in library.books.iter().enumerate() {
    progress(i, books);
    match book.hash() {
      Ok(hash) => {
        originals.entry(hash.clone()).or_insert(i);
        index.push(IndexBook {
          hash,
          title: book.title.clone(),
          authors: book.authors.clone(),
        });
      }
      Err(e) => report
        .failed
        .push((book.path.display().to_string(), e.to_string())),
    }
  }
  report.originals = index.len();

  // Mappings made or removed on the device go into the store.
  let old = read::<DeviceMappings>(&dir.join(MAPPINGS))?.unwrap_or_default();
  let mut changed = false;
  for hash in &old.removed {
    if let Some(m) = store.for_file(hash).cloned() {
      store.mappings.retain(|x| !x.device.same_as(&m.device));
      report.removed.push(m);
      changed = true;
    }
  }
  let mut unresolved = Vec::new();
  for added in &old.added {
    let Some(&i) = originals.get(&added.epub.hash) else {
      let what = added
        .device
        .title()
        .map(str::to_string)
        .unwrap_or_else(|| match &added.device {
          DeviceBook::File { path, .. } => path.clone(),
          other => other.key().to_string(),
        });
      report.unresolved.push((
        what,
        format!(
          "its original (checksum {}) is not in the library",
          added.epub.hash
        ),
      ));
      unresolved.push(added.clone());
      continue;
    };
    let book = &library.books[i];
    let mapping = Mapping {
      device: added.device.clone(),
      epub: Original {
        path: book.path.to_string_lossy().into_owned(),
        hash: added.epub.hash.clone(),
        title: book.title.clone(),
        author: (!book.authors.is_empty()).then(|| book.authors.join(", ")),
      },
      created: added.created.clone(),
    };
    store.insert(mapping.clone());
    report.imported.push(mapping);
    changed = true;
  }
  if changed {
    store.save()?;
  }

  // The mappings of the books on the device, each with its table.
  let mut mappings = Vec::new();
  let mut tables = HashSet::new();
  let mapped: Vec<(&Mapping, &String, &OnDevice)> = store
    .mappings
    .iter()
    .filter_map(|m| match &m.device {
      DeviceBook::File { hash, .. } => on_device.get(hash).map(|book| (m, hash, book)),
      _ => None,
    })
    .collect();
  let total = books + mapped.len();
  for (i, &(m, hash, book)) in mapped.iter().enumerate() {
    progress(books + i, total);
    let what = book.title.clone().unwrap_or_else(|| book.path.clone());
    let state = m.check();
    if state != EpubState::Ok {
      report
        .failed
        .push((what, format!("the mapping to {} is {state}", m.epub.path)));
      continue;
    }
    let path = Table::path(&dir, hash);
    let current = Table::load(&path)
      .ok()
      .filter(|t| t.format == table::FORMAT && t.device == *hash && t.original == m.epub.hash);
    if current.is_some() {
      report.tables_kept += 1;
    } else {
      match build_table(&book.file, hash, m) {
        Ok(table) => {
          write_atomic(&path, &json(&table, false))?;
          report.tables_written += 1;
        }
        Err(e) => {
          report.failed.push((what, format!("no table: {e}")));
          continue;
        }
      }
    }
    tables.insert(format!("{hash}.json"));
    mappings.push(m.clone());
  }
  report.mappings = mappings.len();

  // Tables of books no longer mapped.
  match fs::read_dir(dir.join(table::DIR)) {
    Ok(entries) => {
      for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") && !tables.contains(&name) {
          fs::remove_file(entry.path())?;
          report.tables_removed += 1;
        }
      }
    }
    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
    Err(e) => return Err(e.into()),
  }

  progress(total, total);
  let device_mappings = DeviceMappings {
    format: FORMAT,
    mappings,
    added: unresolved,
    removed: Vec::new(),
  };
  write_atomic(&dir.join(MAPPINGS), &json(&device_mappings, true))?;
  let index = Index {
    format: FORMAT,
    created: now(),
    books: index,
  };
  write_atomic(&dir.join(LIBRARY), &json(&index, false))?;
  Ok(report)
}

fn build_table(file: &Path, hash: &str, mapping: &Mapping) -> Result<Table> {
  let mut device = Epub::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
  let mut original =
    Epub::open(Path::new(&mapping.epub.path)).map_err(|e| format!("{}: {e}", mapping.epub.path))?;
  Ok(Table::build(
    &mut device,
    hash,
    &mut original,
    &mapping.epub.hash,
  )?)
}

/// Puts one mapping onto the prepared device at `root`, with its table:
/// for a book just copied there (a kepub mapped by [`crate::copy`]). The
/// device file must be at the mapping's path. Does nothing on a device
/// that was not prepared (no index): [`prepare`] writes it all then.
pub fn put_mapping(root: &Path, mapping: &Mapping) -> Result<()> {
  let DeviceBook::File { hash, path, .. } = &mapping.device else {
    return Err("only a Kobo or Tolino book has a table".into());
  };
  let dir = root.join(DIR);
  if !dir.join(LIBRARY).is_file() {
    return Ok(());
  }
  let table = build_table(&root.join(path), hash, mapping)?;
  write_atomic(&Table::path(&dir, hash), &json(&table, false))?;
  let mut device = read::<DeviceMappings>(&dir.join(MAPPINGS))?.unwrap_or_default();
  device.format = FORMAT;
  device
    .mappings
    .retain(|m| !m.device.same_as(&mapping.device));
  device.mappings.push(mapping.clone());
  device.removed.retain(|r| r != hash);
  write_atomic(&dir.join(MAPPINGS), &json(&device, true))?;
  Ok(())
}
