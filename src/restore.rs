//! The backups of a device database that `pull --write` and the desktop
//! client make before they write (in [`Config::backup_dir`]), and putting
//! one back. Each backup made since this module exists has a `.device` file
//! next to it naming the device by a checksum of its serial number (the
//! number itself is not kept), so a backup only goes back onto the device
//! it came from; older ones count as of an unknown device.

use std::fs;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::config::Config;
use crate::db::Device;
use crate::kobalt::Kobalt;
use crate::session::Result;
use crate::sync::iso_timestamp;

/// Which device a backup came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
  ThisDevice,
  /// Made before backups named their device.
  Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Backup {
  pub path: PathBuf,
  /// When it was made, Unix seconds (the file's time).
  pub time: i64,
  pub origin: Origin,
}

/// The device at `root` as a backup names it: a checksum of the serial
/// number and model in `.kobo/version`.
pub fn device_tag(root: &Path) -> Option<String> {
  let line = fs::read_to_string(root.join(".kobo/version")).ok()?;
  let fields: Vec<&str> = line.trim().split(',').collect();
  let (serial, model) = (fields.first()?, fields.last()?);
  Some(
    Md5::digest(format!("nickelinksync:{serial}:{model}").as_bytes())
      .iter()
      .map(|b| format!("{b:02x}"))
      .collect(),
  )
}

fn tag_file(backup: &Path) -> PathBuf {
  let mut name = backup.as_os_str().to_owned();
  name.push(".device");
  PathBuf::from(name)
}

/// Notes next to `backup` that it is of the device at `root`.
pub fn tag(backup: &Path, root: &Path) -> std::io::Result<()> {
  match device_tag(root) {
    Some(tag) => fs::write(tag_file(backup), tag),
    None => Ok(()),
  }
}

/// A free name for a new backup in `dir`: `KoboReader-<UTC time>.sqlite`,
/// with `-2`, `-3`, … for a second one in the same second.
pub fn new_path(dir: &Path) -> Result<PathBuf> {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)?
    .as_secs() as i64;
  let stamp: String = iso_timestamp(now)
    .chars()
    .filter(|c| !"-:".contains(*c))
    .collect();
  Ok(
    (1..)
      .map(|n| match n {
        1 => dir.join(format!("KoboReader-{stamp}.sqlite")),
        n => dir.join(format!("KoboReader-{stamp}-{n}.sqlite")),
      })
      .find(|p| !p.exists())
      .expect("a free name"),
  )
}

/// The backups in `dir` that may go back onto the device at `root`, newest
/// first: its own, and those of an unknown device. Another device's are
/// left out.
pub fn list_in(dir: &Path, root: &Path) -> Result<Vec<Backup>> {
  let own = device_tag(root);
  let entries = match fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(e) => return Err(format!("{}: {e}", dir.display()).into()),
  };
  let mut found = Vec::new();
  for entry in entries {
    let path = entry?.path();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !(name.starts_with("KoboReader-") && name.ends_with(".sqlite")) {
      continue;
    }
    let origin = match fs::read_to_string(tag_file(&path)) {
      Ok(tag) if Some(tag.trim()) == own.as_deref() => Origin::ThisDevice,
      Ok(_) => continue,
      Err(_) => Origin::Unknown,
    };
    let time = fs::metadata(&path)?
      .modified()?
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs() as i64)
      .unwrap_or(0);
    found.push(Backup { path, time, origin });
  }
  found.sort_by(|a, b| b.time.cmp(&a.time).then(b.path.cmp(&a.path)));
  Ok(found)
}

/// [`list_in`] the backup directory.
pub fn list(root: &Path) -> Result<Vec<Backup>> {
  let dir = Config::backup_dir().ok_or("no directory for backups (HOME is not set)")?;
  list_in(&dir, root)
}

/// Puts the backup at `source` back onto the mounted device at `root`, after
/// a backup of the database as it is now; returns that one. A backup of
/// another device is refused, and so is any while Kobalt is installed.
pub fn restore(root: &Path, source: &Path) -> Result<PathBuf> {
  if let (Ok(tag), Some(own)) = (fs::read_to_string(tag_file(source)), device_tag(root)) {
    if tag.trim() != own {
      return Err(format!("{} is a backup of another device", source.display()).into());
    }
  }
  // Kobalt manages the positions while it is installed, like for a write.
  Kobalt::detect(root, None).check()?;
  let dir = Config::backup_dir().ok_or("no directory for backups (HOME is not set)")?;
  let backup = new_path(&dir)?;
  let device = Device::open(root, false)?;
  device.restore(source, &backup)?;
  tag(&backup, root)?;
  Ok(backup)
}
