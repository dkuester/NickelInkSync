//! The local configuration file: KOSync server, user, password and how this
//! device names itself on the server.
//!
//! One `key = value` per line; empty lines and lines starting with `#` or
//! `;` are ignored. Keys: `server`, `user`, `password` or `key` (the MD5 of
//! the password), and optionally `device`, `device_id`, `confirm`
//! (`never`, `backwards` or `always`, default `backwards`) and `notify`
//! (`all`, `errors` or `none`, default `all`; only the device plugin uses
//! it, for its messages on the screen) and `library` (the folder with the
//! original EPUBs, a Calibre library or any folder; a leading `~/` is the
//! home folder) and `position` (`exact`, `sentence` or `paragraph`, default
//! `paragraph`: which place `push` sends, see [`PushPosition`]). The
//! password is only ever read from this file, never from the command line.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::kosync;
use crate::sync::Confirm;

#[derive(Debug)]
pub enum Error {
  Io(PathBuf, io::Error),
  /// Line number (1-based) and what is wrong with it.
  Syntax(PathBuf, usize, String),
  Missing(PathBuf, &'static str),
  Invalid(PathBuf, &'static str),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
      Error::Syntax(path, line, e) => write!(f, "{}, line {line}: {e}", path.display()),
      Error::Missing(path, key) => write!(f, "{}: `{key}` is not set", path.display()),
      Error::Invalid(path, e) => write!(f, "{}: {e}", path.display()),
    }
  }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
  /// Base URL, e.g. `https://kosync.eu`.
  pub server: String,
  pub user: String,
  /// MD5 of the password, as sent in `x-auth-key`.
  pub key: String,
  /// Device name shown on the server; defaults to the model of the device.
  pub device: Option<String>,
  /// Defaults to a value derived from the device name.
  pub device_id: Option<String>,
  /// When to ask before overwriting a position.
  pub confirm: Confirm,
  /// Which messages the device plugin shows; the CLI only checks it.
  pub notify: Notify,
  /// Folder with the original EPUBs (Calibre library or plain folder).
  pub library: Option<PathBuf>,
  /// Which place `push` sends.
  pub position: PushPosition,
}

/// `position`: which place `push` sends. The reading position is where the
/// device's page starts, often mid-sentence; KOReader opens the page that
/// holds it, by its own page layout, so the place can be anywhere on that
/// page. A paragraph or sentence start is easier to find there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PushPosition {
  /// The stored position, to the character.
  Exact,
  /// The start of its sentence.
  Sentence,
  /// The start of its paragraph, or of its sentence where the paragraph
  /// starts too far back.
  #[default]
  Paragraph,
}

impl std::str::FromStr for PushPosition {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, ()> {
    match s {
      "exact" => Ok(PushPosition::Exact),
      "sentence" => Ok(PushPosition::Sentence),
      "paragraph" => Ok(PushPosition::Paragraph),
      _ => Err(()),
    }
  }
}

/// The settings that need no server: what the book mapping and the place
/// to send use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalSettings {
  pub library: Option<PathBuf>,
  pub position: PushPosition,
  /// The file names a server or account (`server`, `user`, `password` or
  /// `key`); without, it only holds local settings.
  pub server: bool,
}

/// Every key the file may hold.
const KEYS: [&str; 10] = [
  "server",
  "user",
  "password",
  "key",
  "device",
  "device_id",
  "confirm",
  "notify",
  "library",
  "position",
];

/// `notify`: which messages the device plugin shows on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notify {
  /// Every result of a pull or push, also "up to date".
  #[default]
  All,
  /// Only what went wrong or was left undone.
  Errors,
  None,
}

impl std::str::FromStr for Notify {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, ()> {
    match s {
      "all" => Ok(Notify::All),
      "errors" => Ok(Notify::Errors),
      "none" => Ok(Notify::None),
      _ => Err(()),
    }
  }
}

impl Config {
  /// Where the configuration is looked up without `--config`:
  /// `$NICKELINKSYNC_CONFIG`, else `$XDG_CONFIG_HOME/nickelinksync/config.ini`,
  /// else `~/.config/nickelinksync/config.ini`.
  pub fn default_path() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(path) = var("NICKELINKSYNC_CONFIG") {
      return Some(PathBuf::from(path));
    }
    let base = match var("XDG_CONFIG_HOME") {
      Some(dir) => PathBuf::from(dir),
      None => PathBuf::from(var("HOME")?).join(".config"),
    };
    Some(base.join("nickelinksync/config.ini"))
  }

  /// Where NickelInkSync keeps its own data (book mappings, backups):
  /// `$NICKELINKSYNC_DATA_DIR`, else `$XDG_DATA_HOME/nickelinksync`, else
  /// `~/.local/share/nickelinksync`. The CLI and the desktop client share it.
  pub fn data_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var("NICKELINKSYNC_DATA_DIR") {
      return Some(PathBuf::from(dir));
    }
    let base = match var("XDG_DATA_HOME") {
      Some(dir) => PathBuf::from(dir),
      None => PathBuf::from(var("HOME")?).join(".local/share"),
    };
    Some(base.join("nickelinksync"))
  }

  /// Where `pull --write` keeps database backups:
  /// `$NICKELINKSYNC_BACKUP_DIR`, else `backups` in [`Config::data_dir`].
  pub fn backup_dir() -> Option<PathBuf> {
    match std::env::var_os("NICKELINKSYNC_BACKUP_DIR").filter(|v| !v.is_empty()) {
      Some(dir) => Some(PathBuf::from(dir)),
      None => Some(Self::data_dir()?.join("backups")),
    }
  }

  pub fn load(path: &Path) -> Result<Self, Error> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::Io(path.to_path_buf(), e))?;
    Self::parse(path, &text)
  }

  /// Only the `library` setting of the file at `path`, without needing
  /// the server settings (the book mapping works without a server). A
  /// missing file has none.
  pub fn library_setting(path: &Path) -> Result<Option<PathBuf>, Error> {
    Ok(Self::local_settings(path)?.library)
  }

  /// `library` and `position` of the file at `path`, without needing the
  /// server settings. A missing file has the defaults.
  pub fn local_settings(path: &Path) -> Result<LocalSettings, Error> {
    let text = match std::fs::read_to_string(path) {
      Ok(text) => text,
      Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(LocalSettings::default()),
      Err(e) => return Err(Error::Io(path.to_path_buf(), e)),
    };
    let mut entries = entries(path, &text)?;
    let server = ["server", "user", "password", "key"]
      .iter()
      .any(|k| entries.contains_key(k));
    Ok(LocalSettings {
      server,
      library: entries.remove("library").map(|l| expand_home(&l)),
      position: push_position(path, entries.remove("position"))?,
    })
  }

  /// `path` is only used in error messages.
  pub fn parse(path: &Path, text: &str) -> Result<Self, Error> {
    let mut entries = entries(path, text)?;
    let mut take = |key| entries.remove(key);
    let (server, user, password, key) =
      (take("server"), take("user"), take("password"), take("key"));
    let (device, device_id, confirm, notify) = (
      take("device"),
      take("device_id"),
      take("confirm"),
      take("notify"),
    );
    let library = take("library").map(|l| expand_home(&l));
    let position = push_position(path, take("position"))?;
    let missing = |key| Error::Missing(path.to_path_buf(), key);
    let key = match (password, key) {
      (Some(_), Some(_)) => {
        return Err(Error::Invalid(
          path.to_path_buf(),
          "set either `password` or `key`, not both",
        ))
      }
      (Some(password), None) => kosync::auth_key(&password),
      (None, Some(key)) if is_md5(&key) => key.to_ascii_lowercase(),
      (None, Some(_)) => {
        return Err(Error::Invalid(
          path.to_path_buf(),
          "`key` must be 32 hex digits",
        ))
      }
      (None, None) => return Err(missing("password")),
    };
    let confirm = match confirm {
      None => Confirm::default(),
      Some(value) => value.parse().map_err(|()| {
        Error::Invalid(
          path.to_path_buf(),
          "`confirm` must be `never`, `backwards` or `always`",
        )
      })?,
    };
    let notify = match notify {
      None => Notify::default(),
      Some(value) => value.parse().map_err(|()| {
        Error::Invalid(
          path.to_path_buf(),
          "`notify` must be `all`, `errors` or `none`",
        )
      })?,
    };
    Ok(Config {
      server: server.ok_or_else(|| missing("server"))?,
      user: user.ok_or_else(|| missing("user"))?,
      key,
      device,
      device_id,
      confirm,
      notify,
      library,
      position,
    })
  }

  /// Device name and ID to report: from the file, else `model` and an ID
  /// derived from the name, so each e-reader keeps its own stable ID.
  pub fn identity(&self, model: &str) -> (String, String) {
    let name = self.device.clone().unwrap_or_else(|| model.to_string());
    let id = self.device_id.clone().unwrap_or_else(|| device_id(&name));
    (name, id)
  }
}

/// The non-empty `key = value` pairs, with a syntax error for a line
/// without `=` and for an unknown key; a later line wins.
fn entries(path: &Path, text: &str) -> Result<HashMap<&'static str, String>, Error> {
  let syntax = |line, e: String| Error::Syntax(path.to_path_buf(), line, e);
  let mut entries = HashMap::new();
  for (n, line) in text.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
      continue;
    }
    let Some((name, value)) = line.split_once('=') else {
      return Err(syntax(n + 1, "expected `key = value`".into()));
    };
    let name = name.trim();
    let Some(&key) = KEYS.iter().find(|&&k| k == name) else {
      return Err(syntax(n + 1, format!("unknown key `{name}`")));
    };
    match value.trim() {
      "" => entries.remove(key),
      value => entries.insert(key, value.to_string()),
    };
  }
  Ok(entries)
}

fn push_position(path: &Path, value: Option<String>) -> Result<PushPosition, Error> {
  match value {
    None => Ok(PushPosition::default()),
    Some(value) => value.parse().map_err(|()| {
      Error::Invalid(
        path.to_path_buf(),
        "`position` must be `exact`, `sentence` or `paragraph`",
      )
    }),
  }
}

/// `~/…` with the home folder.
fn expand_home(path: &str) -> PathBuf {
  match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
    (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
    _ => PathBuf::from(path),
  }
}

/// 32 uppercase hex digits like KOReader's random device IDs, but derived
/// from the device name.
pub fn device_id(name: &str) -> String {
  Md5::digest(format!("nickelinksync:{name}").as_bytes())
    .iter()
    .map(|b| format!("{b:02X}"))
    .collect()
}

fn is_md5(s: &str) -> bool {
  s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A warning if the file holds a password and others can read it.
pub fn permission_warning(path: &Path) -> Option<String> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode();
    if mode & 0o077 != 0 {
      return Some(format!(
        "{} can be read by other users; run `chmod 600` on it",
        path.display()
      ));
    }
  }
  #[cfg(not(unix))]
  let _ = path;
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(text: &str) -> Result<Config, Error> {
    Config::parse(Path::new("config.ini"), text)
  }

  #[test]
  fn password_becomes_md5_key() {
    let c = parse(
      "# KOSync\nserver = https://kosync.eu/\nuser = alice\npassword = secret\n\n; comment\n",
    )
    .unwrap();
    assert_eq!(c.server, "https://kosync.eu/");
    assert_eq!(c.user, "alice");
    assert_eq!(c.key, "5ebe2294ecd0e0f08eab7690d2a6ee69");
    assert_eq!(c.device, None);
    assert_eq!(c.confirm, Confirm::Backwards);
  }

  #[test]
  fn confirm_setting() {
    let c = parse("server=s\nuser=u\npassword=p\nconfirm = always").unwrap();
    assert_eq!(c.confirm, Confirm::Always);
    let c = parse("server=s\nuser=u\npassword=p\nconfirm=never").unwrap();
    assert_eq!(c.confirm, Confirm::Never);
    let e = parse("server=s\nuser=u\npassword=p\nconfirm=sometimes")
      .unwrap_err()
      .to_string();
    assert!(e.contains("`confirm` must be"), "{e}");
  }

  #[test]
  fn notify_setting() {
    let c = parse("server=s\nuser=u\npassword=p").unwrap();
    assert_eq!(c.notify, Notify::All);
    let c = parse("server=s\nuser=u\npassword=p\nnotify = errors").unwrap();
    assert_eq!(c.notify, Notify::Errors);
    let c = parse("server=s\nuser=u\npassword=p\nnotify=none").unwrap();
    assert_eq!(c.notify, Notify::None);
    let e = parse("server=s\nuser=u\npassword=p\nnotify=loud")
      .unwrap_err()
      .to_string();
    assert!(e.contains("`notify` must be"), "{e}");
  }

  #[test]
  fn key_instead_of_password() {
    let c = parse("server=s\nuser=u\nkey=5EBE2294ECD0E0F08EAB7690D2A6EE69").unwrap();
    assert_eq!(c.key, "5ebe2294ecd0e0f08eab7690d2a6ee69");
    assert!(parse("server=s\nuser=u\nkey=nope").is_err());
    assert!(parse("server=s\nuser=u\nkey=5ebe2294ecd0e0f08eab7690d2a6ee69\npassword=x").is_err());
  }

  #[test]
  fn errors_name_the_problem() {
    let e = parse("server = s\nuser = u\n").unwrap_err().to_string();
    assert!(e.contains("`password` is not set"), "{e}");
    let e = parse("server = s\nusr = u\n").unwrap_err().to_string();
    assert!(e.contains("line 2: unknown key `usr`"), "{e}");
    let e = parse("server s\n").unwrap_err().to_string();
    assert!(e.contains("line 1"), "{e}");
  }

  #[test]
  fn library_setting() {
    let c = parse("server=s\nuser=u\npassword=p\nlibrary = /books/Calibre Library").unwrap();
    assert_eq!(c.library, Some(PathBuf::from("/books/Calibre Library")));
    let c = parse("server=s\nuser=u\npassword=p\nlibrary =").unwrap();
    assert_eq!(c.library, None);
    if let Some(home) = std::env::var_os("HOME") {
      let c = parse("server=s\nuser=u\npassword=p\nlibrary = ~/Calibre").unwrap();
      assert_eq!(c.library, Some(PathBuf::from(home).join("Calibre")));
    }
  }

  #[test]
  fn position_setting() {
    let c = parse("server=s\nuser=u\npassword=p").unwrap();
    assert_eq!(c.position, PushPosition::Paragraph);
    let c = parse("server=s\nuser=u\npassword=p\nposition = exact").unwrap();
    assert_eq!(c.position, PushPosition::Exact);
    let e = parse("server=s\nuser=u\npassword=p\nposition=page")
      .unwrap_err()
      .to_string();
    assert!(e.contains("`position` must be"), "{e}");
  }

  #[test]
  fn identity_defaults_to_the_model() {
    let mut c = parse("server=s\nuser=u\npassword=p").unwrap();
    let (name, id) = c.identity("Tolino Shine Color");
    assert_eq!(name, "Tolino Shine Color");
    assert_eq!(id, device_id("Tolino Shine Color"));
    assert_eq!(id.len(), 32);
    c.device = Some("Shelf".into());
    c.device_id = Some("ABC".into());
    assert_eq!(c.identity("x"), ("Shelf".into(), "ABC".into()));
  }
}
