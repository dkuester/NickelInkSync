//! One sync run on a device, shared by the CLI and the desktop client: open
//! the device and the server, then one step per book that says what `push`
//! or `pull` would do with it. Nothing here prints or asks: a step that
//! needs an answer says so ([`Action::Ask`]), and the caller asks (the CLI
//! in the terminal, the desktop client in a dialog) before it applies the
//! step, so the rules live in one place.

use std::cell::OnceCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{self, Config, LocalSettings, PushPosition};
use crate::db::{Book, Device, Write};
use crate::epub::Epub;
use crate::hash::partial_md5;
use crate::kfx::Kfx;
use crate::kindle;
use crate::kindle_device::{self, KindleBook, Unreadable};
use crate::kobalt::Kobalt;
use crate::kosync::{self, Client, Progress, Update};
use crate::library::Library;
use crate::mapping::{EpubState, Mapping, Store};
use crate::not_in_line::NotInLine;
use crate::position::{
  from_xpointer, from_xpointer_kepub, to_xpointer, PointFlavor, Position, Translated, XPointer,
};
use crate::restore;
use crate::sync::{decide, iso_timestamp, parse_timestamp, Confirm, Decision, Direction, Local};
use crate::transfer::{step_back, transfer, How};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// What a run works on, as the CLI's `--device`, `--config`, … give it.
#[derive(Debug, Clone)]
pub struct Options {
  /// Mount point of the device, or a copy of a Kindle's books.
  pub device: PathBuf,
  /// Configuration file; `None` for [`Config::default_path`].
  pub config: Option<PathBuf>,
  /// Only books whose title or file path contains this text.
  pub book: Option<String>,
  /// Only the book with exactly this ContentID (Kindle: `content_id`).
  pub content_id: Option<String>,
  /// Nickel is running (on the device): read its database as it is being
  /// written, never write it, no mappings.
  pub live: bool,
  /// How long each request to the server may take.
  pub timeout: Duration,
  /// Library of originals instead of `library` in the configuration.
  pub library: Option<PathBuf>,
}

impl Options {
  fn local_settings(&self) -> Result<LocalSettings> {
    Ok(match self.config.clone().or_else(Config::default_path) {
      Some(config) => Config::local_settings(&config)?,
      None => LocalSettings::default(),
    })
  }
}

/// The mappings in the data directory; none if there is no data directory.
pub fn load_store() -> Result<Store> {
  Ok(match Store::default_path() {
    Some(path) => Store::load(&path)?,
    None => Store::default(),
  })
}

/// The configuration and a client for its server.
pub struct Server {
  pub config: Config,
  pub client: Client,
  /// Other users can read the configuration file.
  pub warning: Option<String>,
}

impl Server {
  /// With `options.config` the file must exist; the default file may be
  /// missing, then `None`.
  pub fn open(options: &Options) -> Result<Option<Self>> {
    let (path, required) = match &options.config {
      Some(path) => (path.clone(), true),
      None => match Config::default_path() {
        Some(path) => (path, false),
        None => return Ok(None),
      },
    };
    if !required && !path.exists() {
      return Ok(None);
    }
    // A file with only `library` or `position` sets up no server.
    if path.exists() && !Config::local_settings(&path)?.server {
      return Ok(None);
    }
    let config = Config::load(&path)?;
    // On the device's FAT storage every file looks world-readable.
    let warning = match options.live {
      true => None,
      false => config::permission_warning(&path),
    };
    let timeout = options.timeout.max(Duration::from_secs(1));
    let client = Client::with_timeout(&config.server, &config.user, &config.key, timeout);
    Ok(Some(Server {
      config,
      client,
      warning,
    }))
  }

  /// Like [`Server::open`], but a configuration is required.
  pub fn require(options: &Options) -> Result<Self> {
    Server::open(options)?.ok_or_else(|| {
      let path = options
        .config
        .clone()
        .or_else(Config::default_path)
        .unwrap_or_else(|| "config.ini".into());
      let what = if path.exists() {
        "no server set up in"
      } else {
        "no configuration at"
      };
      format!("{what} {}; see README.md for the format", path.display()).into()
    })
  }
}

/// Under which checksum a book is synced, if at all.
#[derive(Debug, Clone)]
pub enum Identity {
  /// The device file's own checksum: the same file as in the library, or
  /// no library to check against.
  Own { hash: String, why: Own },
  /// Mapped to an original EPUB: its checksum, and positions translated.
  Mapped { hash: String, mapping: Mapping },
  /// Not synced: `kind` for the plugin, and why.
  Skip {
    hash: Option<String>,
    kind: &'static str,
    message: String,
  },
}

/// Why a book syncs under its own checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Own {
  /// No library is set up, so nothing was checked: a converted EPUB needs
  /// a mapping to reach KOReader (a kepub always needs one).
  NoLibrary,
  /// The book is the original EPUB of a mapping.
  OriginalOfMapping,
  /// The same file is in the library.
  InLibrary,
}

impl std::fmt::Display for Own {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Own::NoLibrary => "no library configured to check it",
      Own::OriginalOfMapping => "the original EPUB of a mapping",
      Own::InLibrary => "the same file as in the library",
    })
  }
}

impl Identity {
  /// The KOSync document: the checksum the server knows the book by.
  pub fn document(&self) -> Option<&str> {
    match self {
      Identity::Own { hash, .. } => Some(hash),
      Identity::Mapped { mapping, .. } => Some(&mapping.epub.hash),
      Identity::Skip { .. } => None,
    }
  }

  /// The device file's checksum.
  pub fn device_hash(&self) -> Option<&str> {
    match self {
      Identity::Own { hash, .. } | Identity::Mapped { hash, .. } => Some(hash),
      Identity::Skip { hash, .. } => hash.as_deref(),
    }
  }
}

/// What to do with a book that both sides hold, or that only the device
/// holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
  /// Both hold the same position.
  InSync,
  /// Overwrite the other side.
  Apply,
  /// Leave the other side alone: it is newer.
  Keep,
  /// Overwrite only after a yes; `backwards`: the new position lies before
  /// the one it would overwrite.
  Ask { backwards: bool },
}

impl Action {
  /// `force` overwrites without asking, also a newer side.
  fn from(decision: Decision, force: bool) -> Self {
    match decision {
      Decision::InSync => Action::InSync,
      Decision::Apply => Action::Apply,
      _ if force => Action::Apply,
      Decision::Keep => Action::Keep,
      Decision::Ask { backwards, .. } => Action::Ask { backwards },
    }
  }
}

/// What `push` would do with one book.
#[derive(Debug)]
pub enum PushOutcome {
  /// Not sent: `kind` for the plugin, and why.
  Skip { kind: &'static str, message: String },
  /// The plugin found Nickel at `at` instead of the server's position, so
  /// the device position says nothing about reading (only `force` sends).
  NotInLine { at: String },
  /// The server could not be asked.
  Failed(kosync::Error),
  /// Asked the server; `action` says what to do.
  Ready {
    local: Local,
    /// What was not exact about the position.
    note: Option<String>,
    remote: Option<Progress>,
    action: Action,
    /// Sent past a not-in-line entry with `force`: once sent, the server
    /// holds the device's position and the entry goes.
    forced: bool,
  },
}

/// The device's position as an update for the server.
pub fn update(document: &str, local: &Local, device: &str, device_id: &str) -> Update {
  Update {
    document: document.to_string(),
    progress: local.xpointer.clone(),
    percentage: local.percentage,
    device: device.to_string(),
    device_id: device_id.to_string(),
  }
}

/// The server's answer for `local` turned into a push outcome.
fn push_ready(
  local: Local,
  note: Option<String>,
  remote: std::result::Result<Option<Progress>, kosync::Error>,
  confirm: Confirm,
  force: bool,
  forced: bool,
) -> PushOutcome {
  let remote = match remote {
    Ok(remote) => remote,
    Err(e) => return PushOutcome::Failed(e),
  };
  let action = match &remote {
    Some(remote) => Action::from(decide(Direction::Push, &local, remote, confirm), force),
    None => Action::Apply,
  };
  PushOutcome::Ready {
    local,
    note,
    remote,
    action,
    forced,
  }
}

/// Why `pull` pays no heed to the device's time (the plugin's not-in-line
/// list, see [`NotInLine`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plugin {
  /// Nickel opened at its own place instead of the server's, and the book
  /// was not read since: the server's position is taken.
  NothingReadSince,
  /// Read on since Nickel missed the server's position: the time does not
  /// count, only the place in the book.
  ReadOnSince,
}

/// What `pull` would do with one book.
#[derive(Debug)]
pub enum PullOutcome {
  /// Not taken: `kind` for the plugin, and why.
  Skip { kind: &'static str, message: String },
  /// The server holds no position for the book.
  NothingStored,
  /// The server could not be asked.
  Failed(kosync::Error),
  /// Asked the server; `action` says what to do.
  Ready {
    remote: Progress,
    /// `None` if the book was never opened or its position not translated.
    local: Option<Local>,
    plugin: Option<Plugin>,
    action: Action,
  },
}

/// One book's step: under which checksum it syncs, and the outcome.
#[derive(Debug)]
pub struct Step<O> {
  pub identity: Identity,
  pub outcome: O,
}

/// A position from the server as it goes into the device database.
#[derive(Debug)]
pub struct Take {
  pub write: Write,
  /// What was not exact about it.
  pub note: Option<String>,
}

/// Where `Session::write` left its backup.
#[derive(Debug)]
pub enum Written {
  /// Written while Nickel runs; a backup is made once a day.
  Live {
    backup: Option<PathBuf>,
    /// Cached pages dropped so Nickel opens at the place.
    pages_dropped: usize,
  },
  /// Written to a mounted device. With pages dropped, Nickel only takes the
  /// place after a restart.
  Mounted {
    backup: PathBuf,
    pages_dropped: usize,
  },
}

/// A Kobo or Tolino and its sideloaded books.
pub struct Session {
  pub device: Device,
  /// "Kobo" or "Tolino".
  pub family: &'static str,
  /// Model name, or the family.
  pub model: String,
  pub flavor: PointFlavor,
  pub books: Vec<Book>,
  /// Book mappings; none on the device itself (`live`).
  pub store: Store,
  /// The library of originals, if one is configured.
  pub library: Option<Library>,
  /// Checksums of the library's EPUBs, computed when first needed.
  library_hashes: OnceCell<HashSet<String>>,
  /// Which place is sent (`position` in the configuration).
  pub position: PushPosition,
  /// Books the plugin could not bring in line.
  pub not_in_line: NotInLine,
  /// What was found of Kobalt: while it is installed, nothing is written
  /// into the device database.
  pub kobalt: Kobalt,
  /// Only the book with this ContentID (`--content-id`).
  content_id: Option<String>,
}

impl Session {
  pub fn open(options: &Options) -> Result<Self> {
    // On the device itself, an installed Kobalt stops everything before
    // the database is opened: the plugin neither writes nor sends.
    let kobalt = match options.live {
      true => Kobalt::detect_live(&options.device),
      false => Kobalt::detect(&options.device, None),
    };
    if options.live {
      kobalt.check()?;
    }
    let device = Device::open(&options.device, options.live)?;
    let family = family(&device)?;
    let model = device
      .firmware
      .as_ref()
      .and_then(|fw| fw.model_name())
      .unwrap_or(family)
      .to_string();
    // How Nickel counts nodes in an Adobe point depends on the firmware line.
    let flavor = match &device.firmware {
      Some(fw) => PointFlavor::for_firmware(&fw.version),
      None if family == "Tolino" => PointFlavor::AllNodes,
      None => PointFlavor::JoinText,
    };
    let books = select(
      device.db.sideloaded_books()?,
      options.book.as_deref(),
      options.content_id.as_deref(),
    );
    let settings = options.local_settings()?;
    // The mappings and the library live on the desktop; the plugin syncs
    // every book under its own checksum, as before.
    let (store, library) = if options.live {
      (Store::default(), None)
    } else {
      let path = options.library.clone().or(settings.library);
      let library = match path {
        Some(path) => Some(Library::open(&path)?),
        None => None,
      };
      (load_store()?, library)
    };
    let not_in_line = NotInLine::load(&device.root)?;
    Ok(Session {
      device,
      family,
      model,
      flavor,
      books,
      store,
      library,
      library_hashes: OnceCell::new(),
      position: settings.position,
      not_in_line,
      kobalt,
      content_id: options.content_id.clone(),
    })
  }

  /// Under which checksum the book is synced. A mapping wins over the
  /// file's own checksum; with a library, a book that is neither mapped
  /// nor the same file as one there is not synced.
  pub fn identity(&self, book: &Book) -> Identity {
    let hash = match self.hash(book) {
      Ok(hash) => hash,
      Err(message) => {
        return Identity::Skip {
          hash: None,
          kind: "skipped",
          message,
        }
      }
    };
    if let Some(mapping) = self.store.for_file(&hash) {
      return match mapping.check() {
        EpubState::Ok => Identity::Mapped {
          hash,
          mapping: mapping.clone(),
        },
        state => Identity::Skip {
          hash: Some(hash),
          kind: "mapping-broken",
          message: format!(
            "not synced: the mapping to {} is {state}; map it again with `map add`",
            mapping.epub.path
          ),
        },
      };
    }
    // A kepub never reaches the server under its own checksum: KOReader
    // knows the book by its original EPUB, so only a mapping syncs it.
    if book.is_kepub() || book.onboard_path().ends_with(".kepub.epub") {
      return Identity::Skip {
        hash: Some(hash),
        kind: "not-mapped",
        message: "not mapped: a kepub syncs only under its original EPUB; map it with \
                  `nickelinksync map add` (see `map suggest`)"
          .into(),
      };
    }
    let Some(library) = &self.library else {
      return Identity::Own {
        hash,
        why: Own::NoLibrary,
      };
    };
    if self.store.mappings.iter().any(|m| m.epub.hash == hash) {
      return Identity::Own {
        hash,
        why: Own::OriginalOfMapping,
      };
    }
    let in_library = self
      .library_hashes
      .get_or_init(|| library.books.iter().filter_map(|b| b.hash().ok()).collect());
    if in_library.contains(&hash) {
      Identity::Own {
        hash,
        why: Own::InLibrary,
      }
    } else {
      Identity::Skip {
        hash: Some(hash),
        kind: "not-mapped",
        message: "not mapped: neither mapped nor the same file as in the library, so not \
                  synced; see `nickelinksync map suggest`"
          .into(),
      }
    }
  }

  fn hash(&self, book: &Book) -> std::result::Result<String, String> {
    partial_md5(&self.device.book_file(book)).map_err(|e| format!("file not readable: {e}"))
  }

  /// The stored position as a KOReader XPointer.
  fn xpointer(&self, book: &Book, stored: &str) -> std::result::Result<XPointer, String> {
    Epub::open(&self.device.book_file(book))
      .map_err(|e| e.to_string())
      .and_then(|mut epub| {
        to_xpointer(&mut epub, &Position::parse(stored), self.flavor).map_err(|e| e.to_string())
      })
  }

  /// A server XPointer (in the original EPUB, for a mapped book) as the
  /// device would store it, and what was not exact about it.
  fn translate(
    &self,
    book: &Book,
    identity: &Identity,
    xpointer: &str,
    percentage: f64,
  ) -> std::result::Result<(Translated, Option<String>), String> {
    let mut device = Epub::open(&self.device.book_file(book)).map_err(|e| e.to_string())?;
    let (xpointer, how) = match identity {
      Identity::Mapped { mapping, .. } => {
        let mut original = Epub::open(Path::new(&mapping.epub.path)).map_err(|e| e.to_string())?;
        let t =
          transfer(&mut original, xpointer, &mut device, percentage).map_err(|e| e.to_string())?;
        (t.xpointer, Some(t.how))
      }
      _ => (xpointer.to_string(), None),
    };
    // Nickel reads a kepub as such and expects a `#kobo.N.M` span.
    let translated = if book.is_kepub() {
      from_xpointer_kepub(&mut device, &xpointer, self.flavor)
    } else {
      from_xpointer(&mut device, &xpointer, self.flavor)
    }
    .map_err(|e| e.to_string())?;
    let note = match how {
      Some(how) if how != How::SameText => Some(how.to_string()),
      _ if !translated.exact => Some("chapter only".to_string()),
      _ => None,
    };
    Ok((translated, note))
  }

  /// `local` to compare with the server's position: if the device holds
  /// exactly what pulling `remote` writes, it is the server's position.
  /// The way back to an XPointer is not exact (it moves to the paragraph
  /// start, or the chapter), so after a pull the device would otherwise
  /// never be in sync.
  pub fn against(
    &self,
    book: &Book,
    identity: &Identity,
    local: Local,
    remote: Option<&Progress>,
  ) -> Local {
    let (Some(remote), Some(stored)) = (remote, book.position.as_deref()) else {
      return local;
    };
    if local.xpointer == remote.progress {
      return local;
    }
    match self.translate(book, identity, &remote.progress, remote.percentage) {
      Ok((translated, _)) if translated.position.to_string() == stored => Local {
        xpointer: remote.progress.clone(),
        percentage: remote.percentage,
        ..local
      },
      _ => local,
    }
  }

  /// The device position for KOSync (in the original EPUB, for a mapped
  /// book), and what was not exact about it; or why there is none.
  pub fn local(
    &self,
    book: &Book,
    identity: &Identity,
  ) -> std::result::Result<(Local, Option<String>), String> {
    let stored = book.position.as_deref().ok_or("never opened")?;
    let x = self
      .xpointer(book, stored)
      .map_err(|e| format!("not translated: {e}"))?;
    let percentage = book.percent_read as f64 / 100.0;
    let device_file = self.device.book_file(book);
    let (xpointer, mut notes, sent_file) = match identity {
      Identity::Mapped { mapping, .. } => {
        let original = PathBuf::from(&mapping.epub.path);
        let moved = Epub::open(&device_file)
          .and_then(|device| Ok((device, Epub::open(&original)?)))
          .map_err(|e| e.to_string())
          .and_then(|(mut device, mut epub)| {
            transfer(&mut device, &x.xpointer, &mut epub, percentage).map_err(|e| e.to_string())
          })
          .map_err(|e| format!("not translated into the original EPUB: {e}"))?;
        let notes: Vec<String> = (moved.how != How::SameText)
          .then(|| moved.how.to_string())
          .into_iter()
          .collect();
        (moved.xpointer, notes, original)
      }
      _ => {
        let notes = (!x.exact).then(|| "chapter only".to_string());
        (x.xpointer, notes.into_iter().collect(), device_file)
      }
    };
    // The start of the sentence or paragraph, which is easier to find on
    // KOReader's page than a place mid-sentence.
    let stepped = Epub::open(&sent_file)
      .map_err(|e| e.to_string())
      .and_then(|mut epub| {
        step_back(&mut epub, &xpointer, self.position).map_err(|e| e.to_string())
      })
      .map_err(|e| format!("not translated: {e}"))?;
    let xpointer = match stepped {
      Some(s) => {
        notes.push(s.to_string());
        s.xpointer
      }
      None => xpointer,
    };
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    let local = Local {
      xpointer,
      percentage: book.percent_read as f64 / 100.0,
      last_read: book.last_read.as_deref().and_then(parse_timestamp),
    };
    Ok((local, note))
  }

  /// What `push` would do with `book`; asks the server. `force` sends
  /// without asking, also past a newer server position or a not-in-line
  /// entry.
  pub fn push_step(
    &self,
    book: &Book,
    client: &Client,
    confirm: Confirm,
    force: bool,
  ) -> Step<PushOutcome> {
    let identity = self.identity(book);
    let outcome = self.push_outcome(book, &identity, client, confirm, force);
    Step { identity, outcome }
  }

  fn push_outcome(
    &self,
    book: &Book,
    identity: &Identity,
    client: &Client,
    confirm: Confirm,
    force: bool,
  ) -> PushOutcome {
    let Some(document) = identity.document() else {
      let Identity::Skip { kind, message, .. } = identity else {
        unreachable!("only a skipped book has no document")
      };
      return PushOutcome::Skip {
        kind,
        message: message.clone(),
      };
    };
    let (local, note) = match self.local(book, identity) {
      Ok(local) => local,
      Err(message) => {
        return PushOutcome::Skip {
          kind: "skipped",
          message,
        }
      }
    };
    let forced = match self.not_in_line.get(&book.content_id) {
      Some(at) if !force => return PushOutcome::NotInLine { at: at.to_string() },
      Some(_) => true,
      None => false,
    };
    let remote = client.progress(document);
    let local = match &remote {
      Ok(remote) => self.against(book, identity, local, remote.as_ref()),
      Err(_) => local,
    };
    push_ready(local, note, remote, confirm, force, forced)
  }

  /// What `pull` would do with `book`; asks the server. `force` takes the
  /// server's position without asking, also if the device's is newer.
  pub fn pull_step(
    &self,
    book: &Book,
    client: &Client,
    confirm: Confirm,
    force: bool,
  ) -> Step<PullOutcome> {
    let identity = self.identity(book);
    let outcome = self.pull_outcome(book, &identity, client, confirm, force);
    Step { identity, outcome }
  }

  fn pull_outcome(
    &self,
    book: &Book,
    identity: &Identity,
    client: &Client,
    confirm: Confirm,
    force: bool,
  ) -> PullOutcome {
    let Some(document) = identity.document() else {
      let Identity::Skip { kind, message, .. } = identity else {
        unreachable!("only a skipped book has no document")
      };
      return PullOutcome::Skip {
        kind,
        message: message.clone(),
      };
    };
    let remote = match client.progress(document) {
      Ok(Some(remote)) => remote,
      Ok(None) => return PullOutcome::NothingStored,
      Err(e) => return PullOutcome::Failed(e),
    };
    // Without a device position there is nothing on the device to keep.
    let local = self
      .local(book, identity)
      .ok()
      .map(|(local, _)| self.against(book, identity, local, Some(&remote)));
    // After a failed take Nickel's DateLastRead says nothing about reading.
    let left_by_nickel = self.not_in_line.get(&book.content_id);
    let (decision, plugin) = match (&local, left_by_nickel) {
      (Some(_), Some(at)) if book.position.as_deref() == Some(at) => {
        (Decision::Apply, Some(Plugin::NothingReadSince))
      }
      (Some(local), Some(_)) => {
        let undated = Local {
          last_read: None,
          ..local.clone()
        };
        (
          decide(Direction::Pull, &undated, &remote, confirm),
          Some(Plugin::ReadOnSince),
        )
      }
      (Some(local), None) => (decide(Direction::Pull, local, &remote, confirm), None),
      (None, _) => (Decision::Apply, None),
    };
    PullOutcome::Ready {
      remote,
      local,
      plugin,
      action: Action::from(decision, force),
    }
  }

  /// The server's position for `book` as a write to the device database.
  pub fn take(
    &self,
    book: &Book,
    identity: &Identity,
    remote: &Progress,
    local: Option<&Local>,
  ) -> std::result::Result<Take, String> {
    let (translated, note) = self.translate(book, identity, &remote.progress, remote.percentage)?;
    // Keep the server the newer side, so the next push does not send the
    // old device position back: DateLastRead stays as it is unless it is
    // newer than the server's position.
    let last_read = local
      .and_then(|l| l.last_read)
      .filter(|&t| t >= remote.timestamp)
      .map(|_| iso_timestamp(remote.timestamp - 1));
    let percent = (remote.percentage * 100.0).round().clamp(0.0, 100.0) as i64;
    Ok(Take {
      write: Write {
        content_id: book.content_id.clone(),
        position: translated.position.to_string(),
        percent,
        last_read,
      },
      note,
    })
  }

  /// Writes the positions into the device database after a backup of it,
  /// and clears their not-in-line entries.
  /// Refused while Kobalt is installed ([`crate::kobalt::Active`]).
  pub fn write(&mut self, writes: &[Write]) -> Result<Written> {
    self.kobalt.check()?;
    let dir = Config::backup_dir().ok_or("no directory for the backup (HOME is not set)")?;
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)?
      .as_secs() as i64;
    let stamp: String = iso_timestamp(now)
      .chars()
      .filter(|c| !"-:".contains(*c))
      .collect();
    let written = if self.device.live {
      // Only the one book the plugin is about to open.
      if self.content_id.is_none() {
        return Err("--write with --live needs --content-id".into());
      }
      let (backup, pages_dropped) = self.device.write_live(writes, &dir, &stamp[..8])?;
      Written::Live {
        backup,
        pages_dropped,
      }
    } else {
      // A second write in the same second (two clicks in the desktop
      // client) gets a backup of its own.
      let backup = restore::new_path(&dir)?;
      let pages_dropped = self.device.write_positions(writes, &backup)?;
      restore::tag(&backup, &self.device.root)?;
      Written::Mounted {
        backup,
        pages_dropped,
      }
    };
    self
      .not_in_line
      .remove(writes.iter().map(|w| w.content_id.as_str()))?;
    Ok(written)
  }
}

/// "Kobo" or "Tolino". Kobo firmware 6.0 has the `tolino_*` tables too,
/// so the model in `.kobo/version` decides, then the reseller in
/// `.kobo/affiliate.conf` (`Kobo`, or a Tolino bookseller such as
/// `Thalia.de`), and only without both the tables.
fn family(device: &Device) -> Result<&'static str> {
  if let Some(name) = device.firmware.as_ref().and_then(|fw| fw.model_name()) {
    return Ok(if name.starts_with("Tolino") {
      "Tolino"
    } else {
      "Kobo"
    });
  }
  let affiliate = std::fs::read_to_string(device.root.join(".kobo/affiliate.conf"))
    .ok()
    .and_then(|text| {
      text.lines().find_map(|l| {
        l.trim()
          .strip_prefix("affiliate=")
          .map(|v| v.trim().to_string())
      })
    });
  Ok(match affiliate {
    Some(a) if a.eq_ignore_ascii_case("kobo") => "Kobo",
    Some(_) => "Tolino",
    None if device.db.is_tolino()? => "Tolino",
    None => "Kobo",
  })
}

/// The books whose title or path contains `filter` (any case), and the one
/// with exactly `content_id`.
pub fn select(mut books: Vec<Book>, filter: Option<&str>, content_id: Option<&str>) -> Vec<Book> {
  if let Some(filter) = filter {
    let filter = filter.to_lowercase();
    books.retain(|b| {
      b.onboard_path().to_lowercase().contains(&filter)
        || b
          .title
          .as_deref()
          .is_some_and(|t| t.to_lowercase().contains(&filter))
    });
  }
  if let Some(id) = content_id {
    books.retain(|b| b.content_id == id);
  }
  books
}

/// The Kindle books whose path, title or author contains `filter` (any
/// case), and the one with `content_id`.
pub fn kindle_matches(
  root: &Path,
  mut books: Vec<KindleBook>,
  filter: Option<&str>,
  content_id: Option<&str>,
) -> Vec<KindleBook> {
  if let Some(filter) = filter {
    let filter = filter.to_lowercase();
    books.retain(|b| {
      b.relative_path(root)
        .to_string_lossy()
        .to_lowercase()
        .contains(&filter)
        || [&b.title, &b.author].iter().any(|t| {
          t.as_deref()
            .is_some_and(|t| t.to_lowercase().contains(&filter))
        })
    });
  }
  if let Some(id) = content_id {
    books.retain(|b| b.content_id.eq_ignore_ascii_case(id));
  }
  books
}

/// What `push` would do with one Kindle book: its mapping (`None` if it
/// has none that works) and the outcome.
#[derive(Debug)]
pub struct KindleStep {
  pub mapping: Option<Mapping>,
  pub outcome: PushOutcome,
}

/// A Kindle: a mounted one, or a copy of its `documents` folder (a stock
/// Kindle on a Mac only offers MTP). Each KFX book syncs under the EPUB it
/// is mapped to; nothing is written to it.
pub struct KindleSession {
  pub root: PathBuf,
  /// The KFX books matching the options' filters.
  pub books: Vec<KindleBook>,
  /// How many KFX books there are, before the filters.
  pub found: usize,
  /// KFX files that could not be read.
  pub unreadable: Vec<Unreadable>,
  pub store: Store,
  pub position: PushPosition,
}

impl KindleSession {
  pub fn open(options: &Options) -> Result<Self> {
    let root = &options.device;
    let (books, unreadable) = kindle_device::books(root)?;
    let found = books.len();
    let books = kindle_matches(
      root,
      books,
      options.book.as_deref(),
      options.content_id.as_deref(),
    );
    let settings = options.local_settings()?;
    Ok(KindleSession {
      root: root.clone(),
      books,
      found,
      unreadable,
      store: load_store()?,
      position: settings.position,
    })
  }

  /// The book's mapping, or why it is not synced.
  pub fn mapping(&self, book: &KindleBook) -> std::result::Result<&Mapping, String> {
    let mapping = self.store.for_kindle(&book.content_id).ok_or(
      "not mapped: a KFX syncs only under the EPUB it is mapped to; see `nickelinksync map suggest`",
    )?;
    match mapping.check() {
      EpubState::Ok => Ok(mapping),
      state => Err(format!(
        "not synced: the mapping to {} is {state}; map it again with `map add`",
        mapping.epub.path
      )),
    }
  }

  /// The position for KOSync in the mapped EPUB, what was not exact about
  /// it, and the stored position for display.
  pub fn local(
    &self,
    book: &KindleBook,
    mapping: &Mapping,
  ) -> std::result::Result<(Local, Option<String>, String), String> {
    let state = book.reading_state()?.ok_or("never opened (no .yjf)")?;
    let lpr = state.last_read.ok_or("never opened (no last page read)")?;
    let kfx = Kfx::open(&book.kfx).map_err(|e| e.to_string())?;
    let mut epub = Epub::open(Path::new(&mapping.epub.path)).map_err(|e| e.to_string())?;
    let x = kindle::to_xpointer(&kfx, &mut epub, &lpr.position)
      .map_err(|e| format!("not translated: {e}"))?;
    let mut notes = Vec::new();
    if !x.exact {
      notes.push("block not found exactly".to_string());
    }
    let xpointer =
      match step_back(&mut epub, &x.xpointer, self.position).map_err(|e| e.to_string())? {
        Some(s) => {
          notes.push(s.to_string());
          s.xpointer
        }
        None => x.xpointer,
      };
    let total = kfx.pid_count().max(1);
    let shown = format!(
      "block {}, offset {} (position {} of {total})",
      lpr.position.eid, lpr.position.offset, lpr.position.pid
    );
    let local = Local {
      xpointer,
      percentage: (f64::from(lpr.position.pid) / f64::from(total)).clamp(0.0, 1.0),
      last_read: lpr.time_ms.map(|ms| ms / 1000),
    };
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    Ok((local, note, shown))
  }

  /// What `push` would do with `book`; asks the server.
  pub fn push_step(
    &self,
    book: &KindleBook,
    client: &Client,
    confirm: Confirm,
    force: bool,
  ) -> KindleStep {
    let mapping = match self.mapping(book) {
      Ok(mapping) => mapping.clone(),
      Err(message) => {
        return KindleStep {
          mapping: None,
          outcome: PushOutcome::Skip {
            kind: "skipped",
            message,
          },
        }
      }
    };
    let outcome = match self.local(book, &mapping) {
      Ok((local, note, _)) => push_ready(
        local,
        note,
        client.progress(&mapping.epub.hash),
        confirm,
        force,
        false,
      ),
      Err(message) => PushOutcome::Skip {
        kind: "skipped",
        message,
      },
    };
    let mapping = match outcome {
      PushOutcome::Skip { .. } => None,
      _ => Some(mapping),
    };
    KindleStep { mapping, outcome }
  }
}
