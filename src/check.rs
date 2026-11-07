//! `nickelinksync device check`: a read-only look at a mounted Kobo or
//! Tolino before a device test. It writes nothing and never prints the
//! password or key of `config.ini`.
//!
//! It answers the questions that cost earlier test rounds: is this one of
//! the tested firmwares, is the package on the device the build that was just
//! made (an old one once sat on the device while `dist/` was stale), does
//! `config.ini` say `confirm = never` (then a conflict never comes up), was
//! the device prepared, and is anything left over.

use crate::db::Firmware;
use crate::kobalt::{Kobalt, State as KobaltState};
use crate::prepare::{self, Prepared};
use crate::state::State;
use md5::{Digest, Md5};
use std::fs;
use std::path::{Path, PathBuf};

/// Model and firmware the owner has tested; anything else gets a warning.
pub const TESTED: [(&str, &str); 3] = [
  ("Kobo Clara BW", "6.0.279225"),
  ("Kobo Libra 2", "4.38.23852"),
  ("Tolino Shine Color", "5.18.279364"),
];

/// Where the new build lies on the computer, to compare with what is on the
/// device. Each part is optional.
#[derive(Debug, Default, Clone)]
pub struct Build {
  /// The program for the device (`dist/NickelInkSync/.adds/NickelInkSync/nickelinksync`).
  pub program: Option<PathBuf>,
  /// `Kobo.tgz` (Kobo 6.x, Tolino).
  pub plugin: Option<PathBuf>,
  /// `KoboRoot.tgz` (Kobo 4.x).
  pub plugin_qt5: Option<PathBuf>,
}

impl Build {
  /// The files of a folder laid out like `dist/NickelInkSync` (`just
  /// package`); `KoboRoot.tgz` is looked for in `kobo-4.x/` and next to it.
  pub fn from_package(dir: &Path) -> Self {
    let first = |names: &[&str]| names.iter().map(|n| dir.join(n)).find(|p| p.is_file());
    Build {
      program: first(&[".adds/NickelInkSync/nickelinksync"]),
      plugin: first(&["Kobo.tgz"]),
      plugin_qt5: first(&["kobo-4.x/KoboRoot.tgz", "KoboRoot.tgz"]),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
  Ok,
  /// Not wrong, but worth knowing before the test.
  Warn,
  /// The test would be wasted.
  Bad,
}

#[derive(Debug, Clone)]
pub struct Line {
  pub mark: Mark,
  pub what: &'static str,
  pub text: String,
  /// What to do about it.
  pub todo: Option<String>,
}

#[derive(Debug, Default)]
pub struct Report {
  pub lines: Vec<Line>,
}

impl Report {
  fn add(&mut self, mark: Mark, what: &'static str, text: impl Into<String>, todo: Option<&str>) {
    self.lines.push(Line {
      mark,
      what,
      text: text.into(),
      todo: todo.map(str::to_string),
    });
  }

  pub fn problems(&self) -> usize {
    self.lines.iter().filter(|l| l.mark == Mark::Bad).count()
  }

  pub fn warnings(&self) -> usize {
    self.lines.iter().filter(|l| l.mark == Mark::Warn).count()
  }

  /// No ✗.
  pub fn ok(&self) -> bool {
    self.problems() == 0
  }

  /// The text to print.
  pub fn render(&self) -> String {
    let mut out = String::new();
    for l in &self.lines {
      let mark = match l.mark {
        Mark::Ok => "✓",
        Mark::Warn => "!",
        Mark::Bad => "✗",
      };
      out.push_str(&format!("{mark} {:<9} {}\n", l.what, l.text));
      if let Some(todo) = &l.todo {
        out.push_str(&format!("            → {todo}\n"));
      }
    }
    out.push_str(&match (self.problems(), self.warnings()) {
      (0, 0) => "Ready for the test.\n".to_string(),
      (0, w) => format!("No problem found, {w} to look at (!) before the test.\n"),
      (p, w) => format!("{p} problem(s) (✗) and {w} to look at (!): fix the ✗ before the test.\n"),
    });
    out
  }
}

/// Checks the device mounted at `root`. `system` is the device's root file
/// system if it can be seen (only on the device itself).
pub fn check(root: &Path, system: Option<&Path>, build: &Build) -> Report {
  let mut r = Report::default();
  let dir = root.join(prepare::DIR);
  let qt5 = device(&mut r, root);
  plugin(&mut r, root, system, build, qt5);
  program(&mut r, &dir, build);
  config(&mut r, &dir);
  kobalt(&mut r, root, system);
  prepared(&mut r, root);
  state(&mut r, root);
  leftovers(&mut r, root);
  r
}

/// Model and firmware. Returns whether the plugin needs the Qt 5 build
/// (firmware before 5.0, i.e. Kobo 4.x).
fn device(r: &mut Report, root: &Path) -> Option<bool> {
  let Ok(line) = fs::read_to_string(root.join(".kobo/version")) else {
    r.add(
      Mark::Bad,
      "Device",
      "no .kobo/version: this does not look like a mounted Kobo or Tolino",
      Some("give the mount point of the device, e.g. /Volumes/KOBOeReader, with --device"),
    );
    return None;
  };
  let Some(fw) = Firmware::parse(&line) else {
    r.add(
      Mark::Bad,
      "Device",
      ".kobo/version cannot be read",
      Some("reconnect the device; if it stays so, this firmware is not known"),
    );
    return None;
  };
  let qt5 = fw
    .version
    .split('.')
    .next()
    .and_then(|m| m.parse::<u32>().ok())
    .map(|major| major < 5);
  let list = TESTED
    .iter()
    .map(|(m, v)| format!("{m} {v}"))
    .collect::<Vec<_>>()
    .join(", ");
  match fw.model_name() {
    Some(name) if TESTED.contains(&(name, fw.version.as_str())) => r.add(
      Mark::Ok,
      "Device",
      format!("{name}, firmware {} (tested)", fw.version),
      None,
    ),
    Some(name) => r.add(
      Mark::Warn,
      "Device",
      format!(
        "{name}, firmware {}: not one of the tested ones",
        fw.version
      ),
      Some(&format!(
        "tested are {list}; other firmware may work, nobody has checked"
      )),
    ),
    None => r.add(
      Mark::Warn,
      "Device",
      format!(
        "unknown model {}, firmware {}: not one of the tested ones",
        fw.model_id, fw.version
      ),
      Some(&format!("tested are {list}")),
    ),
  }
  qt5
}

fn md5(path: &Path) -> Option<String> {
  let bytes = fs::read(path).ok()?;
  Some(
    Md5::digest(&bytes)
      .iter()
      .map(|b| format!("{b:02x}"))
      .collect(),
  )
}

fn same(a: &Path, b: &Path) -> Option<bool> {
  Some(md5(a)? == md5(b)?)
}

/// The plugin file waiting in `.kobo/` (the device installs it on the next
/// start and deletes it) and, on the device itself, the installed library.
fn plugin(r: &mut Report, root: &Path, system: Option<&Path>, build: &Build, qt5: Option<bool>) {
  let (name, mine) = match qt5 {
    Some(true) => ("KoboRoot.tgz", &build.plugin_qt5),
    _ => ("Kobo.tgz", &build.plugin),
  };
  let other = if name == "Kobo.tgz" {
    "KoboRoot.tgz"
  } else {
    "Kobo.tgz"
  };
  let waiting = root.join(".kobo").join(name);
  let wrong = root.join(".kobo").join(other);
  if qt5.is_some() && wrong.exists() {
    r.add(
      Mark::Bad,
      "Plugin",
      format!(".kobo/{other} is the file for the other firmware; this device installs only {name}"),
      Some(&format!(
        "delete .kobo/{other} and copy {name} ({})",
        if name == "KoboRoot.tgz" {
          "just plugin-qt5"
        } else {
          "just plugin && just package"
        }
      )),
    );
    return;
  }
  let installed = system.map(|s| s.join("usr/local/Kobo/imageformats/libnickelinksync.so"));
  if waiting.is_file() {
    match mine.as_deref().map(|b| same(&waiting, b)) {
      Some(Some(true)) => r.add(
        Mark::Ok,
        "Plugin",
        format!(".kobo/{name} waits for the next start and is the new build"),
        None,
      ),
      Some(_) => r.add(
        Mark::Bad,
        "Plugin",
        format!(".kobo/{name} on the device is not the build on the computer (an old package?)"),
        Some("rebuild (just plugin && just package, or just plugin-qt5), copy the new file over it, eject"),
      ),
      None => r.add(
        Mark::Warn,
        "Plugin",
        format!(".kobo/{name} waits for the next start; not compared (no build given)"),
        None,
      ),
    }
  } else if installed.as_ref().is_some_and(|p| p.is_file()) {
    r.add(Mark::Ok, "Plugin", "installed (libnickelinksync.so)", None);
  } else if root.join(prepare::DIR).join("NickelInkSync.log").is_file() {
    r.add(
      Mark::Warn,
      "Plugin",
      format!("no .kobo/{name} waiting; the device has run the plugin before, so it is probably installed (not visible over USB), but not whether it is the new build"),
      Some(&format!("to be sure, copy {name} again and eject")),
    );
  } else {
    r.add(
      Mark::Bad,
      "Plugin",
      format!("no .kobo/{name} waiting and no sign that the plugin ever ran"),
      Some(&format!(
        "copy {name} into the hidden .kobo/ folder and eject ({})",
        if name == "KoboRoot.tgz" {
          "just plugin-qt5"
        } else {
          "just plugin && just package"
        }
      )),
    );
  }
}

fn program(r: &mut Report, dir: &Path, build: &Build) {
  let path = dir.join("nickelinksync");
  if !path.is_file() {
    r.add(
      Mark::Bad,
      "Program",
      ".adds/NickelInkSync/nickelinksync is missing",
      Some("copy the .adds/NickelInkSync folder of the package (just package) to the device"),
    );
    return;
  }
  match build.program.as_deref().map(|b| same(&path, b)) {
    Some(Some(true)) => r.add(Mark::Ok, "Program", "is the new build", None),
    Some(_) => r.add(
      Mark::Bad,
      "Program",
      "is not the build on the computer (an old package?)",
      Some("run just build-device && just package, copy .adds/NickelInkSync/nickelinksync over it"),
    ),
    None => r.add(
      Mark::Warn,
      "Program",
      "present; not compared (no build given)",
      None,
    ),
  }
}

/// `key = value` lines of `config.ini`, only the keys asked for; passwords
/// and keys are not read into the report.
fn setting(text: &str, key: &str) -> Option<String> {
  let mut found = None;
  for line in text.lines() {
    let line = line.trim();
    if line.starts_with('#') || line.starts_with(';') {
      continue;
    }
    if let Some((k, v)) = line.split_once('=') {
      if k.trim() == key {
        found = Some(v.trim().to_string()).filter(|v| !v.is_empty());
      }
    }
  }
  found
}

fn config(r: &mut Report, dir: &Path) {
  let path = dir.join("config.ini");
  let Ok(text) = fs::read_to_string(&path) else {
    r.add(
      Mark::Bad,
      "Config",
      ".adds/NickelInkSync/config.ini is missing",
      Some("save config.ini.example from the package as config.ini and fill in server, user and password"),
    );
    return;
  };
  let server = setting(&text, "server");
  let user = setting(&text, "user");
  let secret = setting(&text, "password").is_some() || setting(&text, "key").is_some();
  let mut missing = vec![];
  if server.is_none() {
    missing.push("server");
  }
  if user.is_none() {
    missing.push("user");
  }
  if !secret {
    missing.push("password or key");
  }
  if missing.is_empty() {
    r.add(
      Mark::Ok,
      "Config",
      format!(
        "server {}, user {}, password or key set",
        server.unwrap_or_default(),
        user.unwrap_or_default()
      ),
      None,
    );
  } else {
    r.add(
      Mark::Bad,
      "Config",
      format!("config.ini lacks {}", missing.join(", ")),
      Some("fill them in; the device cannot sync without an account"),
    );
  }
  match setting(&text, "confirm").as_deref() {
    None | Some("backwards") => r.add(
      Mark::Ok,
      "confirm",
      format!(
        "{} (a book read on both sides, server further back, is a conflict)",
        if setting(&text, "confirm").is_none() {
          "not set = backwards"
        } else {
          "backwards"
        }
      ),
      None,
    ),
    Some("never") => r.add(
      Mark::Bad,
      "confirm",
      "never: the newer position always wins, so a conflict never comes up",
      Some("for the conflict test set `confirm = backwards` in config.ini"),
    ),
    Some(other) => r.add(
      Mark::Warn,
      "confirm",
      format!("{other}: every position that differs asks, not only going backwards"),
      Some("for the conflict test set `confirm = backwards`"),
    ),
  }
  match setting(&text, "notify").as_deref() {
    None | Some("all") => r.add(Mark::Ok, "notify", "all (every message is shown)", None),
    Some(other) => r.add(
      Mark::Warn,
      "notify",
      format!("{other}: some messages will not appear on the screen"),
      Some("for a test set `notify = all` or remove the line"),
    ),
  }
}

fn kobalt(r: &mut Report, root: &Path, system: Option<&Path>) {
  let k = Kobalt::detect(root, system);
  match (k.state, k.ignored) {
    (KobaltState::Absent, _) => r.add(Mark::Ok, "Kobalt", "not installed", None),
    (KobaltState::Leftover, _) => r.add(
      Mark::Ok,
      "Kobalt",
      ".adds/kobalt exists, but Kobalt is not installed (ignored)",
      None,
    ),
    (KobaltState::Installed, false) => r.add(
      Mark::Warn,
      "Kobalt",
      "installed: the plugin stays inactive and sends nothing",
      Some("right only for the Kobalt-guard test; otherwise remove Kobalt, or create the empty file .adds/NickelInkSync/ignore-kobalt for a test next to it"),
    ),
    (KobaltState::Installed, true) => r.add(
      Mark::Warn,
      "Kobalt",
      "installed, but ignore-kobalt exists: NickelInkSync works next to it",
      Some("for the Kobalt-guard test delete .adds/NickelInkSync/ignore-kobalt"),
    ),
  }
}

fn prepared(r: &mut Report, root: &Path) {
  match Prepared::load(root) {
    Ok(Some(p)) => r.add(
      Mark::Ok,
      "Prepared",
      format!(
        "yes: {} originals in the index, {} mappings",
        p.index.books.len(),
        p.mappings.holding().count()
      ),
      None,
    ),
    Ok(None) => r.add(
      Mark::Warn,
      "Prepared",
      "no library.json: the device is not prepared, the plugin fetches but sends nothing",
      Some("on the computer: nickelinksync device prepare --device <this device> (right only for the \"not prepared\" test)"),
    ),
    Err(e) => r.add(
      Mark::Bad,
      "Prepared",
      format!("library.json or mappings.json cannot be read: {e}"),
      Some("run device prepare again with the current version"),
    ),
  }
}

fn state(r: &mut Report, root: &Path) {
  match State::load(root) {
    Ok(s) => {
      let pending: Vec<_> = s
        .pending()
        .map(|(id, e)| e.title.clone().unwrap_or_else(|| id.to_string()))
        .collect();
      if pending.is_empty() {
        r.add(Mark::Ok, "Pending", "no book waits to be sent", None);
      } else {
        r.add(
          Mark::Warn,
          "Pending",
          format!("{} book(s) wait to be sent: {}", pending.len(), pending.join(", ")),
          Some("fine if left from the last test; they go out on the next open with Wi-Fi, which changes the server"),
        );
      }
    }
    Err(e) => r.add(
      Mark::Bad,
      "Pending",
      format!("state.json cannot be read: {e}"),
      Some("the file comes from another version; move it away (the plugin starts a new one)"),
    ),
  }
}

fn leftovers(r: &mut Report, root: &Path) {
  let found: Vec<_> = ["-wal", "-journal", "-shm"]
    .iter()
    .map(|s| format!(".kobo/KoboReader.sqlite{s}"))
    .filter(|f| fs::metadata(root.join(f)).is_ok_and(|m| m.len() > 0))
    .collect();
  if found.is_empty() {
    r.add(
      Mark::Ok,
      "Database",
      "no leftover -wal or -journal file",
      None,
    );
  } else {
    r.add(
      Mark::Warn,
      "Database",
      format!("{} not empty: Nickel did not close the database cleanly", found.join(", ")),
      Some("the latest positions may be missing here; eject properly, let the device start, then read again"),
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Tree(PathBuf);

  impl Tree {
    fn new(name: &str) -> Self {
      let root =
        std::env::temp_dir().join(format!("nickelinksync-check-{}-{name}", std::process::id()));
      let _ = fs::remove_dir_all(&root);
      fs::create_dir_all(root.join(".kobo")).unwrap();
      fs::create_dir_all(root.join(".adds/NickelInkSync")).unwrap();
      Tree(root)
    }

    fn put(&self, file: &str, text: &str) -> &Self {
      let p = self.0.join(file);
      fs::create_dir_all(p.parent().unwrap()).unwrap();
      fs::write(p, text).unwrap();
      self
    }
  }

  impl Drop for Tree {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  fn find<'a>(r: &'a Report, what: &str) -> &'a Line {
    r.lines.iter().find(|l| l.what == what).unwrap()
  }

  const VERSION_4: &str =
    "N418123456789,4.9.77,4.38.23852,4.9.77,4.9.77,00000000-0000-0000-0000-000000000388\n";
  const VERSION_6: &str =
    "N418123456789,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n";

  #[test]
  fn tested_firmware_and_a_matching_build_pass() {
    let t = Tree::new("good");
    t.put(".kobo/version", VERSION_4)
      .put(".kobo/KoboRoot.tgz", "plugin")
      .put(".adds/NickelInkSync/nickelinksync", "program")
      .put(
        ".adds/NickelInkSync/config.ini",
        "server = https://x\nuser = a\npassword = secret-pw\nconfirm = backwards\n",
      )
      .put("build/KoboRoot.tgz", "plugin")
      .put("build/nickelinksync", "program");
    let build = Build {
      program: Some(t.0.join("build/nickelinksync")),
      plugin: None,
      plugin_qt5: Some(t.0.join("build/KoboRoot.tgz")),
    };
    let r = check(&t.0, None, &build);
    assert_eq!(find(&r, "Device").mark, Mark::Ok);
    assert_eq!(find(&r, "Plugin").mark, Mark::Ok);
    assert_eq!(find(&r, "Program").mark, Mark::Ok);
    assert_eq!(find(&r, "Config").mark, Mark::Ok);
    assert_eq!(find(&r, "confirm").mark, Mark::Ok);
    // not prepared is only a warning: it is also a test case
    assert_eq!(find(&r, "Prepared").mark, Mark::Warn);
    assert!(r.ok());
    assert!(!r.render().contains("secret-pw"));
  }

  #[test]
  fn an_old_package_is_found() {
    let t = Tree::new("old");
    t.put(".kobo/version", VERSION_6)
      .put(".kobo/Kobo.tgz", "old plugin")
      .put(".adds/NickelInkSync/nickelinksync", "old program")
      .put("build/Kobo.tgz", "new plugin")
      .put("build/nickelinksync", "new program");
    let build = Build {
      program: Some(t.0.join("build/nickelinksync")),
      plugin: Some(t.0.join("build/Kobo.tgz")),
      plugin_qt5: None,
    };
    let r = check(&t.0, None, &build);
    assert_eq!(find(&r, "Plugin").mark, Mark::Bad);
    assert_eq!(find(&r, "Program").mark, Mark::Bad);
    assert!(!r.ok());
  }

  #[test]
  fn the_file_for_the_other_firmware_is_wrong() {
    let t = Tree::new("other");
    t.put(".kobo/version", VERSION_6)
      .put(".kobo/KoboRoot.tgz", "x");
    let r = check(&t.0, None, &Build::default());
    assert_eq!(find(&r, "Plugin").mark, Mark::Bad);
    assert!(find(&r, "Plugin").text.contains("other firmware"));
  }

  #[test]
  fn confirm_never_is_a_problem_and_the_secret_stays_unprinted() {
    let t = Tree::new("never");
    t.put(".kobo/version", VERSION_6)
      .put(".adds/NickelInkSync/config.ini", "server = https://x\nuser = a\nkey = 0123456789abcdef0123456789abcdef\nconfirm = never\nnotify = errors\n");
    let r = check(&t.0, None, &Build::default());
    assert_eq!(find(&r, "confirm").mark, Mark::Bad);
    assert_eq!(find(&r, "notify").mark, Mark::Warn);
    assert!(!r.render().contains("0123456789abcdef"));
  }

  #[test]
  fn untested_firmware_unprepared_pending_and_leftovers_are_noted() {
    let t = Tree::new("notes");
    t.put(".kobo/version", "N1,4.9.77,5.99.1,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n")
      .put(".kobo/KoboReader.sqlite-wal", "x")
      .put(
        ".adds/NickelInkSync/state.json",
        r#"{"version":1,"books":{"file:///mnt/onboard/a.epub":{"title":"Alpha","pending":{"since":1,"reason":"network"}}}}"#,
      );
    let r = check(&t.0, None, &Build::default());
    assert_eq!(find(&r, "Device").mark, Mark::Warn);
    assert_eq!(find(&r, "Prepared").mark, Mark::Warn);
    assert_eq!(find(&r, "Pending").mark, Mark::Warn);
    assert!(find(&r, "Pending").text.contains("Alpha"));
    assert_eq!(find(&r, "Database").mark, Mark::Warn);
  }

  #[test]
  fn kobalt_is_reported() {
    let t = Tree::new("kobalt");
    t.put(".kobo/version", VERSION_6)
      .put(".adds/kobalt/uninstall", "");
    let r = check(&t.0, None, &Build::default());
    assert_eq!(find(&r, "Kobalt").mark, Mark::Warn);
    assert!(find(&r, "Kobalt").text.contains("installed"));
  }

  #[test]
  fn nothing_is_written() {
    let t = Tree::new("readonly");
    t.put(".kobo/version", VERSION_6);
    let before =
      fs::read_dir(&t.0).unwrap().count() + fs::read_dir(t.0.join(".kobo")).unwrap().count();
    let _ = check(&t.0, None, &Build::default());
    let after =
      fs::read_dir(&t.0).unwrap().count() + fs::read_dir(t.0.join(".kobo")).unwrap().count();
    assert_eq!(before, after);
    assert!(!t.0.join(".adds/NickelInkSync/state.json").exists());
  }
}
