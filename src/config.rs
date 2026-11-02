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
//! password is only ever read from this file, never from the command line;
//! without `password` and `key` the key comes from the system keychain
//! ([`credentials`]), where the desktop client puts it.
//!
//! The desktop client edits the file line by line ([`Editable`]): comments
//! and the order stay, a backup goes next to it first.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::credentials;
use crate::kosync;
use crate::sync::Confirm;

#[derive(Debug)]
pub enum Error {
  Io(PathBuf, io::Error),
  /// Line number (1-based) and what is wrong with it.
  Syntax(PathBuf, usize, String),
  Missing(PathBuf, &'static str),
  Invalid(PathBuf, &'static str),
  /// The system keychain could not be read or written.
  Keychain(String),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
      Error::Syntax(path, line, e) => write!(f, "{}, line {line}: {e}", path.display()),
      Error::Missing(path, key) => write!(f, "{}: `{key}` is not set", path.display()),
      Error::Invalid(path, e) => write!(f, "{}: {e}", path.display()),
      Error::Keychain(e) => write!(f, "keychain: {e}"),
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

impl fmt::Display for PushPosition {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      PushPosition::Exact => "exact",
      PushPosition::Sentence => "sentence",
      PushPosition::Paragraph => "paragraph",
    })
  }
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
  /// else `~/.config/nickelinksync/config.ini`; on Windows
  /// `%APPDATA%\nickelinksync\config.ini`.
  pub fn default_path() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(path) = var("NICKELINKSYNC_CONFIG") {
      return Some(PathBuf::from(path));
    }
    if cfg!(windows) {
      return Some(
        PathBuf::from(var("APPDATA")?)
          .join("nickelinksync")
          .join("config.ini"),
      );
    }
    let base = match var("XDG_CONFIG_HOME") {
      Some(dir) => PathBuf::from(dir),
      None => PathBuf::from(var("HOME")?).join(".config"),
    };
    Some(base.join("nickelinksync/config.ini"))
  }

  /// Where NickelInkSync keeps its own data (book mappings, backups):
  /// `$NICKELINKSYNC_DATA_DIR`, else `$XDG_DATA_HOME/nickelinksync`, else
  /// `~/.local/share/nickelinksync`; on Windows
  /// `%LOCALAPPDATA%\nickelinksync` (the backups are large, so not in the
  /// roaming profile). The CLI and the desktop client share it.
  pub fn data_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var("NICKELINKSYNC_DATA_DIR") {
      return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
      return Some(PathBuf::from(var("LOCALAPPDATA")?).join("nickelinksync"));
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

  /// The file at `path`; without `password` and `key`, the key from the
  /// system keychain.
  pub fn load(path: &Path) -> Result<Self, Error> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::Io(path.to_path_buf(), e))?;
    Self::parse_with(path, &text, true)
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

  /// `path` is only used in error messages. The key only from the text.
  pub fn parse(path: &Path, text: &str) -> Result<Self, Error> {
    Self::parse_with(path, text, false)
  }

  fn parse_with(path: &Path, text: &str, keychain: bool) -> Result<Self, Error> {
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
      (None, None) => match (&server, &user) {
        (Some(server), Some(user)) if keychain => {
          match credentials::get(server, user).map_err(Error::Keychain)? {
            Some(key) if is_md5(&key) => key.to_ascii_lowercase(),
            Some(_) => return Err(Error::Keychain("the stored key is no MD5".into())),
            None => return Err(missing("password")),
          }
        }
        _ => return Err(missing("password")),
      },
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

/// `~/…` with the home folder (`%USERPROFILE%` on Windows, also `~\…`).
fn expand_home(path: &str) -> PathBuf {
  let rest = path
    .strip_prefix("~/")
    .or_else(|| path.strip_prefix("~\\").filter(|_| cfg!(windows)));
  match (rest, home()) {
    (Some(rest), Some(home)) => home.join(rest),
    _ => PathBuf::from(path),
  }
}

/// The home folder: `$HOME`, on Windows `%USERPROFILE%`.
pub fn home() -> Option<PathBuf> {
  let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
  std::env::var_os(name)
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
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

/// Where the key for the server is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
  /// `password` or `key` in the file.
  InFile,
  InKeychain,
  Missing,
}

/// The file's settings as the desktop client edits them; empty text is
/// not set. The password never: only where the key is.
#[derive(Debug, Clone, PartialEq)]
pub struct Editable {
  pub server: String,
  pub user: String,
  pub device: String,
  pub confirm: Confirm,
  /// As written in the file (`~/` not expanded).
  pub library: String,
  pub position: PushPosition,
  pub secret: Secret,
}

impl Editable {
  /// The settings in the file at `path`; a missing file has none.
  pub fn read(path: &Path) -> Result<Self, Error> {
    let text = read_or_empty(path)?;
    let mut entries = entries(path, &text)?;
    let mut take = |key| entries.remove(key).unwrap_or_default();
    let (server, user, device, library) = (
      take("server"),
      take("user"),
      take("device"),
      take("library"),
    );
    let (password, key, confirm, position) = (
      take("password"),
      take("key"),
      take("confirm"),
      take("position"),
    );
    let secret = if !password.is_empty() || !key.is_empty() {
      Secret::InFile
    } else if server.is_empty() || user.is_empty() {
      Secret::Missing
    } else {
      match credentials::get(&server, &user).map_err(Error::Keychain)? {
        Some(_) => Secret::InKeychain,
        None => Secret::Missing,
      }
    };
    Ok(Editable {
      server,
      user,
      device,
      confirm: confirm.parse().unwrap_or_default(),
      library,
      position: position.parse().unwrap_or_default(),
      secret,
    })
  }

  /// Writes the settings into the file at `path`, after a backup of it.
  /// A new `password` goes into the keychain as its key (into the file as
  /// `key` where there is no keychain), and out of the file; a key in the
  /// keychain moves along when the server or user changes.
  pub fn save(&self, path: &Path, password: Option<&str>) -> Result<(), Error> {
    let before = Editable::read(path)?;
    let key = password.filter(|p| !p.is_empty()).map(kosync::auth_key);
    let mut changes: Vec<(&str, Option<String>)> = Vec::new();
    let account = |e: &Editable| (e.server.clone(), e.user.clone());
    if credentials::available() {
      if let Some(key) = &key {
        credentials::set(&self.server, &self.user, key).map_err(Error::Keychain)?;
        changes.push(("password", None));
        changes.push(("key", None));
      } else if before.secret == Secret::InKeychain && account(&before) != account(self) {
        let old = credentials::get(&before.server, &before.user).map_err(Error::Keychain)?;
        if let Some(old) = old {
          credentials::set(&self.server, &self.user, &old).map_err(Error::Keychain)?;
          credentials::delete(&before.server, &before.user).map_err(Error::Keychain)?;
        }
      }
    } else if let Some(key) = &key {
      changes.push(("password", None));
      changes.push(("key", Some(key.clone())));
    }
    let text = |v: &str| Some(v.trim().to_string()).filter(|v| !v.is_empty());
    changes.extend([
      ("server", text(&self.server)),
      ("user", text(&self.user)),
      ("device", text(&self.device)),
      ("confirm", Some(self.confirm.to_string())),
      ("library", text(&self.library)),
      ("position", Some(self.position.to_string())),
    ]);
    write_changes(path, &changes)
  }
}

/// Moves `password` or `key` from the file at `path` into the keychain
/// (as the key), after a backup of the file.
pub fn move_to_keychain(path: &Path) -> Result<(), Error> {
  let config = Config::parse(path, &read_or_empty(path)?)?;
  credentials::set(&config.server, &config.user, &config.key).map_err(Error::Keychain)?;
  write_changes(path, &[("password", None), ("key", None)])
}

fn read_or_empty(path: &Path) -> Result<String, Error> {
  match std::fs::read_to_string(path) {
    Ok(text) => Ok(text),
    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
    Err(e) => Err(Error::Io(path.to_path_buf(), e)),
  }
}

/// `text` with each key of `changes` set (in place of its first line, its
/// later lines dropped, or appended) or removed (`None`); comments, other
/// keys and the order stay.
fn rewrite(text: &str, changes: &[(&str, Option<String>)]) -> String {
  let mut lines = Vec::new();
  let mut done = HashSet::new();
  for line in text.lines() {
    let key = line
      .split_once('=')
      .map(|(k, _)| k.trim())
      .filter(|_| !line.trim_start().starts_with(['#', ';']));
    match key.and_then(|k| changes.iter().find(|(c, _)| *c == k)) {
      Some((key, value)) => {
        if done.insert(*key) {
          if let Some(value) = value {
            lines.push(format!("{key} = {value}"));
          }
        }
      }
      None => lines.push(line.to_string()),
    }
  }
  for (key, value) in changes {
    if let (false, Some(value)) = (done.contains(key), value) {
      lines.push(format!("{key} = {value}"));
    }
  }
  let mut text = lines.join("\n");
  text.push('\n');
  text
}

/// Writes `changes` into the file at `path`: a backup (`config.ini.bak`)
/// first, readable only by the owner like the file.
fn write_changes(path: &Path, changes: &[(&str, Option<String>)]) -> Result<(), Error> {
  let io = |e| Error::Io(path.to_path_buf(), e);
  if changes
    .iter()
    .any(|(_, v)| v.as_deref().is_some_and(|v| v.contains(['\n', '\r'])))
  {
    return Err(Error::Invalid(path.to_path_buf(), "a value spans lines"));
  }
  let before = read_or_empty(path)?;
  let text = rewrite(&before, changes);
  entries(path, &text)?;
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).map_err(io)?;
  }
  if path.exists() {
    let mut backup = path.as_os_str().to_owned();
    backup.push(".bak");
    write_private(Path::new(&backup), &before).map_err(io)?;
  }
  write_private(path, &text).map_err(io)
}

/// Writes `text` to `path` through a temporary file, mode 600 on Unix.
fn write_private(path: &Path, text: &str) -> io::Result<()> {
  let mut temp = path.as_os_str().to_owned();
  temp.push(".tmp");
  let temp = PathBuf::from(temp);
  std::fs::write(&temp, text)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))?;
  }
  std::fs::rename(&temp, path)
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

  /// A folder of this test run standing in for the keychain.
  fn keychain() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR
      .get_or_init(|| {
        let dir =
          std::env::temp_dir().join(format!("nickelinksync-{}-keychain", std::process::id()));
        std::env::set_var("NICKELINKSYNC_KEYCHAIN_DIR", &dir);
        dir
      })
      .clone()
  }

  fn temp_config(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nickelinksync-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.ini");
    std::fs::write(&path, text).unwrap();
    path
  }

  #[test]
  fn rewrite_keeps_comments_and_order() {
    let before = "# mine\nserver = a\n; note\nuser = u\npassword = p\nuser = v\nnotify = none\n";
    let after = rewrite(
      before,
      &[
        ("user", Some("w".into())),
        ("password", None),
        ("device", Some("Shine".into())),
      ],
    );
    assert_eq!(
      after,
      "# mine\nserver = a\n; note\nuser = w\nnotify = none\ndevice = Shine\n"
    );
    // A commented-out key stays a comment.
    assert_eq!(
      rewrite("# user = x\n", &[("user", Some("y".into()))]),
      "# user = x\nuser = y\n"
    );
  }

  #[test]
  fn save_puts_the_key_into_the_keychain() {
    keychain();
    let path = temp_config(
      "save",
      "# KOSync\nserver = https://example.org\nuser = alice\npassword = secret\nnotify = errors\n",
    );
    let mut edit = Editable::read(&path).unwrap();
    assert_eq!(edit.secret, Secret::InFile);
    assert_eq!(edit.user, "alice");
    edit.device = "Shine".into();
    edit.save(&path, Some("secret")).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
      !text.contains("password") && !text.contains("secret"),
      "{text}"
    );
    assert!(
      text.starts_with("# KOSync\nserver = https://example.org\nuser = alice\n"),
      "{text}"
    );
    assert!(
      text.contains("notify = errors\n") && text.contains("device = Shine\n"),
      "{text}"
    );
    // The backup holds the file as it was.
    let backup = std::fs::read_to_string(path.with_extension("ini.bak")).unwrap();
    assert!(backup.contains("password = secret"));
    assert_eq!(Editable::read(&path).unwrap().secret, Secret::InKeychain);
    let config = Config::load(&path).unwrap();
    assert_eq!(config.key, "5ebe2294ecd0e0f08eab7690d2a6ee69");
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = std::fs::metadata(&path).unwrap().permissions().mode();
      assert_eq!(mode & 0o777, 0o600);
    }

    // A new user name takes the key along.
    let mut edit = Editable::read(&path).unwrap();
    edit.user = "bob".into();
    edit.save(&path, None).unwrap();
    assert_eq!(
      Config::load(&path).unwrap().key,
      "5ebe2294ecd0e0f08eab7690d2a6ee69"
    );
    assert_eq!(
      credentials::get("https://example.org", "alice").unwrap(),
      None
    );
    // Parsing the text alone does not look into the keychain.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(matches!(
      Config::parse(&path, &text),
      Err(Error::Missing(_, "password"))
    ));
    credentials::delete("https://example.org", "bob").unwrap();
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
  }

  #[test]
  fn move_to_keychain_takes_the_key_out_of_the_file() {
    keychain();
    let path = temp_config(
      "move",
      "server = https://example.net\nuser = carol\nkey = 5EBE2294ECD0E0F08EAB7690D2A6EE69\n",
    );
    move_to_keychain(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "server = https://example.net\nuser = carol\n");
    assert_eq!(
      Config::load(&path).unwrap().key,
      "5ebe2294ecd0e0f08eab7690d2a6ee69"
    );
    credentials::delete("https://example.net", "carol").unwrap();
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
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
    if let Some(home) = home() {
      let c = parse("server=s\nuser=u\npassword=p\nlibrary = ~/Calibre").unwrap();
      assert_eq!(c.library, Some(home.join("Calibre")));
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
