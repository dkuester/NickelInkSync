//! Everything slow (searching drives, hashing and reading books, later the
//! network and copying) runs on one worker thread, so the window never
//! waits. The window sends [`Command`]s; the worker answers with
//! [`Event`]s and asks for a repaint after each.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::devices::{self, Found, Kind};
use crate::model::{self, Loaded};

pub enum Command {
  /// Search the drives, plus the chosen Kindle copy.
  Scan { kindle_folder: Option<PathBuf> },
  /// Read the books of a device.
  Load { root: PathBuf, kind: Kind },
}

pub enum Event {
  Devices(Vec<Found>),
  Progress {
    root: PathBuf,
    done: usize,
    total: usize,
  },
  Loaded {
    root: PathBuf,
    result: Result<Loaded, String>,
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
      Command::Load { root, kind } => {
        let result = model::load(&root, kind, &mut |done, total| {
          send(Event::Progress {
            root: root.clone(),
            done,
            total,
          })
        });
        send(Event::Loaded { root, result });
      }
    }
  }
}
