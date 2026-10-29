//! Finding devices: a Kobo or Tolino mounted as a USB drive, a Kindle that
//! still mounts as one, and the chosen copy of a Kindle's books. Finding a
//! device only lists it; nothing is read or synced until it is chosen.

use std::path::{Path, PathBuf};

use nickelinksync::kindle_device;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  /// Kobo or Tolino: `.kobo/KoboReader.sqlite`.
  Nickel,
  /// A mounted Kindle or a copy of its books.
  Kindle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
  pub root: PathBuf,
  pub kind: Kind,
  /// The volume or folder name.
  pub name: String,
}

impl Found {
  fn new(root: PathBuf, kind: Kind) -> Self {
    let name = root
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_else(|| root.display().to_string());
    Found { root, kind, name }
  }
}

/// Where the system mounts removable drives.
pub fn mount_parents() -> Vec<PathBuf> {
  let mut parents = Vec::new();
  if cfg!(target_os = "macos") {
    parents.push(PathBuf::from("/Volumes"));
  } else if cfg!(target_os = "linux") {
    if let Some(user) = std::env::var_os("USER") {
      parents.push(Path::new("/media").join(&user));
      parents.push(Path::new("/run/media").join(&user));
    }
  }
  parents
}

/// Drive roots that are mount points themselves (Windows drive letters).
pub fn drive_roots() -> Vec<PathBuf> {
  if cfg!(windows) {
    ('D'..='Z')
      .map(|d| PathBuf::from(format!("{d}:\\")))
      .collect()
  } else {
    Vec::new()
  }
}

pub fn is_nickel(root: &Path) -> bool {
  root.join(".kobo/KoboReader.sqlite").is_file()
}

/// A Kindle that mounts as a drive (older models): `documents` and
/// `system`. Other drives are not searched for KFX files.
pub fn is_mounted_kindle(root: &Path) -> bool {
  root.join("documents").is_dir() && root.join("system").is_dir() && !root.join(".kobo").exists()
}

/// The devices among the children of `parents` and among `roots`, sorted
/// by name. Symbolic links are skipped (on a Mac `/Volumes` links to the
/// system disk).
pub fn scan(parents: &[PathBuf], roots: &[PathBuf]) -> Vec<Found> {
  let mut candidates: Vec<PathBuf> = roots.to_vec();
  for parent in parents {
    let Ok(entries) = std::fs::read_dir(parent) else {
      continue;
    };
    for entry in entries.flatten() {
      if entry.file_type().is_ok_and(|t| t.is_dir()) {
        candidates.push(entry.path());
      }
    }
  }
  let mut found: Vec<Found> = candidates
    .into_iter()
    .filter_map(|root| {
      if is_nickel(&root) {
        Some(Found::new(root, Kind::Nickel))
      } else if is_mounted_kindle(&root) {
        Some(Found::new(root, Kind::Kindle))
      } else {
        None
      }
    })
    .collect();
  found.sort_by(|a, b| a.name.cmp(&b.name));
  found
}

/// The chosen copy of a Kindle's books, if it is one.
pub fn kindle_folder(path: &Path) -> Option<Found> {
  kindle_device::is_kindle(path).then(|| Found::new(path.to_path_buf(), Kind::Kindle))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn finds_nickel_and_kindle_drives_only() {
    let base = std::env::temp_dir().join(format!("nickelinksync-gui-{}-scan", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let volumes = base.join("Volumes");
    fs::create_dir_all(volumes.join("TOLINO/.kobo")).unwrap();
    fs::write(volumes.join("TOLINO/.kobo/KoboReader.sqlite"), "").unwrap();
    fs::create_dir_all(volumes.join("Kindle/documents")).unwrap();
    fs::create_dir_all(volumes.join("Kindle/system")).unwrap();
    // A stick with a documents folder is no Kindle.
    fs::create_dir_all(volumes.join("STICK/documents")).unwrap();
    fs::write(volumes.join("file.txt"), "").unwrap();
    let found = scan(&[volumes.clone(), base.join("missing")], &[]);
    assert_eq!(
      found
        .iter()
        .map(|f| (f.name.as_str(), f.kind))
        .collect::<Vec<_>>(),
      [("Kindle", Kind::Kindle), ("TOLINO", Kind::Nickel)]
    );
    assert_eq!(found[1].root, volumes.join("TOLINO"));
    // A drive root counts like a child.
    let found = scan(&[], &[volumes.join("TOLINO"), volumes.join("STICK")]);
    assert_eq!(found.len(), 1);
    fs::remove_dir_all(&base).unwrap();
  }

  #[test]
  fn kindle_copy_needs_kfx_or_documents() {
    let base = std::env::temp_dir().join(format!("nickelinksync-gui-{}-kfx", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("Items01")).unwrap();
    assert_eq!(kindle_folder(&base), None);
    fs::write(base.join("Items01/book.kfx"), "").unwrap();
    assert_eq!(kindle_folder(&base).map(|f| f.kind), Some(Kind::Kindle));
    fs::remove_dir_all(&base).unwrap();
  }
}
