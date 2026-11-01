//! Everything slow (searching drives, hashing and reading books, comparing
//! library books, the server, later copying) runs on one worker
//! thread, so the window never waits. The window sends [`Command`]s; the
//! worker answers with [`Event`]s and asks for a repaint after each.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use nickelinksync::config::{self, Config, Editable};
use nickelinksync::copy::{self, Copied, Format};
use nickelinksync::history::Entry;
use nickelinksync::library::Library;

use crate::devices::{self, Found, Kind};
use crate::model::{self, Loaded, Mapped, Suggestions};
use crate::overview::{self, Overview};
use crate::sync::{self, Question, Report, ServerState};

pub enum Command {
  /// Search the drives, plus the chosen Kindle copy.
  Scan { kindle_folder: Option<PathBuf> },
  /// Read the books of a device, then ask the server about them.
  Load { root: PathBuf, kind: Kind },
  /// Push the books with `ids`, then ask the server again.
  Push {
    root: PathBuf,
    kind: Kind,
    ids: Vec<String>,
  },
  /// Write the server's positions for the books with `ids` into a Tolino,
  /// then read it again.
  Pull { root: PathBuf, ids: Vec<String> },
  /// Do what the questions answered with yes ask for: send the device's
  /// positions, or write the server's.
  Answer {
    root: PathBuf,
    kind: Kind,
    questions: Vec<Question>,
  },
  /// The last entries of the sync history.
  History,
  /// Every device's position of each book side by side, with the server's.
  Overview { devices: Vec<Found> },
  /// The library's books, to choose some to copy.
  LibraryBooks,
  /// Copy the EPUBs at `books` onto the device (as kepubs: `kepub`), then
  /// read it again.
  Copy {
    root: PathBuf,
    books: Vec<PathBuf>,
    kepub: bool,
  },
  /// The backups of the device database that may go back.
  Backups { root: PathBuf },
  /// Put a backup back onto the device, then read it again.
  Restore { root: PathBuf, backup: PathBuf },
  /// Ask GitHub for a newer release.
  CheckRelease,
  /// Prepare a Kobo or Tolino to sync on its own (index, mappings,
  /// tables), then read it again.
  Prepare { root: PathBuf },
  /// The settings in `config.ini`.
  ReadSettings,
  /// Write the settings; a typed password goes in as its key.
  SaveSettings {
    edit: Editable,
    password: Option<String>,
  },
  /// Move `password`/`key` from `config.ini` into the keychain.
  MoveToKeychain,
  /// The library's candidates for one book.
  Suggest { root: PathBuf, id: String },
  /// Map one book to an EPUB, then read the device again.
  Map {
    root: PathBuf,
    kind: Kind,
    id: String,
    epub: PathBuf,
  },
  /// Remove a mapping, then read the device again.
  Unmap {
    root: PathBuf,
    kind: Kind,
    key: String,
  },
}

pub enum Event {
  Devices(Vec<Found>),
  /// The server's positions for the books of a device.
  Compared {
    root: PathBuf,
    state: ServerState,
  },
  /// What a push or pull did; the server is asked again after it, and the
  /// device read again after a write.
  Synced {
    root: PathBuf,
    result: Result<Report, String>,
  },
  History(Result<Vec<Entry>, String>),
  Overview(Overview),
  Settings(Result<Editable, String>),
  /// How far preparing a device got.
  Preparing {
    root: PathBuf,
    done: usize,
    total: usize,
  },
  /// What preparing a device did, or why not; it is read again after it.
  Prepared {
    root: PathBuf,
    result: Result<nickelinksync::prepare::Report, String>,
  },
  /// A newer release, none, or why GitHub could not be asked.
  Release(Result<Option<crate::release::Release>, String>),
  Backups(Result<Vec<nickelinksync::restore::Backup>, String>),
  /// A backup was put back (the backup of the state before), or why not.
  Restored {
    root: PathBuf,
    result: Result<PathBuf, String>,
  },
  LibraryBooks(Result<Vec<nickelinksync::library::LibraryBook>, String>),
  /// What copying did, by title; the device is read again after it.
  Copied {
    root: PathBuf,
    result: Result<Vec<(String, Copied)>, String>,
  },
  /// Settings saved (`false`) or the key moved (`true`), or why not.
  SettingsSaved(Result<bool, String>),
  Progress {
    root: PathBuf,
    done: usize,
    total: usize,
  },
  Loaded {
    root: PathBuf,
    result: Result<Loaded, String>,
  },
  Suggestions {
    id: String,
    result: Result<Suggestions, String>,
  },
  /// A mapping was made (or failed); the device is read again after it.
  Mapped {
    id: String,
    result: Result<Mapped, String>,
  },
  /// A mapping was removed (or not); the device is read again after it.
  Unmapped {
    result: Result<(), String>,
  },
}

pub struct Worker {
  commands: Sender<Command>,
  pub events: Receiver<Event>,
}

impl Worker {
  /// Starts the thread; `repaint` wakes the window after each event.
  pub fn start(repaint: impl Fn() + Send + 'static) -> Self {
    let (commands, inbox) = mpsc::channel();
    let (outbox, events) = mpsc::channel();
    thread::spawn(move || run(&inbox, &outbox, &repaint));
    Worker { commands, events }
  }

  pub fn send(&self, command: Command) {
    // The worker only stops when the window is gone.
    let _ = self.commands.send(command);
  }
}

fn run(inbox: &Receiver<Command>, outbox: &Sender<Event>, repaint: &dyn Fn()) {
  let send = |event| {
    let _ = outbox.send(event);
    repaint();
  };
  let compare = |root: PathBuf, kind: Kind| {
    let state = sync::compare(&model::options(&root), kind);
    send(Event::Compared { root, state });
  };
  let load = |root: PathBuf, kind: Kind| {
    let result = model::load(&root, kind, &mut |done, total| {
      send(Event::Progress {
        root: root.clone(),
        done,
        total,
      })
    });
    let loaded = result.is_ok();
    send(Event::Loaded {
      root: root.clone(),
      result,
    });
    if loaded {
      compare(root, kind);
    }
  };
  let synced = |root: PathBuf, kind: Kind, result: Result<Report, String>| {
    let wrote = result.as_ref().is_ok_and(|r| r.backup.is_some());
    send(Event::Synced {
      root: root.clone(),
      result,
    });
    if wrote {
      load(root, kind);
    } else {
      compare(root, kind);
    }
  };
  // Read once, when first needed: a big library takes a while.
  let mut library: Option<Option<Library>> = None;
  while let Ok(command) = inbox.recv() {
    match command {
      Command::Scan { kindle_folder } => {
        let mut found = devices::scan(&devices::mount_parents(), &devices::drive_roots());
        if let Some(copy) = kindle_folder.as_deref().and_then(devices::kindle_folder) {
          if !found.iter().any(|f| f.root == copy.root) {
            found.push(copy);
          }
        }
        send(Event::Devices(found));
      }
      Command::Load { root, kind } => load(root, kind),
      Command::Push { root, kind, ids } => {
        let result = sync::push(&model::options(&root), kind, &ids);
        synced(root, kind, result);
      }
      Command::Pull { root, ids } => {
        let result = sync::pull(&model::options(&root), &ids);
        synced(root, Kind::Nickel, result);
      }
      Command::Answer {
        root,
        kind,
        questions,
      } => {
        let result = sync::answer(&model::options(&root), kind, questions);
        synced(root, kind, result);
      }
      Command::History => send(Event::History(sync::history(200))),
      Command::Overview { devices } => send(Event::Overview(overview::build(&devices))),
      Command::LibraryBooks => {
        let result = match &library {
          Some(library) => Ok(library.as_ref()),
          None => model::library().map(|l| library.insert(l).as_ref()),
        }
        .and_then(|l| {
          l.map(|l| l.books.clone())
            .ok_or_else(|| "no library set up (library = in config.ini)".to_string())
        });
        send(Event::LibraryBooks(result));
      }
      Command::Copy { root, books, kepub } => {
        let format = if kepub { Format::Kepub } else { Format::Epub };
        let kepubify = copy::kepubify();
        let result = nickelinksync::session::load_store()
          .map_err(|e| e.to_string())
          .map(|mut store| {
            books
              .iter()
              .map(|book| {
                let title = book
                  .file_name()
                  .map(|n| n.to_string_lossy().into_owned())
                  .unwrap_or_default();
                let copied = copy::copy(&root, book, format, kepubify.as_deref(), &mut store);
                (title, copied)
              })
              .collect()
          });
        send(Event::Copied {
          root: root.clone(),
          result,
        });
        load(root, Kind::Nickel);
      }
      Command::Backups { root } => send(Event::Backups(
        nickelinksync::restore::list(&root).map_err(|e| e.to_string()),
      )),
      Command::Restore { root, backup } => {
        let result = nickelinksync::restore::restore(&root, &backup).map_err(|e| e.to_string());
        let restored = result.is_ok();
        send(Event::Restored {
          root: root.clone(),
          result,
        });
        if restored {
          load(root, Kind::Nickel);
        }
      }
      Command::CheckRelease => send(Event::Release(crate::release::check())),
      Command::Prepare { root } => {
        let result = match &library {
          Some(library) => Ok(library.as_ref()),
          None => model::library().map(|l| library.insert(l).as_ref()),
        }
        .and_then(|l| l.ok_or_else(|| "no library set up (library = in config.ini)".to_string()))
        .and_then(|library| {
          let mut store = nickelinksync::session::load_store().map_err(|e| e.to_string())?;
          nickelinksync::prepare::prepare(&root, library, &mut store, &mut |done, total| {
            send(Event::Preparing {
              root: root.clone(),
              done,
              total,
            })
          })
          .map_err(|e| e.to_string())
        });
        send(Event::Prepared {
          root: root.clone(),
          result,
        });
        load(root, Kind::Nickel);
      }
      Command::ReadSettings => send(Event::Settings(
        config_path().and_then(|p| Editable::read(&p).map_err(|e| e.to_string())),
      )),
      Command::SaveSettings { edit, password } => {
        let result = config_path().and_then(|p| {
          edit
            .save(&p, password.as_deref())
            .map_err(|e| e.to_string())
        });
        send(Event::SettingsSaved(result.map(|()| false)));
      }
      Command::MoveToKeychain => {
        let result =
          config_path().and_then(|p| config::move_to_keychain(&p).map_err(|e| e.to_string()));
        send(Event::SettingsSaved(result.map(|()| true)));
      }
      Command::Suggest { root, id } => {
        let result = match &library {
          Some(library) => Ok(library.as_ref()),
          None => model::library().map(|l| library.insert(l).as_ref()),
        }
        .and_then(|library| model::suggest(&root, &id, library));
        send(Event::Suggestions { id, result });
      }
      Command::Map {
        root,
        kind,
        id,
        epub,
      } => {
        let result = model::map_book(&root, &id, &epub);
        send(Event::Mapped { id, result });
        load(root, kind);
      }
      Command::Unmap { root, kind, key } => {
        send(Event::Unmapped {
          result: model::unmap(&key),
        });
        load(root, kind);
      }
    }
  }
}

/// `config.ini`, where the CLI looks for it.
fn config_path() -> Result<std::path::PathBuf, String> {
  Config::default_path().ok_or_else(|| "no configuration folder (HOME is not set)".into())
}
