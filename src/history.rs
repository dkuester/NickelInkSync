//! The sync history: one JSON line for each position sent to the server or
//! written to a device, in `history.jsonl` in the data directory, shared
//! by the CLI and the desktop client. Only appended to; a line that cannot
//! be read is skipped.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Which way a position went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
  /// From the device to the server.
  Push,
  /// From the server into the device database.
  Pull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
  /// Unix seconds.
  pub time: i64,
  pub direction: Direction,
  /// The device, as the server knows it ("Kobo Clara BW", "Kindle").
  pub device: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  /// The KOSync document: the checksum the book syncs under.
  pub document: String,
  /// What was sent (KOReader XPointer) or written (the device's format).
  pub position: String,
  /// 0.0 to 1.0.
  pub percentage: f64,
  /// "cli" or "gui".
  pub by: String,
}

/// `history.jsonl` in [`Config::data_dir`].
pub fn default_path() -> Option<PathBuf> {
  Some(Config::data_dir()?.join("history.jsonl"))
}

/// Seconds since 1970, for [`Entry::time`].
pub fn now() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0)
}

/// Appends `entries` to the history at `path`, one line each.
pub fn append(path: &Path, entries: &[Entry]) -> io::Result<()> {
  if entries.is_empty() {
    return Ok(());
  }
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }
  let mut text = String::new();
  for entry in entries {
    text.push_str(&serde_json::to_string(entry).map_err(io::Error::other)?);
    text.push('\n');
  }
  // One write call, so lines from the CLI and the window do not mix.
  OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)?
    .write_all(text.as_bytes())
}

/// Appends to the default history; without a data directory nothing is
/// kept.
pub fn record(entries: &[Entry]) -> io::Result<()> {
  match default_path() {
    Some(path) => append(&path, entries),
    None => Ok(()),
  }
}

/// The last `n` entries at `path`, newest first; none if there is no
/// history yet.
pub fn read(path: &Path, n: usize) -> io::Result<Vec<Entry>> {
  let text = match fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(e) => return Err(e),
  };
  Ok(
    text
      .lines()
      .rev()
      .filter_map(|line| serde_json::from_str(line).ok())
      .take(n)
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(time: i64, direction: Direction) -> Entry {
    Entry {
      time,
      direction,
      device: "Tolino Shine Color".into(),
      title: Some("Book A".into()),
      document: "0123456789abcdef0123456789abcdef".into(),
      position: "/body[1]/DocFragment[82]/body[1]/div[1]/text()[1].0".into(),
      percentage: 0.74,
      by: "cli".into(),
    }
  }

  #[test]
  fn append_and_read_newest_first() {
    let dir = std::env::temp_dir().join(format!("nickelinksync-{}-history", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("history.jsonl");
    assert_eq!(read(&path, 10).unwrap(), []);
    append(
      &path,
      &[entry(1, Direction::Push), entry(2, Direction::Pull)],
    )
    .unwrap();
    append(&path, &[]).unwrap();
    // A broken line (e.g. from a newer version) is skipped.
    OpenOptions::new()
      .append(true)
      .open(&path)
      .unwrap()
      .write_all(b"{broken\n")
      .unwrap();
    append(&path, &[entry(3, Direction::Push)]).unwrap();
    let times: Vec<i64> = read(&path, 10).unwrap().iter().map(|e| e.time).collect();
    assert_eq!(times, [3, 2, 1]);
    assert_eq!(read(&path, 2).unwrap().len(), 2);
    let text = fs::read_to_string(&path).unwrap();
    assert!(
      text.starts_with("{\"time\":1,\"direction\":\"push\","),
      "{text}"
    );
    fs::remove_dir_all(&dir).unwrap();
  }
}
