//! Copying a Kindle's books and reading states over MTP into a folder, read
//! only (release 0.2.0, stage M1; see `docs/kindle-mtp-feasibility.md`).
//!
//! A recent stock Kindle offers only MTP, which neither macOS nor Windows
//! mounts as a drive. This module copies what [`crate::kindle_device`] needs
//! from `documents/` (the `.sdr` folders, optionally the `.kfx` books) into a
//! plain folder, which then works as `--device`. Nothing is ever written to,
//! moved on or deleted from the Kindle: only listing and downloading calls
//! are used.
//!
//! The planning (which names are safe, which objects to fetch, what a cached
//! copy is) is plain code and tested here; the USB part is behind the `mtp`
//! feature, which needs the `mtp-rs` crate.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Files above this size are never fetched (a Kindle book is a few MB).
pub const MAX_FILE_BYTES: u64 = 300 * 1024 * 1024;

/// What to fetch from `documents/`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wanted {
  /// Also the `.kfx` books (needed to map a book), not only the reading
  /// states.
  pub books: bool,
}

/// An object found on the Kindle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
  /// Names from `documents/` down, the object's own name last.
  pub path: Vec<String>,
  pub size: u64,
  pub is_folder: bool,
}

/// A name from the device that is safe as one path component: not empty,
/// no separator, not `.` or `..`, no control characters. The device is not
/// trusted: nothing it names may leave the destination folder.
pub fn safe_component(name: &str) -> Option<&str> {
  let bad = name.is_empty()
    || name == "."
    || name == ".."
    || name
      .chars()
      .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    || name.ends_with(' ')
    || name.ends_with('.') && name != ".";
  (!bad).then_some(name)
}

/// The local path of an entry below `dest`, or `None` if any name is unsafe.
pub fn local_path(dest: &Path, path: &[String]) -> Option<PathBuf> {
  let mut out = dest.join("documents");
  for name in path {
    out.push(safe_component(name)?);
  }
  Some(out)
}

fn is_sdr(name: &str) -> bool {
  name.to_lowercase().ends_with(".sdr")
}

impl Wanted {
  /// Whether the file at `path` (a file, with its folders before it) is
  /// fetched: everything inside a `.sdr` folder, and with `books` the
  /// `.kfx` files.
  pub fn file(&self, path: &[String]) -> bool {
    let Some((name, folders)) = path.split_last() else {
      return false;
    };
    folders.iter().any(|f| is_sdr(f)) || (self.books && name.to_lowercase().ends_with(".kfx"))
  }
}

/// A copy already on disk counts as current when the size is the same (the
/// Kindle's dates have no time zone; a reading state that changes its content
/// without changing its size is caught by `refresh`).
pub fn is_current(local: &Path, size: u64) -> bool {
  std::fs::metadata(local).is_ok_and(|m| m.is_file() && m.len() == size)
}

/// What a run found and did.
#[derive(Debug, Default)]
pub struct Report {
  /// The device, for the log.
  pub device: String,
  /// Names of the objects in the root of the storage.
  pub root: Vec<String>,
  pub folders: usize,
  pub files: usize,
  /// `.sdr` folders seen: zero means the reading states are not visible.
  pub sdr_folders: usize,
  pub kfx_files: usize,
  pub fetched: usize,
  pub fetched_bytes: u64,
  /// Already there with the same size.
  pub cached: usize,
  /// Skipped, with the reason.
  pub skipped: Vec<(String, String)>,
  /// The entries, for `--list`.
  pub entries: Vec<Entry>,
}

impl Report {
  /// The text printed at the end.
  pub fn render(&self, list: bool) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Device: {}", self.device);
    let _ = writeln!(s, "Root of the storage: {}", self.root.join(", "));
    if list {
      for e in &self.entries {
        let _ = writeln!(
          s,
          "  {}{}{}",
          e.path.join("/"),
          if e.is_folder { "/" } else { "" },
          if e.is_folder {
            String::new()
          } else {
            format!("  ({} bytes)", e.size)
          }
        );
      }
    }
    let _ = writeln!(
      s,
      "Found {} folders, {} files, {} .sdr folders, {} .kfx books",
      self.folders, self.files, self.sdr_folders, self.kfx_files
    );
    if self.sdr_folders == 0 {
      let _ = writeln!(
        s,
        "No .sdr folder is visible over MTP: the reading positions cannot be read this way."
      );
    }
    if !list {
      let _ = writeln!(
        s,
        "Copied {} files ({} bytes), {} were already current",
        self.fetched, self.fetched_bytes, self.cached
      );
    }
    for (what, why) in &self.skipped {
      let _ = writeln!(s, "Skipped {what}: {why}");
    }
    s
  }
}

/// What to tell the user for an error from the library. Only explains; the
/// program never stops another process or changes a setting.
#[cfg(feature = "mtp")]
pub fn explain(e: &mtp_rs::Error) -> String {
  if e.is_exclusive_access() {
    "another program is using the Kindle. Quit OpenMTP, Android File Transfer, Image \
     Capture and Photos, unplug and plug in the Kindle again, and try again. On a Mac the \
     camera service `ptpcamerad` can also hold it: it is in Activity Monitor (search for \
     \"ptpcamerad\"); the program will not touch it for you."
      .into()
  } else if e.is_permission_denied() {
    "the system denied access to the USB device (on Linux, a udev rule is missing)".into()
  } else if e.is_disconnected() {
    "the Kindle was disconnected or stopped answering; unplug it, plug it in and try again".into()
  } else {
    e.to_string()
  }
}

#[cfg(feature = "mtp")]
pub use device::{copy, Options};

#[cfg(feature = "mtp")]
mod device {
  use std::fs;
  use std::io::Write;
  use std::path::Path;

  use futures::executor::block_on;
  use mtp_rs::{ByteRange, MtpDevice, ObjectHandle, Storage};

  use super::{
    explain, is_current, local_path, safe_component, Entry, Report, Wanted, MAX_FILE_BYTES,
  };

  /// Amazon's USB vendor ID.
  const AMAZON: u16 = 0x1949;

  pub struct Options<'a> {
    /// Destination folder; `documents/` is made inside it.
    pub dest: &'a Path,
    pub wanted: Wanted,
    /// List only, fetch nothing, make no folder.
    pub list_only: bool,
    /// The serial number of the device to use [default: the one Kindle].
    pub serial: Option<&'a str>,
    /// Say what is going on.
    pub progress: &'a mut dyn FnMut(&str),
  }

  /// Copy the Kindle's `documents/` into `options.dest`, read only.
  pub fn copy(options: Options<'_>) -> Result<Report, String> {
    block_on(run(options))
  }

  async fn run(options: Options<'_>) -> Result<Report, String> {
    let Options {
      dest,
      wanted,
      list_only,
      serial,
      progress,
    } = options;
    let devices = MtpDevice::list_devices().map_err(|e| explain(&e))?;
    let chosen: Vec<_> = devices
      .iter()
      .filter(|d| match serial {
        Some(s) => d.serial_number.as_deref() == Some(s),
        None => {
          d.vendor_id == AMAZON
            || d
              .manufacturer
              .as_deref()
              .is_some_and(|m| m.contains("Amazon"))
            || d.product.as_deref().is_some_and(|p| p.contains("Kindle"))
        }
      })
      .collect();
    let info = match chosen.as_slice() {
      [one] => (*one).clone(),
      [] => {
        let mut e = String::from("no Kindle found over MTP.");
        if devices.is_empty() {
          e.push_str(" No MTP device is visible at all: is the Kindle unlocked and connected with a data cable?");
        } else {
          e.push_str(" MTP devices seen (use --serial to pick one):");
          for d in &devices {
            e.push_str(&format!(
              "\n  {:04x}:{:04x} {} {} serial {}",
              d.vendor_id,
              d.product_id,
              d.manufacturer.as_deref().unwrap_or("?"),
              d.product.as_deref().unwrap_or("?"),
              d.serial_number.as_deref().unwrap_or("?")
            ));
          }
        }
        return Err(e);
      }
      _ => return Err("more than one Kindle: choose one with --serial".into()),
    };
    let device = MtpDevice::open_by_location(info.location_id)
      .await
      .map_err(|e| explain(&e))?;
    let mut report = Report {
      device: format!(
        "{} {} (serial {}, {:04x}:{:04x}, firmware {})",
        device.device_info().manufacturer,
        device.device_info().model,
        device.device_info().serial_number,
        info.vendor_id,
        info.product_id,
        device.device_info().device_version
      ),
      ..Report::default()
    };
    progress(&report.device.clone());
    let storages = device.storages().await.map_err(|e| explain(&e))?;
    let mut found = None;
    for storage in &storages {
      let root = storage.list_objects(None).await.map_err(|e| explain(&e))?;
      if report.root.is_empty() {
        report.root = root.iter().map(|o| o.filename.clone()).collect();
      }
      if let Some(d) = root
        .iter()
        .find(|o| o.is_folder() && o.filename == "documents")
      {
        found = Some((storage, d.handle));
        break;
      }
    }
    let Some((storage, documents)) = found else {
      return Err(format!(
        "no `documents` folder in the storage; the root holds: {}",
        report.root.join(", ")
      ));
    };
    walk(
      storage,
      documents,
      dest,
      wanted,
      list_only,
      &mut report,
      progress,
    )
    .await?;
    let _ = device.close().await;
    Ok(report)
  }

  async fn walk(
    storage: &Storage,
    documents: ObjectHandle,
    dest: &Path,
    wanted: Wanted,
    list_only: bool,
    report: &mut Report,
    progress: &mut dyn FnMut(&str),
  ) -> Result<(), String> {
    let mut stack: Vec<(ObjectHandle, Vec<String>)> = vec![(documents, Vec::new())];
    while let Some((folder, path)) = stack.pop() {
      let children = storage
        .list_objects(Some(folder))
        .await
        .map_err(|e| explain(&e))?;
      for child in children {
        let mut p = path.clone();
        p.push(child.filename.clone());
        let name = p.join("/");
        if safe_component(&child.filename).is_none() {
          report.skipped.push((name, "unsafe name".into()));
          continue;
        }
        if child.is_folder() {
          report.folders += 1;
          if child.filename.to_lowercase().ends_with(".sdr") {
            report.sdr_folders += 1;
          }
          report.entries.push(Entry {
            path: p.clone(),
            size: 0,
            is_folder: true,
          });
          stack.push((child.handle, p));
          continue;
        }
        report.files += 1;
        if child.filename.to_lowercase().ends_with(".kfx") {
          report.kfx_files += 1;
        }
        report.entries.push(Entry {
          path: p.clone(),
          size: child.size,
          is_folder: false,
        });
        if list_only || !wanted.file(&p) {
          continue;
        }
        if child.size > MAX_FILE_BYTES {
          report
            .skipped
            .push((name, format!("{} bytes is too large", child.size)));
          continue;
        }
        let Some(local) = local_path(dest, &p) else {
          report.skipped.push((name, "unsafe name".into()));
          continue;
        };
        if is_current(&local, child.size) {
          report.cached += 1;
          continue;
        }
        progress(&format!("fetching {name}"));
        match fetch(storage, child.handle, &local).await {
          Ok(bytes) => {
            report.fetched += 1;
            report.fetched_bytes += bytes;
          }
          Err(e) => report.skipped.push((name, e)),
        }
      }
    }
    Ok(())
  }

  /// Download one object into `local` through a `.part` file.
  async fn fetch(storage: &Storage, handle: ObjectHandle, local: &Path) -> Result<u64, String> {
    if let Some(dir) = local.parent() {
      fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let part = local.with_extension("part-nickelinksync");
    let mut file = fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut download = storage
      .download(handle, ByteRange::Full)
      .await
      .map_err(|e| explain(&e))?;
    let mut total = 0u64;
    while let Some(chunk) = download.next_chunk().await {
      let chunk = chunk.map_err(|e| explain(&e))?;
      file.write_all(&chunk).map_err(|e| e.to_string())?;
      total += chunk.len() as u64;
    }
    drop(file);
    fs::rename(&part, local).map_err(|e| e.to_string())?;
    Ok(total)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn p(s: &[&str]) -> Vec<String> {
    s.iter().map(|x| x.to_string()).collect()
  }

  #[test]
  fn unsafe_names_are_refused() {
    for bad in ["", ".", "..", "a/b", "a\\b", "C:x", "a\nb", "x ", "x."] {
      assert!(safe_component(bad).is_none(), "{bad:?}");
    }
    for good in [
      "Downloads",
      "book-p_ABC.kfx",
      "book.sdr",
      ".hidden",
      "Über Bücher.kfx",
    ] {
      assert!(safe_component(good).is_some(), "{good:?}");
    }
  }

  #[test]
  fn local_path_stays_below_dest() {
    let dest = Path::new("/tmp/k");
    assert_eq!(
      local_path(dest, &p(&["Downloads", "Items01", "a.sdr", "a.yjf"])),
      Some(PathBuf::from(
        "/tmp/k/documents/Downloads/Items01/a.sdr/a.yjf"
      ))
    );
    assert!(local_path(dest, &p(&["..", "etc"])).is_none());
    assert!(local_path(dest, &p(&["a", "b/c"])).is_none());
  }

  #[test]
  fn wanted_files() {
    let states = Wanted { books: false };
    let books = Wanted { books: true };
    let yjf = p(&["Downloads", "Items01", "a.sdr", "a.yjf"]);
    let kfx = p(&["Downloads", "Items01", "a.kfx"]);
    let other = p(&["Downloads", "Items01", "a.epub"]);
    assert!(states.file(&yjf) && books.file(&yjf));
    assert!(!states.file(&kfx) && books.file(&kfx));
    assert!(!states.file(&other) && !books.file(&other));
    assert!(!states.file(&[]));
    // Case does not matter.
    assert!(books.file(&p(&["X.KFX"])) && states.file(&p(&["X.SDR", "y"])));
  }

  #[test]
  fn cached_copy_needs_the_same_size() {
    let dir = std::env::temp_dir().join(format!("nis-kmtp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.yjf");
    std::fs::write(&f, b"12345").unwrap();
    assert!(is_current(&f, 5));
    assert!(!is_current(&f, 6));
    assert!(!is_current(&dir.join("missing"), 5));
    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn report_says_when_no_sdr_is_visible() {
    let r = Report {
      folders: 3,
      files: 4,
      ..Report::default()
    };
    assert!(r.render(false).contains("No .sdr folder is visible"));
    let r = Report {
      sdr_folders: 1,
      ..Report::default()
    };
    assert!(!r.render(false).contains("No .sdr folder"));
  }
}
