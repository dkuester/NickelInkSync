//! Kobalt (<https://github.com/nicoverbruggen/kobalt>, MIT) is a reader for
//! Kobo and Tolino that runs inside Nickel as a NickelHook plugin. It writes
//! reading positions into Nickel's database itself, and its development
//! branch has a KOSync client of its own. With Kobalt installed, NickelInkSync
//! writes nothing into the device database and the plugin sends nothing: two
//! programs changing the same positions, and two KOSync clients, only get in
//! each other's way.
//!
//! Kobalt is installed when all of these hold (what Kobalt's NickelHook
//! checks before it hooks into Nickel):
//!
//! - its library is in Nickel's plugin folder, `libkobalt.so`, or one folder
//!   up as `libkobalt.so.failsafe`, where NickelHook moves it for the first
//!   seconds after each start. Only visible on the device itself (`--live`).
//! - `.adds/kobalt/uninstall` exists: deleting it uninstalls Kobalt at the
//!   next start, keeping its folder and data.
//! - `.adds/kobalt/uninstall-now` does not exist: it uninstalls Kobalt at the
//!   next start too.
//!
//! A folder `.adds/kobalt` alone is a leftover (or a package copied but not
//! yet installed): reported, but it blocks nothing. With the file
//! `.adds/NickelInkSync/ignore-kobalt`, an installed Kobalt blocks nothing
//! either (for tests). The plugin repeats these paths in
//! `plugin/src/nickelinksync.cc`.

use std::fmt;
use std::path::{Path, PathBuf};

/// Kobalt's profile folder on the device storage.
pub const PROFILE: &str = ".adds/kobalt";
/// While this exists, Kobalt stays installed.
pub const KEEP_FLAG: &str = ".adds/kobalt/uninstall";
/// If this exists, Kobalt uninstalls itself at the next start.
pub const REMOVE_FLAG: &str = ".adds/kobalt/uninstall-now";
/// Kobalt's library, relative to the device's root file system.
pub const LIBRARY: &str = "usr/local/Kobo/imageformats/libkobalt.so";
/// Where NickelHook keeps the library for the first seconds after a start.
pub const FAILSAFE: &str = "usr/local/Kobo/libkobalt.so.failsafe";
/// With this file, an installed Kobalt blocks nothing (for tests).
pub const IGNORE: &str = ".adds/NickelInkSync/ignore-kobalt";
/// The device's root file system for `--live`; `$NICKELINKSYNC_SYSTEM_ROOT`
/// overrides it (for tests).
pub const SYSTEM_ROOT: &str = "/";

/// What is found of Kobalt on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
  /// Nothing.
  Absent,
  /// Its folder, but Kobalt is not installed or uninstalls itself at the
  /// next start.
  Leftover,
  /// Kobalt is installed.
  Installed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kobalt {
  pub state: State,
  /// `ignore-kobalt` exists.
  pub ignored: bool,
}

impl Kobalt {
  /// Looks for Kobalt on the device whose storage is at `onboard`. `system`
  /// is the device's root file system when running on the device itself;
  /// over USB it cannot be seen (`None`), and the flags in `.adds/kobalt`
  /// decide alone.
  pub fn detect(onboard: &Path, system: Option<&Path>) -> Self {
    let library = match system {
      Some(system) => system.join(LIBRARY).exists() || system.join(FAILSAFE).exists(),
      None => true,
    };
    let flags = onboard.join(KEEP_FLAG).exists() && !onboard.join(REMOVE_FLAG).exists();
    let state = if library && flags {
      State::Installed
    } else if onboard.join(PROFILE).is_dir() || (system.is_some() && library) {
      State::Leftover
    } else {
      State::Absent
    };
    Kobalt {
      state,
      ignored: onboard.join(IGNORE).exists(),
    }
  }

  /// On the device itself: the root file system is `/`, or
  /// `$NICKELINKSYNC_SYSTEM_ROOT`.
  pub fn detect_live(onboard: &Path) -> Self {
    let system = std::env::var_os("NICKELINKSYNC_SYSTEM_ROOT")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from(SYSTEM_ROOT));
    Kobalt::detect(onboard, Some(&system))
  }

  /// Kobalt is installed and not ignored: nothing may be written into the
  /// device database, and on the device nothing is sent.
  pub fn blocks(&self) -> bool {
    self.state == State::Installed && !self.ignored
  }

  /// A line for the user, if anything was found.
  pub fn note(&self) -> Option<String> {
    match (self.state, self.ignored) {
      (State::Absent, _) => None,
      (State::Leftover, _) => Some(format!(
        "{PROFILE} exists, but Kobalt is not installed: ignored"
      )),
      (State::Installed, true) => Some(format!(
        "Kobalt is installed, but {IGNORE} exists: syncing anyway"
      )),
      (State::Installed, false) => Some(Active.to_string()),
    }
  }

  /// [`Active`] if Kobalt blocks.
  pub fn check(&self) -> Result<(), Active> {
    match self.blocks() {
      true => Err(Active),
      false => Ok(()),
    }
  }
}

/// Refused because Kobalt is installed; `NICKELINKSYNC_ERROR<tab>kobalt` for
/// the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Active;

impl fmt::Display for Active {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "Kobalt is installed on the device and manages its reading positions itself: \
       nothing is written into the device database (remove Kobalt, or create {IGNORE} \
       to sync anyway)"
    )
  }
}

impl std::error::Error for Active {}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  /// A fake device: storage under `onboard/`, root file system under
  /// `system/`, with the given files.
  struct Tree(PathBuf);

  impl Tree {
    fn new(name: &str, files: &[&str]) -> Self {
      let root = std::env::temp_dir().join(format!(
        "nickelinksync-kobalt-{}-{name}",
        std::process::id()
      ));
      let _ = fs::remove_dir_all(&root);
      fs::create_dir_all(root.join("onboard/.kobo")).unwrap();
      fs::create_dir_all(root.join("system/usr/local/Kobo/imageformats")).unwrap();
      for file in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
      }
      Tree(root)
    }

    fn live(&self) -> Kobalt {
      Kobalt::detect(&self.0.join("onboard"), Some(&self.0.join("system")))
    }

    fn usb(&self) -> Kobalt {
      Kobalt::detect(&self.0.join("onboard"), None)
    }
  }

  impl Drop for Tree {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  const LIB: &str = "system/usr/local/Kobo/imageformats/libkobalt.so";
  const SAFE: &str = "system/usr/local/Kobo/libkobalt.so.failsafe";
  const KEEP: &str = "onboard/.adds/kobalt/uninstall";
  const NOW: &str = "onboard/.adds/kobalt/uninstall-now";
  const ASSETS: &str = "onboard/.adds/kobalt/assets/en.strings";

  #[test]
  fn a_normal_device_has_no_kobalt() {
    // Other mods next to it, as on the test Tolino.
    let t = Tree::new(
      "none",
      &[
        "system/usr/local/Kobo/imageformats/libnickelinksync.so",
        "system/usr/local/Kobo/imageformats/libnm.so",
        "onboard/.adds/NickelInkSync/config.ini",
        "onboard/.adds/nm/config",
      ],
    );
    for k in [t.live(), t.usb()] {
      assert_eq!(k.state, State::Absent);
      assert!(!k.blocks());
      assert_eq!(k.note(), None);
    }
  }

  #[test]
  fn installed_kobalt_blocks() {
    let t = Tree::new("installed", &[LIB, KEEP, ASSETS]);
    assert_eq!(t.live().state, State::Installed);
    assert!(t.live().blocks());
    assert_eq!(t.usb().state, State::Installed);
    assert_eq!(t.live().check(), Err(Active));
  }

  #[test]
  fn the_failsafe_copy_counts_as_installed() {
    // NickelHook moves the library aside for the first seconds of a start.
    let t = Tree::new("failsafe", &[SAFE, KEEP]);
    assert!(t.live().blocks());
  }

  #[test]
  fn uninstalled_by_removing_the_flag() {
    // Kobalt's own manual removal: the folder and its data stay; the
    // library is gone after the next start, or still there before it.
    let gone = Tree::new("removed", &[ASSETS, "onboard/.adds/kobalt/config.json"]);
    for k in [gone.live(), gone.usb()] {
      assert_eq!(k.state, State::Leftover);
      assert!(!k.blocks());
      assert!(k.note().unwrap().contains("not installed"));
    }
    let pending = Tree::new("removed-pending", &[LIB, ASSETS]);
    assert_eq!(pending.live().state, State::Leftover);
  }

  #[test]
  fn uninstall_now_wins() {
    let t = Tree::new("now", &[LIB, KEEP, NOW]);
    assert_eq!(t.live().state, State::Leftover);
    assert_eq!(t.usb().state, State::Leftover);
  }

  #[test]
  fn package_copied_but_not_installed() {
    // The ZIP is copied; Kobalt writes `uninstall` only when it first loads.
    let t = Tree::new("copied", &[ASSETS, "onboard/.kobo/Kobo.tgz"]);
    assert_eq!(t.usb().state, State::Leftover);
    assert_eq!(t.live().state, State::Leftover);
  }

  #[test]
  fn library_without_flags_on_the_device() {
    // First start: the library is there before Kobalt writes its flags.
    let t = Tree::new("first-start", &[LIB]);
    assert_eq!(t.live().state, State::Leftover);
    assert!(!t.live().blocks());
  }

  #[test]
  fn usb_cannot_see_the_library() {
    // The flags alone decide over USB, even if the library was removed.
    let t = Tree::new("usb", &[KEEP]);
    assert!(t.usb().blocks());
    assert!(!t.live().blocks());
  }

  #[test]
  fn ignore_file_lets_it_sync() {
    let t = Tree::new(
      "ignored",
      &[LIB, KEEP, "onboard/.adds/NickelInkSync/ignore-kobalt"],
    );
    let k = t.live();
    assert_eq!(k.state, State::Installed);
    assert!(k.ignored);
    assert!(!k.blocks());
    assert_eq!(k.check(), Ok(()));
    assert!(k.note().unwrap().contains("syncing anyway"));
  }

  #[test]
  fn the_plugin_uses_the_same_paths() {
    let plugin = include_str!("../plugin/src/nickelinksync.cc");
    for path in [KEEP_FLAG, REMOVE_FLAG, IGNORE] {
      assert!(
        plugin.contains(&format!("/mnt/onboard/{path}\"")),
        "plugin lacks {path}"
      );
    }
    for path in [LIBRARY, FAILSAFE] {
      assert!(
        plugin.contains(&format!("\"/{path}\"")),
        "plugin lacks {path}"
      );
    }
  }
}
