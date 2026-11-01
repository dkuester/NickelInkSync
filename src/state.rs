//! What the device knows about each book's sync, shared by the CLI on the
//! device (`--live`, run by the plugin) and on the desktop (USB):
//! `<device>/.adds/NickelInkSync/state.json`.
//!
//! For each book, keyed by its ContentID:
//!
//! - the **base**: the position last agreed with the server (the server's
//!   XPointer, percentage and time) and the place Nickel stored for it then.
//!   With a base, the conflict rule is three-way ([`crate::sync::Moved`]):
//!   a side that did not move since loses to the one that did, so Nickel's
//!   `DateLastRead` from merely opening a book no longer counts as reading.
//! - **not in line**: the plugin wrote the server's position, but Nickel
//!   opened the book elsewhere (the chapter start) and saved that place
//!   with a new `DateLastRead`. Such a book is never pushed (`--force`
//!   does), and a pull takes the server's position without asking while
//!   Nickel's place is still the one it opened at; after reading on, the
//!   place in the book decides and the time does not count.
//! - **pending**: a push from the device failed (no network, a server
//!   error); `sync --pending` sends it later.
//!
//! The plugin used to keep the not-in-line books in `not-in-line.tsv`
//! (`ContentID<tab>ChapterIDBookmarked` per line). [`State::load`] takes
//! that file over; the first save writes them into `state.json` and removes
//! it. Until the plugin writes `state.json` itself, a new line in the file
//! is taken over the same way.
//!
//! Two CLI runs may overlap on the device (the plugin pushes in the
//! background while the next book opens), so [`State::save`] reads the file
//! again and writes back only the books this run changed, atomically.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DIR: &str = ".adds/NickelInkSync";
const FILE: &str = "state.json";
const NOT_IN_LINE: &str = "not-in-line.tsv";
/// The format of `state.json`; a newer one is not read.
pub const VERSION: u32 = 1;

/// The position last agreed with the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Base {
  /// The KOSync document the book synced under; a base for another one
  /// (the book was mapped since) does not count.
  pub document: String,
  /// Nickel's `ChapterIDBookmarked` at that time: while it holds this, the
  /// device has not moved.
  pub device: String,
  /// The server's position (KOReader XPointer).
  pub xpointer: String,
  /// 0.0 to 1.0.
  pub percentage: f64,
  /// When the server stored it, Unix seconds.
  pub timestamp: i64,
}

/// A push from the device that still has to reach the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pending {
  /// When the first push failed, Unix seconds.
  pub since: i64,
  /// Why it failed: `timeout`, `network` or `server`.
  pub reason: String,
}

/// What `state.json` holds about one book.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
  /// For the plugin's list of pending books.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub base: Option<Base>,
  /// Where Nickel opened the book instead of the server's position.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub not_in_line: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub pending: Option<Pending>,
}

impl Entry {
  fn is_empty(&self) -> bool {
    self.base.is_none() && self.not_in_line.is_none() && self.pending.is_none()
  }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StateFile {
  version: u32,
  #[serde(default)]
  books: BTreeMap<String, Entry>,
}

#[derive(Debug, Default)]
pub struct State {
  dir: PathBuf,
  books: BTreeMap<String, Entry>,
  /// Books changed by this run, to write back.
  changed: BTreeSet<String>,
  /// The lines of `not-in-line.tsv` when it was read.
  not_in_line_file: Option<BTreeMap<String, String>>,
}

impl State {
  /// The state of the device mounted at `root`, with the books of
  /// `not-in-line.tsv` taken over; empty if there is neither file.
  pub fn load(root: &Path) -> io::Result<Self> {
    let dir = root.join(DIR);
    let mut state = State {
      books: read(&dir.join(FILE))?,
      not_in_line_file: read_not_in_line(&dir.join(NOT_IN_LINE))?,
      dir,
      changed: BTreeSet::new(),
    };
    for (id, at) in state.not_in_line_file.clone().unwrap_or_default() {
      state.entry(&id).not_in_line = Some(at);
    }
    Ok(state)
  }

  /// Where `state.json` is.
  pub fn path(&self) -> PathBuf {
    self.dir.join(FILE)
  }

  pub fn get(&self, content_id: &str) -> Option<&Entry> {
    self.books.get(content_id)
  }

  /// Where Nickel opened `content_id`, if it is not in line.
  pub fn not_in_line(&self, content_id: &str) -> Option<&str> {
    self.get(content_id)?.not_in_line.as_deref()
  }

  /// The base of `content_id` for `document`, if it has one.
  pub fn base(&self, content_id: &str, document: &str) -> Option<&Base> {
    self
      .get(content_id)?
      .base
      .as_ref()
      .filter(|b| b.document == document)
  }

  /// The books with a pending push, by ContentID.
  pub fn pending(&self) -> impl Iterator<Item = (&str, &Entry)> {
    self
      .books
      .iter()
      .filter(|(_, e)| e.pending.is_some())
      .map(|(id, e)| (id.as_str(), e))
  }

  fn entry(&mut self, content_id: &str) -> &mut Entry {
    self.changed.insert(content_id.to_string());
    self.books.entry(content_id.to_string()).or_default()
  }

  /// The book and the server agree on `base` now: it is in line, and
  /// nothing is pending.
  pub fn agreed(&mut self, content_id: &str, title: Option<&str>, base: Base) {
    let entry = self.entry(content_id);
    entry.base = Some(base);
    entry.not_in_line = None;
    entry.pending = None;
    if title.is_some() {
      entry.title = title.map(str::to_string);
    }
  }

  /// Takes books off the not-in-line list (their position was written).
  pub fn clear_not_in_line<'a>(&mut self, content_ids: impl IntoIterator<Item = &'a str>) {
    for id in content_ids {
      if self.not_in_line(id).is_some() {
        self.entry(id).not_in_line = None;
      }
    }
  }

  /// A push of `content_id` failed for `reason`: keep it for later. A book
  /// already pending keeps its first time.
  pub fn mark_pending(&mut self, content_id: &str, title: Option<&str>, reason: &str, now: i64) {
    let entry = self.entry(content_id);
    let since = entry.pending.as_ref().map_or(now, |p| p.since);
    entry.pending = Some(Pending {
      since,
      reason: reason.to_string(),
    });
    if title.is_some() {
      entry.title = title.map(str::to_string);
    }
  }

  /// Nothing more to send for `content_id`.
  pub fn clear_pending(&mut self, content_id: &str) {
    if self.get(content_id).is_some_and(|e| e.pending.is_some()) {
      self.entry(content_id).pending = None;
    }
  }

  /// Forgets `content_id` (the book is no longer on the device).
  pub fn remove(&mut self, content_id: &str) {
    if self.books.remove(content_id).is_some() {
      self.changed.insert(content_id.to_string());
    }
  }

  /// Writes the books this run changed into `state.json`, over what is
  /// there now, and removes `not-in-line.tsv` once taken over. Does
  /// nothing if nothing changed.
  pub fn save(&mut self) -> io::Result<()> {
    if self.changed.is_empty() {
      return Ok(());
    }
    fs::create_dir_all(&self.dir)?;
    let path = self.path();
    let mut books = read(&path)?;
    for id in &self.changed {
      match self.books.get(id).filter(|e| !e.is_empty()) {
        Some(entry) => books.insert(id.clone(), entry.clone()),
        None => books.remove(id),
      };
    }
    // Lines the plugin added to not-in-line.tsv since it was read.
    let list = self.dir.join(NOT_IN_LINE);
    let seen = self.not_in_line_file.take().unwrap_or_default();
    let now = read_not_in_line(&list)?;
    for (id, at) in now.iter().flatten() {
      if seen.get(id) != Some(at) {
        books.entry(id.clone()).or_default().not_in_line = Some(at.clone());
      }
    }
    let file = StateFile {
      version: VERSION,
      books,
    };
    let mut text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    text.push('\n');
    let tmp = self.dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    let written = (|| {
      let mut out = fs::File::create(&tmp)?;
      out.write_all(text.as_bytes())?;
      out.sync_all()
    })();
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, &path)) {
      let _ = fs::remove_file(&tmp);
      return Err(e);
    }
    self.books = file.books;
    self.changed.clear();
    if now.is_some() {
      match fs::remove_file(&list) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
      }
    }
    Ok(())
  }
}

/// The books in `state.json`; none if there is no file.
fn read(path: &Path) -> io::Result<BTreeMap<String, Entry>> {
  let text = match fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
    Err(e) => return Err(e),
  };
  let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
  let file: StateFile = serde_json::from_str(&text)
    .map_err(|e| invalid(format!("{} is not readable: {e}", path.display())))?;
  if file.version > VERSION {
    return Err(invalid(format!(
      "{} has format {}, written by a newer NickelInkSync",
      path.display(),
      file.version
    )));
  }
  Ok(file.books)
}

/// The plugin's old list, `ContentID<tab>ChapterIDBookmarked` per line;
/// `None` if there is none.
fn read_not_in_line(path: &Path) -> io::Result<Option<BTreeMap<String, String>>> {
  let text = match fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
    Err(e) => return Err(e),
  };
  Ok(Some(
    text
      .lines()
      .filter_map(|line| {
        let (id, position) = line.split_once('\t')?;
        Some((id.to_string(), position.trim_end_matches('\r').to_string()))
      })
      .filter(|(id, _)| !id.is_empty())
      .collect(),
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn device(name: &str) -> PathBuf {
    let root =
      std::env::temp_dir().join(format!("nickelinksync-state-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(DIR)).unwrap();
    root
  }

  fn base(device: &str) -> Base {
    Base {
      document: "d0c".into(),
      device: device.into(),
      xpointer: "/body[1]/DocFragment[3]/body[1]/p[1]/text()[1].0".into(),
      percentage: 0.25,
      timestamp: 1_800_000_000,
    }
  }

  const A: &str = "file:///mnt/onboard/a.epub";
  const B: &str = "file:///mnt/onboard/b.epub";

  #[test]
  fn missing_files_are_an_empty_state() {
    let root = device("missing");
    let mut state = State::load(&root).unwrap();
    assert_eq!(state.get(A), None);
    assert_eq!(state.pending().count(), 0);
    // Nothing changed: nothing written.
    state.save().unwrap();
    assert!(!state.path().exists());
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn saves_and_loads_back() {
    let root = device("save");
    let mut state = State::load(&root).unwrap();
    state.agreed(A, Some("Book A"), base("c3.xhtml#point(/1/4/2/1:0)"));
    state.mark_pending(B, None, "network", 100);
    state.mark_pending(B, None, "timeout", 200);
    state.save().unwrap();
    let text = fs::read_to_string(state.path()).unwrap();
    assert!(text.starts_with("{\n  \"version\": 1,"), "{text}");
    let state = State::load(&root).unwrap();
    assert_eq!(state.base(A, "d0c").unwrap().percentage, 0.25);
    assert_eq!(state.base(A, "other"), None);
    assert_eq!(state.get(A).unwrap().title.as_deref(), Some("Book A"));
    let pending: Vec<_> = state.pending().collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(
      pending[0].1.pending,
      Some(Pending {
        since: 100,
        reason: "timeout".into()
      })
    );
    // No temporary file left behind.
    assert_eq!(fs::read_dir(root.join(DIR)).unwrap().count(), 1);
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn writes_back_only_its_own_books() {
    let root = device("overlap");
    let mut first = State::load(&root).unwrap();
    let mut second = State::load(&root).unwrap();
    first.mark_pending(A, None, "network", 1);
    first.save().unwrap();
    second.agreed(B, None, base("x"));
    second.save().unwrap();
    let state = State::load(&root).unwrap();
    assert!(state.get(A).unwrap().pending.is_some());
    assert!(state.get(B).unwrap().base.is_some());
    // An entry with nothing left goes.
    let mut state = state;
    state.clear_pending(A);
    state.save().unwrap();
    assert_eq!(State::load(&root).unwrap().get(A), None);
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn takes_over_not_in_line_tsv() {
    let root = device("tsv");
    let list = root.join(DIR).join(NOT_IN_LINE);
    fs::write(
      &list,
      format!("{A}\tOEBPS/c3.xhtml#point(/1/4/2/1:0)\r\n{B}\tOEBPS/c1.xhtml\njunk without a tab\n"),
    )
    .unwrap();
    let mut state = State::load(&root).unwrap();
    assert_eq!(
      state.not_in_line(A),
      Some("OEBPS/c3.xhtml#point(/1/4/2/1:0)")
    );
    // Reading changes nothing on the device.
    assert!(list.exists());
    assert!(!state.path().exists());
    // A's position was written: in line again. B stays, now in state.json.
    state.clear_not_in_line([A]);
    // Meanwhile the plugin puts another book on the old list.
    fs::write(
      &list,
      format!("{A}\tOEBPS/c3.xhtml#point(/1/4/2/1:0)\n{B}\tOEBPS/c1.xhtml\nfile:///c\tc.xhtml\n"),
    )
    .unwrap();
    state.save().unwrap();
    assert!(!list.exists());
    let state = State::load(&root).unwrap();
    assert_eq!(state.not_in_line(A), None);
    assert_eq!(state.not_in_line(B), Some("OEBPS/c1.xhtml"));
    assert_eq!(state.not_in_line("file:///c"), Some("c.xhtml"));
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn agreeing_clears_not_in_line_and_pending() {
    let root = device("agree");
    let mut state = State::load(&root).unwrap();
    state.mark_pending(A, None, "server", 1);
    state.entry(A).not_in_line = Some("x".into());
    state.agreed(A, None, base("y"));
    let entry = state.get(A).unwrap();
    assert_eq!(
      (entry.pending.as_ref(), entry.not_in_line.as_ref()),
      (None, None)
    );
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn refuses_a_newer_format() {
    let root = device("newer");
    fs::write(root.join(DIR).join(FILE), "{\"version\": 2, \"books\": {}}").unwrap();
    let e = State::load(&root).unwrap_err();
    assert!(e.to_string().contains("newer NickelInkSync"), "{e}");
    fs::write(root.join(DIR).join(FILE), "{broken").unwrap();
    assert!(State::load(&root).is_err());
    fs::remove_dir_all(&root).unwrap();
  }
}
