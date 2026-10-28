//! A Kindle mounted over USB: its KFX books and their metadata.
//!
//! Only what a stock Kindle (no jailbreak) shows over USB: the files under
//! `documents/`. Send-to-Kindle stores a book as
//! `documents/Downloads/Items01/<title>_<content_id>.kfx`, with its reading
//! state in the `.sdr` folder of the same name (see SCOPE.md, Kindle).
//! Nothing here writes to the device.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::kfx::Kfx;

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
}

impl KindleBook {
  /// The book's `.sdr` folder (reading position, highlights).
  pub fn sdr(&self) -> PathBuf {
    self.kfx.with_extension("sdr")
  }

  /// The path below the mount point, for display.
  pub fn relative_path<'a>(&'a self, root: &Path) -> &'a Path {
    self.kfx.strip_prefix(root).unwrap_or(&self.kfx)
  }
}

/// A mounted Kindle: a `documents` folder and no `.kobo` folder.
pub fn is_kindle(root: &Path) -> bool {
  root.join("documents").is_dir() && !root.join(".kobo").exists()
}

/// A KFX that could not be read, and why.
pub type Unreadable = (PathBuf, String);

/// Every KFX under `documents/`, sorted by path, and the files that could
/// not be read.
pub fn books(root: &Path) -> io::Result<(Vec<KindleBook>, Vec<Unreadable>)> {
  let mut files = Vec::new();
  collect_kfx(&root.join("documents"), &mut files)?;
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
