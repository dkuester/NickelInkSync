//! A Kindle mounted over USB: its KFX books and their metadata.
//!
//! Only what a stock Kindle (no jailbreak) shows over USB: the files under
//! `documents/`. Send-to-Kindle stores a book as
//! `documents/Downloads/Items01/<title>_<content_id>.kfx`, with its reading
//! state in the `.sdr` folder of the same name (see SCOPE.md, Kindle).
//! Recent Kindles offer only MTP, which a Mac does not mount, so a copy
//! works as well: a folder with `documents/`, or any folder with the KFX
//! files and their `.sdr` folders (copied with OpenMTP, for example).
//! Nothing here writes to the device.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::kfx::Kfx;
use crate::krds::{self, ReadingState};

/// A KFX book on the Kindle.
#[derive(Debug, Clone, PartialEq)]
pub struct KindleBook {
  pub kfx: PathBuf,
  /// `content_id` from the metadata (else the 32 hex digits at the end of
  /// the file name), upper case.
  pub content_id: String,
  /// `title`: for Send-to-Kindle the file stem it was sent under, not the
  /// EPUB's `dc:title`.
  pub title: Option<String>,
  pub author: Option<String>,
  /// Number of KFX sections (one per spine item of the EPUB it was made
  /// from).
  pub sections: usize,
}

impl KindleBook {
  /// The book's `.sdr` folder (reading position, highlights).
  pub fn sdr(&self) -> PathBuf {
    self.kfx.with_extension("sdr")
  }

  /// The reading state from the `.yjf` and `.yjr` files in the `.sdr`
  /// folder (not the `.bad_file` copies the Kindle leaves); `None` if
  /// there are none, as for a book never opened.
  pub fn reading_state(&self) -> Result<Option<ReadingState>, String> {
    let sdr = self.sdr();
    let Ok(entries) = fs::read_dir(&sdr) else {
      return Ok(None);
    };
    let mut objects = Vec::new();
    let mut found = false;
    for entry in entries {
      let path = entry.map_err(|e| e.to_string())?.path();
      let name = path.file_name().unwrap_or_default().to_string_lossy();
      if name.starts_with('.') || !(name.ends_with(".yjf") || name.ends_with(".yjr")) {
        continue;
      }
      let data = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
      objects.extend(krds::parse(&data).map_err(|e| format!("{}: {e}", path.display()))?);
      found = true;
    }
    Ok(found.then(|| ReadingState::from_objects(&objects)))
  }

  /// The path below the mount point, for display.
  pub fn relative_path<'a>(&'a self, root: &Path) -> &'a Path {
    self.kfx.strip_prefix(root).unwrap_or(&self.kfx)
  }
}

/// A mounted Kindle or a copy of its books: no `.kobo` folder, and a
/// `documents` folder or KFX files up to two folders down (a copy of
/// `Downloads/Items01`).
pub fn is_kindle(root: &Path) -> bool {
  !root.join(".kobo").exists() && (root.join("documents").is_dir() || has_kfx(root, 2))
}

/// Where the books are: `documents/`, else the folder itself.
pub fn documents(root: &Path) -> PathBuf {
  let documents = root.join("documents");
  if documents.is_dir() {
    documents
  } else {
    root.to_path_buf()
  }
}

fn has_kfx(dir: &Path, depth: usize) -> bool {
  let Ok(entries) = fs::read_dir(dir) else {
    return false;
  };
  entries.flatten().any(|e| {
    let name = e.file_name().to_string_lossy().to_lowercase();
    if name.starts_with('.') {
      false
    } else if e.file_type().is_ok_and(|t| t.is_dir()) {
      depth > 0 && !name.ends_with(".sdr") && has_kfx(&e.path(), depth - 1)
    } else {
      name.ends_with(".kfx")
    }
  })
}

/// A KFX that could not be read, and why.
pub type Unreadable = (PathBuf, String);

/// Every KFX under `documents/`, sorted by path, and the files that could
/// not be read.
pub fn books(root: &Path) -> io::Result<(Vec<KindleBook>, Vec<Unreadable>)> {
  let mut files = Vec::new();
  collect_kfx(&documents(root), &mut files)?;
  files.sort();
  let (mut books, mut unreadable) = (Vec::new(), Vec::new());
  for kfx in files {
    match Kfx::open(&kfx) {
      Ok(parsed) => {
        let content_id = parsed
          .meta("content_id")
          .or_else(|| parsed.meta("ASIN"))
          .map(str::to_string)
          .or_else(|| id_from_name(&kfx));
        let Some(content_id) = content_id else {
          unreadable.push((kfx, "no content_id".into()));
          continue;
        };
        books.push(KindleBook {
          content_id: content_id.to_ascii_uppercase(),
          title: parsed.meta("title").map(str::to_string),
          author: parsed.meta("author").map(str::to_string),
          sections: parsed.sections.len(),
          kfx,
        });
      }
      Err(e) => unreadable.push((kfx, e.to_string())),
    }
  }
  Ok((books, unreadable))
}

fn collect_kfx(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    let name = entry.file_name();
    let name = name.to_string_lossy();
    // macOS AppleDouble files and the books' own state folders.
    if name.starts_with('.') {
      continue;
    }
    if entry.file_type()?.is_dir() {
      if !name.ends_with(".sdr") {
        collect_kfx(&path, out)?;
      }
    } else if name.to_ascii_lowercase().ends_with(".kfx") {
      out.push(path);
    }
  }
  Ok(())
}

/// The 32 hex digits after the last `_` of the file stem.
fn id_from_name(kfx: &Path) -> Option<String> {
  let stem = kfx.file_stem()?.to_str()?;
  let (_, id) = stem.rsplit_once('_')?;
  (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn id_from_the_file_name() {
    assert_eq!(
      id_from_name(Path::new("a/book-p_0000000000000000000000000000000a.kfx")).as_deref(),
      Some("0000000000000000000000000000000a")
    );
    assert_eq!(id_from_name(Path::new("a/book-p.kfx")), None);
    assert_eq!(id_from_name(Path::new("a/book_p.kfx")), None);
  }
}
