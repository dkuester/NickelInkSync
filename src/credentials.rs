//! The KOSync key (the MD5 of the password, never the password) in the
//! system's credential store: the keychain on macOS, the Credential Manager
//! on Windows. Elsewhere (the device, Linux) there is none, and the key
//! stays in `config.ini`.
//!
//! One entry per account, service "NickelInkSync", account
//! `<user>@<server>`. With `$NICKELINKSYNC_KEYCHAIN_DIR` set, a file per
//! account in that folder stands in for the store (for tests).

use std::path::PathBuf;

pub type Error = String;

/// The store's name of the account.
pub fn account(server: &str, user: &str) -> String {
  format!("{user}@{}", server.trim_end_matches('/'))
}

/// Whether there is a store to keep the key in.
pub fn available() -> bool {
  test_dir().is_some() || cfg!(any(target_os = "macos", target_os = "windows"))
}

/// The key stored for the account, if any.
pub fn get(server: &str, user: &str) -> Result<Option<String>, Error> {
  let account = account(server, user);
  if let Some(dir) = test_dir() {
    return match std::fs::read_to_string(dir.join(file(&account))) {
      Ok(key) => Ok(Some(key)),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(e) => Err(e.to_string()),
    };
  }
  native::get(&account)
}

/// Stores `key` for the account, replacing an earlier one.
pub fn set(server: &str, user: &str, key: &str) -> Result<(), Error> {
  let account = account(server, user);
  if let Some(dir) = test_dir() {
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    return std::fs::write(dir.join(file(&account)), key).map_err(|e| e.to_string());
  }
  native::set(&account, key)
}

/// Removes the account's key; none is fine.
pub fn delete(server: &str, user: &str) -> Result<(), Error> {
  let account = account(server, user);
  if let Some(dir) = test_dir() {
    return match std::fs::remove_file(dir.join(file(&account))) {
      Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
      _ => Ok(()),
    };
  }
  native::delete(&account)
}

fn test_dir() -> Option<PathBuf> {
  std::env::var_os("NICKELINKSYNC_KEYCHAIN_DIR")
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
}

/// The account as a file name in the test folder.
fn file(account: &str) -> String {
  account
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
    .collect()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod native {
  use keyring::Entry;

  use super::Error;

  const SERVICE: &str = "NickelInkSync";

  fn entry(account: &str) -> Result<Entry, Error> {
    Entry::new(SERVICE, account).map_err(|e| e.to_string())
  }

  pub fn get(account: &str) -> Result<Option<String>, Error> {
    match entry(account)?.get_password() {
      Ok(key) => Ok(Some(key)),
      Err(keyring::Error::NoEntry) => Ok(None),
      Err(e) => Err(e.to_string()),
    }
  }

  pub fn set(account: &str, key: &str) -> Result<(), Error> {
    entry(account)?.set_password(key).map_err(|e| e.to_string())
  }

  pub fn delete(account: &str) -> Result<(), Error> {
    match entry(account)?.delete_credential() {
      Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
      Err(e) => Err(e.to_string()),
    }
  }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod native {
  use super::Error;

  const NONE: &str = "no keychain on this system: the key stays in config.ini";

  pub fn get(_: &str) -> Result<Option<String>, Error> {
    Ok(None)
  }

  pub fn set(_: &str, _: &str) -> Result<(), Error> {
    Err(NONE.into())
  }

  pub fn delete(_: &str) -> Result<(), Error> {
    Ok(())
  }
}
