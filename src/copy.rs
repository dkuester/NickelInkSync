//! Copying a book from the library onto a mounted Kobo or Tolino, into
//! `Books/`: the EPUB as it is, or on a Kobo a kepub made by kepubify,
//! mapped to the EPUB so it syncs with KOReader at once. The library is
//! only read, and nothing on the device is overwritten: a file of the same
//! name, or the same EPUB already in `Books/`, is left as it is.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hash::partial_md5;
use crate::map;
use crate::mapping::{DeviceBook, Mapping, Store};
use crate::sync::iso_timestamp;

/// The folder on the device the books go into.
pub const BOOKS: &str = "Books";

/// How a book goes onto the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
  /// The EPUB file as it is.
  Epub,
  /// Converted by kepubify (Kobo), and mapped to the EPUB.
  Kepub,
}

/// What became of one book.
#[derive(Debug, Clone, PartialEq)]
pub enum Copied {
  /// Copied to this path on the device (`Books/…`); `mapped` when a
  /// mapping to the original was made.
  Done {
    path: String,
    mapped: bool,
  },
  /// A file of this name is already on the device; left as it is.
  NameTaken(String),
  /// The same EPUB is already on the device, at this path.
  AlreadyThere(String),
  Failed(String),
}

/// The kepubify program: `$NICKELINKSYNC_KEPUBIFY`, else the one shipped
/// next to the program (the macOS app, the Windows ZIP), else `kepubify` in
/// the `PATH`, in Homebrew's folders or in `~/go/bin` (`go install`); an
/// app started from the Finder does not get the shell's `PATH`.
pub fn kepubify() -> Option<PathBuf> {
  if let Some(path) = std::env::var_os("NICKELINKSYNC_KEPUBIFY").filter(|v| !v.is_empty()) {
    return Some(PathBuf::from(path));
  }
  let name = if cfg!(windows) {
    "kepubify.exe"
  } else {
    "kepubify"
  };
  let shipped = std::env::current_exe()
    .ok()
    .and_then(|exe| Some(exe.parent()?.join(name)))
    .filter(|p| p.is_file());
  if shipped.is_some() {
    return shipped;
  }
  let path = std::env::var_os("PATH").unwrap_or_default();
  let go = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("go/bin"));
  std::env::split_paths(&path)
    .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from))
    .chain(go)
    .map(|dir| dir.join(name))
    .find(|p| p.is_file())
}

/// Copies the EPUB at `epub` onto the device at `root`. A kepub needs
/// `kepubify` and goes into `store` as mapped to `epub`.
pub fn copy(
  root: &Path,
  epub: &Path,
  format: Format,
  kepubify: Option<&Path>,
  store: &mut Store,
) -> Copied {
  match copy_book(root, epub, format, kepubify, store) {
    Ok(copied) => copied,
    Err(e) => Copied::Failed(e),
  }
}

fn copy_book(
  root: &Path,
  epub: &Path,
  format: Format,
  kepubify: Option<&Path>,
  store: &mut Store,
) -> Result<Copied, String> {
  let fail = |e: io::Error| format!("{}: {e}", epub.display());
  let file_name = epub
    .file_name()
    .and_then(|n| n.to_str())
    .ok_or_else(|| format!("{}: no file name", epub.display()))?;
  let books = root.join(BOOKS);
  fs::create_dir_all(&books).map_err(|e| format!("{}: {e}", books.display()))?;
  let name = match format {
    Format::Epub => file_name.to_string(),
    Format::Kepub => {
      let stem = file_name.strip_suffix(".epub").unwrap_or(file_name);
      format!("{stem}.kepub.epub")
    }
  };
  let on_device = format!("{BOOKS}/{name}");
  if books.join(&name).exists() {
    return Ok(Copied::NameTaken(on_device));
  }
  if format == Format::Epub {
    if let Some(there) = same_file(&books, epub).map_err(fail)? {
      return Ok(Copied::AlreadyThere(format!("{BOOKS}/{there}")));
    }
  }
  let source = match format {
    Format::Epub => epub.to_path_buf(),
    Format::Kepub => {
      let program = kepubify.ok_or("kepubify was not found")?;
      convert(program, epub)?
    }
  };
  let target = books.join(&name);
  let written = write_new(&source, &target);
  if format == Format::Kepub {
    if let Some(dir) = source.parent() {
      let _ = fs::remove_dir_all(dir);
    }
  }
  written.map_err(|e| format!("{}: {e}", target.display()))?;
  let mapped = match format {
    Format::Epub => false,
    Format::Kepub => {
      map_kepub(store, &target, &on_device, epub)?;
      true
    }
  };
  Ok(Copied::Done {
    path: on_device,
    mapped,
  })
}

/// The name of a file in `books` with the same content as `epub`.
fn same_file(books: &Path, epub: &Path) -> io::Result<Option<String>> {
  let size = fs::metadata(epub)?.len();
  let mut hash = None;
  for entry in fs::read_dir(books)? {
    let entry = entry?;
    let meta = entry.metadata()?;
    if !meta.is_file() || meta.len() != size {
      continue;
    }
    let own = match &hash {
      Some(h) => h,
      None => hash.insert(partial_md5(epub)?),
    };
    if partial_md5(&entry.path())? == *own {
      return Ok(Some(entry.file_name().to_string_lossy().into_owned()));
    }
  }
  Ok(None)
}

/// Runs kepubify on `epub` into a folder of its own; the kepub's path.
fn convert(program: &Path, epub: &Path) -> Result<PathBuf, String> {
  let dir = std::env::temp_dir().join(format!(
    "nickelinksync-kepubify-{}-{}",
    std::process::id(),
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0)
  ));
  fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
  let output = Command::new(program)
    .arg("-o")
    .arg(&dir)
    .arg(epub)
    .output()
    .map_err(|e| format!("{}: {e}", program.display()));
  let made = output.and_then(|output| {
    if !output.status.success() {
      let err = String::from_utf8_lossy(&output.stderr);
      return Err(format!("kepubify failed: {}", err.trim()));
    }
    let found = fs::read_dir(&dir)
      .map_err(|e| e.to_string())?
      .filter_map(Result::ok)
      .map(|e| e.path())
      .find(|p| p.to_string_lossy().ends_with(".kepub.epub"));
    found.ok_or_else(|| "kepubify made no kepub".to_string())
  });
  if made.is_err() {
    let _ = fs::remove_dir_all(&dir);
  }
  made
}

/// Copies `source` to `target`, which must not exist yet; a partial file
/// is removed.
fn write_new(source: &Path, target: &Path) -> io::Result<()> {
  let mut from = File::open(source)?;
  let mut to = File::create_new(target)?;
  let copied = io::copy(&mut from, &mut to).and_then(|_| to.sync_all());
  if copied.is_err() {
    let _ = fs::remove_file(target);
  }
  copied
}

/// Maps the kepub at `target` (`on_device` on the device) to `epub`.
fn map_kepub(store: &mut Store, target: &Path, on_device: &str, epub: &Path) -> Result<(), String> {
  let (original, _) = map::original(epub).map_err(|e| e.to_string())?;
  let hash = partial_md5(target).map_err(|e| format!("{}: {e}", target.display()))?;
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  store.insert(Mapping {
    device: DeviceBook::File {
      hash,
      path: on_device.to_string(),
      title: original.title.clone(),
      author: original.author.clone(),
    },
    epub: original,
    created: iso_timestamp(now),
  });
  store.save().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn epub() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epub/book-a.epub")
  }

  fn temp(name: &str) -> PathBuf {
    let dir =
      std::env::temp_dir().join(format!("nickelinksync-{}-copy-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
  }

  #[test]
  fn an_epub_is_copied_once_and_nothing_is_overwritten() {
    let root = temp("epub");
    let mut store = Store::load(&root.join("mappings.json")).unwrap();
    assert_eq!(
      copy(&root, &epub(), Format::Epub, None, &mut store),
      Copied::Done {
        path: "Books/book-a.epub".into(),
        mapped: false
      }
    );
    assert_eq!(
      fs::read(root.join("Books/book-a.epub")).unwrap(),
      fs::read(epub()).unwrap()
    );
    assert_eq!(
      copy(&root, &epub(), Format::Epub, None, &mut store),
      Copied::NameTaken("Books/book-a.epub".into())
    );
    // The same file under another name counts as there.
    fs::rename(
      root.join("Books/book-a.epub"),
      root.join("Books/renamed.epub"),
    )
    .unwrap();
    assert_eq!(
      copy(&root, &epub(), Format::Epub, None, &mut store),
      Copied::AlreadyThere("Books/renamed.epub".into())
    );
    assert!(!root.join("Books/book-a.epub").exists());
    assert!(store.mappings.is_empty());
    fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn a_kepub_without_kepubify_is_not_copied() {
    let root = temp("no-kepubify");
    let mut store = Store::load(&root.join("mappings.json")).unwrap();
    assert!(matches!(
      copy(&root, &epub(), Format::Kepub, None, &mut store),
      Copied::Failed(_)
    ));
    assert_eq!(fs::read_dir(root.join("Books")).unwrap().count(), 0);
    fs::remove_dir_all(&root).unwrap();
  }

  /// A stand-in for kepubify: copies the EPUB and appends a byte, so the
  /// kepub has a checksum of its own.
  #[cfg(unix)]
  #[test]
  fn a_kepub_is_made_by_kepubify_and_mapped() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp("kepub");
    let program = root.join("kepubify");
    fs::write(
      &program,
      "#!/bin/sh\n# -o DIR FILE\nname=$(basename \"$3\" .epub)\ncp \"$3\" \"$2/$name.kepub.epub\"\nprintf x >> \"$2/$name.kepub.epub\"\n",
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let mut store = Store::load(&root.join("mappings.json")).unwrap();
    assert_eq!(
      copy(&root, &epub(), Format::Kepub, Some(&program), &mut store),
      Copied::Done {
        path: "Books/book-a.kepub.epub".into(),
        mapped: true
      }
    );
    let kepub = root.join("Books/book-a.kepub.epub");
    let mapping = store.for_file(&partial_md5(&kepub).unwrap()).unwrap();
    assert_eq!(mapping.epub.hash, partial_md5(&epub()).unwrap());
    assert!(
      matches!(&mapping.device, DeviceBook::File { path, .. } if path == "Books/book-a.kepub.epub")
    );
    // Saved for the next run.
    let saved = Store::load(&root.join("mappings.json")).unwrap();
    assert_eq!(saved.mappings.len(), 1);
    assert_eq!(
      copy(&root, &epub(), Format::Kepub, Some(&program), &mut store),
      Copied::NameTaken("Books/book-a.kepub.epub".into())
    );
    fs::remove_dir_all(&root).unwrap();
  }
}
