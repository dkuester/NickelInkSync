//! Books the device plugin could not bring in line with the server: it
//! wrote the server's position, but Nickel opened the book elsewhere (the
//! chapter start) and saves that place, with a new `DateLastRead`, when the
//! book is left. The device position is then Nickel's, not the reader's,
//! and looks newer than the server's.
//!
//! So such a book is never pushed (`--force` does), and a pull ignores its
//! `DateLastRead`: while the row still holds exactly the place Nickel
//! opened at, the server's position is taken without asking; after reading
//! on from there, the place in the book decides as usual.
//!
//! The list is `<device>/.adds/NickelInkSync/not-in-line.tsv`, one line
//! `ContentID<tab>ChapterIDBookmarked` per book. The plugin adds a book when
//! the check after opening fails and removes it when it succeeds; the CLI
//! (on the device and on the desktop) reads it and removes the books it
//! writes with `pull --write` or sends with `push --force`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const FILE: &str = ".adds/NickelInkSync/not-in-line.tsv";

#[derive(Debug, Default)]
pub struct NotInLine {
  path: PathBuf,
  /// ContentID → the position Nickel opened at.
  entries: BTreeMap<String, String>,
}

impl NotInLine {
  /// The list of the device mounted at `root`; empty if there is none.
  pub fn load(root: &Path) -> io::Result<Self> {
    let path = root.join(FILE);
    let text = match std::fs::read_to_string(&path) {
      Ok(text) => text,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };
    let entries = text
      .lines()
      .filter_map(|line| {
        let (id, position) = line.split_once('\t')?;
        Some((id.to_string(), position.trim_end_matches('\r').to_string()))
      })
      .filter(|(id, _)| !id.is_empty())
      .collect();
    Ok(NotInLine { path, entries })
  }

  /// Where Nickel opened `content_id`, if the book is on the list.
  pub fn get(&self, content_id: &str) -> Option<&str> {
    self.entries.get(content_id).map(String::as_str)
  }

  /// Takes books off the list and saves it (removes the file once empty).
  /// Does nothing if none of them is on it.
  pub fn remove<'a>(&mut self, content_ids: impl IntoIterator<Item = &'a str>) -> io::Result<()> {
    let before = self.entries.len();
    for id in content_ids {
      self.entries.remove(id);
    }
    if self.entries.len() == before {
      return Ok(());
    }
    if self.entries.is_empty() {
      return match std::fs::remove_file(&self.path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
      };
    }
    let text: String = self
      .entries
      .iter()
      .map(|(id, position)| format!("{id}\t{position}\n"))
      .collect();
    std::fs::write(&self.path, text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn device(name: &str) -> PathBuf {
    let root =
      std::env::temp_dir().join(format!("nickelinksync-nil-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".adds/NickelInkSync")).unwrap();
    root
  }

  #[test]
  fn missing_file_is_an_empty_list() {
    let root = device("missing");
    let list = NotInLine::load(&root).unwrap();
    assert_eq!(list.get("file:///mnt/onboard/a.epub"), None);
    std::fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn reads_and_removes_entries() {
    let root = device("entries");
    std::fs::write(
      root.join(FILE),
      "file:///mnt/onboard/a.epub\tOEBPS/c3.xhtml#point(/1/4/2/1:0)\r\n\
       file:///mnt/onboard/b.epub\tOEBPS/c1.xhtml#point(/1/4/2/1:0)\n\
       junk without a tab\n",
    )
    .unwrap();
    let mut list = NotInLine::load(&root).unwrap();
    assert_eq!(
      list.get("file:///mnt/onboard/a.epub"),
      Some("OEBPS/c3.xhtml#point(/1/4/2/1:0)")
    );
    list.remove(["file:///mnt/onboard/a.epub"]).unwrap();
    assert_eq!(
      std::fs::read_to_string(root.join(FILE)).unwrap(),
      "file:///mnt/onboard/b.epub\tOEBPS/c1.xhtml#point(/1/4/2/1:0)\n"
    );
    list.remove(["file:///mnt/onboard/b.epub"]).unwrap();
    assert!(!root.join(FILE).exists());
    std::fs::remove_dir_all(&root).unwrap();
  }
}
