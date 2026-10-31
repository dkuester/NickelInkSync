//! Settings only the desktop client has, in `gui.json` in the data
//! directory next to the mappings. `config.ini` is shared with the CLI and
//! the device, and rejects keys it does not know.

use std::io;
use std::path::{Path, PathBuf};

use nickelinksync::config::Config;
use serde::{Deserialize, Serialize};

use crate::i18n::Choice;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
  pub language: Choice,
  /// The last chosen copy of a Kindle's books.
  pub kindle_folder: Option<PathBuf>,
}

impl Settings {
  /// `gui.json` in [`Config::data_dir`].
  pub fn default_path() -> Option<PathBuf> {
    Some(Config::data_dir()?.join("gui.json"))
  }

  /// The settings at `path`; the defaults if the file is missing or
  /// unreadable (they only affect the window).
  pub fn load(path: &Path) -> Self {
    std::fs::read_to_string(path)
      .ok()
      .and_then(|text| serde_json::from_str(&text).ok())
      .unwrap_or_default()
  }

  /// Writes to a temporary file first, then renames it over `path`.
  pub fn save(&self, path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent() {
      std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
    std::fs::write(&tmp, text + "\n")?;
    std::fs::rename(&tmp, path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trip_and_defaults() {
    let dir =
      std::env::temp_dir().join(format!("nickelinksync-gui-{}-settings", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("gui.json");
    assert_eq!(Settings::load(&path), Settings::default());
    let settings = Settings {
      language: Choice::De,
      kindle_folder: Some("/tmp/kindle".into()),
    };
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path), settings);
    assert!(std::fs::read_to_string(&path)
      .unwrap()
      .contains("\"language\": \"de\""));
    // A broken file falls back to the defaults.
    std::fs::write(&path, "{").unwrap();
    assert_eq!(Settings::load(&path), Settings::default());
    std::fs::remove_dir_all(&dir).unwrap();
  }
}
