//! Everything slow (searching drives, hashing and reading books, comparing
//! library books, the server, later copying) runs on one worker
//! thread, so the window never waits. The window sends [`Command`]s; the
//! worker answers with [`Event`]s and asks for a repaint after each.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use nickelinksync::history::Entry;
use nickelinksync::library::Library;

use crate::devices::{self, Found, Kind};
use crate::model::{self, Loaded, Mapped, Suggestions};
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
  /// Send the positions of the questions answered with "device".
  Answer {
    root: PathBuf,
    kind: Kind,
    questions: Vec<Question>,
  },
  /// The last entries of the sync history.
  History,
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
  /// What a push did; the server is asked again after it.
  Pushed {
    root: PathBuf,
    result: Result<Report, String>,
  },
  History(Result<Vec<Entry>, String>),
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
        send(Event::Pushed {
          root: root.clone(),
          result,
        });
        compare(root, kind);
      }
      Command::Answer {
        root,
        kind,
        questions,
      } => {
        let result = sync::send_answers(&model::options(&root), questions);
        send(Event::Pushed {
          root: root.clone(),
          result,
        });
        compare(root, kind);
      }
      Command::History => send(Event::History(sync::history(200))),
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
