//! The window: devices on the left, the chosen device's books in the
//! middle, grouped by how they sync, with the server's position for each;
//! dialogs to map a book to its original EPUB and to answer the `confirm`
//! questions of a push or pull; the sync history.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use nickelinksync::copy;
use nickelinksync::history::{self, Entry};
use nickelinksync::library;
use nickelinksync::session::Own;
use nickelinksync::suggest;
use nickelinksync::sync::{format_timestamp, Outlook};

use crate::copy_dialog::{self, CopyDialog};
use crate::devices::{Found, Kind};
use crate::i18n::{self, Choice, Lang, Texts};
use crate::model::{Group, Loaded, Mapped, Row, Suggestions};
use crate::overview::Overview;
use crate::restore_dialog::{self, RestoreDialog};
use crate::settings::Settings;
use crate::settings_dialog::{self, Request, SettingsDialog};
use crate::sync::{Compared, Question, ServerState};
use crate::worker::{Command, Event, Worker};

/// How often the drives are searched again.
const SCAN_EVERY: Duration = Duration::from_secs(2);

enum View {
  Nothing,
  Loading { done: usize, total: usize },
  Failed(String),
  Loaded(Loaded),
}

/// What a click in the book list asks for.
enum Action {
  /// Open the mapping dialog for the book with `id`.
  Map { id: String, title: String },
  /// Ask before removing the mapping with `key`.
  Unmap { key: String, title: String },
  /// Push the books with these ids.
  Push(Vec<String>),
  /// Take the server's positions for the books with these ids (Tolino).
  Pull(Vec<String>),
  /// Choose books of the library to copy onto the device.
  AddFromLibrary { kobo: bool },
  /// Choose a backup of the device database to put back.
  Restore,
  /// Ask before pushing a book without a reference EPUB.
  PushUnreferenced { id: String, title: String },
}

/// The library's candidates, while they are being compared and after.
enum Candidates {
  Loading,
  Ready(Suggestions),
  Failed(String),
}

/// Mapping one book: the library's candidates, or an EPUB chosen by hand.
struct MapDialog {
  id: String,
  title: String,
  candidates: Candidates,
  chosen: Option<PathBuf>,
  /// The mapping was sent; waiting for the answer.
  busy: bool,
  error: Option<String>,
  /// Typed path of the EPUB, where there is no file dialog.
  #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
  typed: String,
}

/// Asking before a mapping is removed.
struct UnmapDialog {
  key: String,
  title: String,
}

/// The `confirm` questions of a push or pull: send the device's position,
/// or take the server's (`true`), or leave both as they are.
struct AskDialog {
  questions: Vec<(Question, bool)>,
}

pub struct App {
  worker: Worker,
  settings: Settings,
  settings_path: Option<PathBuf>,
  devices: Vec<Found>,
  /// A scan was sent and not answered yet.
  scanning: bool,
  last_scan: Option<Instant>,
  selected: Option<Found>,
  view: View,
  /// Shown under the Kindle button after a folder that is no Kindle copy.
  kindle_error: bool,
  /// Typed path of the Kindle copy, where there is no folder dialog.
  kindle_path: String,
  map_dialog: Option<MapDialog>,
  unmap_dialog: Option<UnmapDialog>,
  /// What the last mapping change or push did, shown above the books.
  notice: Option<Result<String, String>>,
  /// The server's positions for the shown device; `None` while asking.
  server: Option<ServerState>,
  /// A push or pull was sent and not answered yet.
  busy: bool,
  ask_dialog: Option<AskDialog>,
  /// A book without a reference EPUB, to send only after a yes.
  unreferenced: Option<(String, String)>,
  /// The history window: open, and its entries once read.
  history_open: bool,
  history: Option<Result<Vec<Entry>, String>>,
  /// The overview window: open, and its contents once read.
  overview_open: bool,
  overview: Option<Overview>,
  settings_dialog: Option<SettingsDialog>,
  copy_dialog: Option<CopyDialog>,
  restore_dialog: Option<RestoreDialog>,
}

impl App {
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
    let ctx = cc.egui_ctx.clone();
    let settings_path = Settings::default_path();
    let settings = settings_path
      .as_deref()
      .map(Settings::load)
      .unwrap_or_default();
    let kindle_path = settings
      .kindle_folder
      .as_ref()
      .map(|p| p.display().to_string())
      .unwrap_or_default();
    App {
      worker: Worker::start(move || ctx.request_repaint()),
      settings,
      settings_path,
      devices: Vec::new(),
      scanning: false,
      last_scan: None,
      selected: None,
      view: View::Nothing,
      kindle_error: false,
      kindle_path,
      map_dialog: None,
      unmap_dialog: None,
      notice: None,
      server: None,
      busy: false,
      ask_dialog: None,
      unreferenced: None,
      history_open: false,
      overview_open: false,
      overview: None,
      settings_dialog: None,
      copy_dialog: None,
      restore_dialog: None,
      history: None,
    }
  }

  fn lang(&self) -> Lang {
    self.settings.language.lang()
  }

  fn save_settings(&self) {
    if let Some(path) = &self.settings_path {
      if let Err(e) = self.settings.save(path) {
        eprintln!("nickelinksync-gui: {}: {e}", path.display());
      }
    }
  }

  fn scan(&mut self) {
    self.scanning = true;
    self.last_scan = Some(Instant::now());
    self.worker.send(Command::Scan {
      kindle_folder: self.settings.kindle_folder.clone(),
    });
  }

  fn select(&mut self, found: Found) {
    self.notice = None;
    self.server = None;
    self.view = View::Loading { done: 0, total: 0 };
    self.worker.send(Command::Load {
      root: found.root.clone(),
      kind: found.kind,
    });
    self.selected = Some(found);
  }

  fn set_kindle_folder(&mut self, path: PathBuf) {
    match crate::devices::kindle_folder(&path) {
      Some(found) => {
        self.kindle_error = false;
        self.settings.kindle_folder = Some(path);
        self.save_settings();
        self.scan();
        self.select(found);
      }
      None => self.kindle_error = true,
    }
  }

  /// Forgets the Kindle copy: it leaves the list (and the view, if shown).
  fn clear_kindle_folder(&mut self) {
    let Some(old) = self.settings.kindle_folder.take() else {
      return;
    };
    self.save_settings();
    self.kindle_error = false;
    self.kindle_path.clear();
    self.devices.retain(|f| f.root != old);
    if self.selected.as_ref().is_some_and(|s| s.root == old) {
      self.selected = None;
      self.view = View::Nothing;
    }
    self.scan();
  }

  /// The remembered Kindle copy, with a button to forget it.
  fn kindle_folder_line(&mut self, ui: &mut egui::Ui, t: &Texts) {
    let Some(dir) = &self.settings.kindle_folder else {
      return;
    };
    ui.weak(format!("{}: {}", t.kindle_folder_path, dir.display()));
    if ui.button(t.remove_kindle_folder).clicked() {
      self.clear_kindle_folder();
    }
  }

  fn handle_events(&mut self) {
    while let Ok(event) = self.worker.events.try_recv() {
      let current = |root: &PathBuf| self.selected.as_ref().is_some_and(|s| &s.root == root);
      match event {
        Event::Devices(mut found) => {
          self.scanning = false;
          // A scan sent before the Kindle copy was removed still lists it.
          found.retain(|f| {
            f.kind == Kind::Nickel
              || self.settings.kindle_folder.as_ref() == Some(&f.root)
              || crate::devices::is_mounted_kindle(&f.root)
          });
          // A device that went away leaves the view.
          if let Some(selected) = &self.selected {
            if !found.iter().any(|f| f.root == selected.root) {
              self.selected = None;
              self.view = View::Nothing;
            }
          }
          self.devices = found;
        }
        Event::Progress { root, done, total } if current(&root) => {
          self.view = View::Loading { done, total };
        }
        Event::Loaded { root, result } if current(&root) => {
          self.view = match result {
            Ok(loaded) => View::Loaded(loaded),
            Err(e) => View::Failed(e),
          };
        }
        Event::Compared { root, state } if current(&root) => self.server = Some(state),
        Event::Synced { root, result } if current(&root) => {
          let lang = self.lang();
          self.busy = false;
          self.server = None;
          match result {
            Ok(mut report) => {
              self.notice = Some(Ok(i18n::report(lang, &report)));
              let questions = std::mem::take(&mut report.questions);
              if !questions.is_empty() {
                self.ask_dialog = Some(AskDialog {
                  questions: questions.into_iter().map(|q| (q, false)).collect(),
                });
              }
            }
            Err(e) => self.notice = Some(Err(e)),
          }
          if self.history_open {
            self.worker.send(Command::History);
          }
          if self.overview_open {
            self.refresh_overview();
          }
        }
        Event::History(entries) => self.history = Some(entries),
        Event::Overview(overview) => self.overview = Some(overview),
        Event::LibraryBooks(result) => {
          if let Some(dialog) = &mut self.copy_dialog {
            dialog.books = Some(result);
          }
        }
        Event::Copied { root, result } => {
          let lang = self.lang();
          self.copy_dialog = None;
          if current(&root) {
            self.notice = Some(result.map(|books| i18n::copied(lang, &books)));
          }
        }
        Event::Backups(result) => {
          if let Some(dialog) = &mut self.restore_dialog {
            dialog.backups = Some(result);
          }
        }
        Event::Restored { root, result } => {
          let lang = self.lang();
          match result {
            Ok(before) => {
              self.restore_dialog = None;
              if current(&root) {
                self.notice = Some(Ok(i18n::restored(lang, &before)));
              }
            }
            Err(e) => {
              if let Some(dialog) = &mut self.restore_dialog {
                dialog.busy = false;
                dialog.error = Some(e);
              }
            }
          }
        }
        Event::Settings(result) => {
          if let Some(dialog) = &mut self.settings_dialog {
            dialog.edit = Some(result);
          }
        }
        Event::SettingsSaved(result) => {
          let lang = self.lang();
          let t = i18n::texts(lang);
          match result {
            // The key moved: the dialog stays, with the new state.
            Ok(true) => {
              self.notice = Some(Ok(t.moved_to_keychain.into()));
              if let Some(dialog) = &mut self.settings_dialog {
                dialog.busy = false;
                dialog.edit = None;
                self.worker.send(Command::ReadSettings);
              }
            }
            Ok(false) => {
              self.settings_dialog = None;
              // The server or library may have changed.
              if let Some(selected) = self.selected.clone() {
                self.select(selected);
              }
              self.notice = Some(Ok(t.settings_saved.into()));
            }
            Err(e) => {
              if let Some(dialog) = &mut self.settings_dialog {
                dialog.busy = false;
                dialog.error = Some(e);
              }
            }
          }
        }
        // For a device no longer shown.
        Event::Progress { .. }
        | Event::Loaded { .. }
        | Event::Compared { .. }
        | Event::Synced { .. } => {}
        Event::Suggestions { id, result } => {
          if let Some(dialog) = self.map_dialog.as_mut().filter(|d| d.id == id) {
            dialog.chosen = match &result {
              Ok(s) if s.same.is_none() => s
                .candidates
                .first()
                .filter(|c| c.score >= suggest::LIKELY)
                .map(|c| c.path.clone()),
              _ => None,
            };
            dialog.candidates = match result {
              Ok(s) => Candidates::Ready(s),
              Err(e) => Candidates::Failed(e),
            };
          }
        }
        Event::Mapped { id, result } => {
          let lang = self.lang();
          if let Some(dialog) = self.map_dialog.as_mut().filter(|d| d.id == id) {
            match result {
              Ok(mapped) => {
                self.notice = Some(Ok(match mapped {
                  Mapped::SameFile => i18n::same_file_notice(lang, &dialog.title),
                  Mapped::To(path) => i18n::mapped_notice(lang, &dialog.title, &path),
                }));
                self.map_dialog = None;
              }
              Err(e) => {
                dialog.busy = false;
                dialog.error = Some(e);
              }
            }
          }
        }
        Event::Unmapped { result } => {
          let lang = self.lang();
          let title = self
            .unmap_dialog
            .take()
            .map(|d| d.title)
            .unwrap_or_default();
          self.notice = Some(result.map(|()| i18n::unmapped_notice(lang, &title)));
        }
      }
    }
  }

  fn top_bar(&mut self, ui: &mut egui::Ui, t: &Texts) {
    ui.horizontal(|ui| {
      ui.heading("NickelInkSync");
      ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let before = self.settings.language;
        egui::ComboBox::from_id_salt("language")
          .selected_text(match self.settings.language {
            Choice::Auto => t.language_auto,
            Choice::De => "Deutsch",
            Choice::En => "English",
          })
          .show_ui(ui, |ui| {
            ui.selectable_value(&mut self.settings.language, Choice::Auto, t.language_auto);
            ui.selectable_value(&mut self.settings.language, Choice::De, "Deutsch");
            ui.selectable_value(&mut self.settings.language, Choice::En, "English");
          });
        ui.label(t.language);
        if self.settings.language != before {
          self.save_settings();
        }
        if ui.button(t.settings).clicked() {
          self.settings_dialog = Some(SettingsDialog::new());
          self.worker.send(Command::ReadSettings);
        }
        if ui.button(t.history).clicked() {
          self.history_open = true;
          self.worker.send(Command::History);
        }
        if ui
          .add_enabled(!self.devices.is_empty(), egui::Button::new(t.overview))
          .clicked()
        {
          self.overview_open = true;
          self.refresh_overview();
        }
      });
    });
  }

  fn device_list(&mut self, ui: &mut egui::Ui, t: &Texts) {
    ui.horizontal(|ui| {
      ui.strong(t.devices);
      if ui.button(t.rescan).clicked() {
        self.scan();
      }
    });
    ui.add_space(4.0);
    let mut chosen = None;
    let nickel: Vec<&Found> = self
      .devices
      .iter()
      .filter(|f| f.kind == Kind::Nickel)
      .collect();
    if nickel.is_empty() {
      ui.weak(t.no_devices);
    }
    for found in self.devices.iter() {
      let selected = self.selected.as_ref().is_some_and(|s| s.root == found.root);
      let label = match found.kind {
        Kind::Nickel => found.name.clone(),
        Kind::Kindle => format!("{} ({})", t.kindle_folder, found.name),
      };
      if ui
        .selectable_label(selected, label)
        .on_hover_text(found.root.display().to_string())
        .clicked()
      {
        chosen = Some(found.clone());
      }
    }
    if let Some(found) = chosen {
      self.select(found);
    }
    ui.separator();
    self.kindle_picker(ui, t);
    ui.separator();
    ui.weak(t.no_auto_sync);
  }

  #[cfg(any(target_os = "macos", target_os = "windows"))]
  fn kindle_picker(&mut self, ui: &mut egui::Ui, t: &Texts) {
    if ui
      .button(t.choose_kindle_folder)
      .on_hover_text(t.kindle_folder_hint)
      .clicked()
    {
      let mut dialog = rfd::FileDialog::new();
      if let Some(dir) = &self.settings.kindle_folder {
        dialog = dialog.set_directory(dir);
      }
      if let Some(path) = dialog.pick_folder() {
        self.set_kindle_folder(path);
      }
    }
    self.kindle_folder_line(ui, t);
    if self.kindle_error {
      ui.colored_label(ui.visuals().error_fg_color, t.not_a_kindle_folder);
    }
  }

  /// Without a folder dialog (Linux): the path typed in.
  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  fn kindle_picker(&mut self, ui: &mut egui::Ui, t: &Texts) {
    ui.label(t.kindle_folder_path)
      .on_hover_text(t.kindle_folder_hint);
    let edit =
      ui.add(egui::TextEdit::singleline(&mut self.kindle_path).hint_text(t.choose_kindle_folder));
    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
      let path = PathBuf::from(self.kindle_path.trim());
      self.set_kindle_folder(path);
    }
    self.kindle_folder_line(ui, t);
    if self.kindle_error {
      ui.colored_label(ui.visuals().error_fg_color, t.not_a_kindle_folder);
    }
  }

  fn books(&mut self, ui: &mut egui::Ui, t: &Texts) {
    let lang = self.lang();
    match &self.notice {
      Some(Ok(notice)) => {
        ui.label(notice);
      }
      Some(Err(e)) => {
        ui.colored_label(ui.visuals().error_fg_color, e);
      }
      None => {}
    }
    let mut action = None;
    match &self.view {
      View::Nothing => {
        ui.weak(t.choose_device);
      }
      View::Loading { done, total } => {
        ui.horizontal(|ui| {
          ui.spinner();
          ui.label(t.loading);
          if *total > 0 {
            ui.weak(i18n::progress(lang, *done, *total));
          }
        });
      }
      View::Failed(e) => {
        ui.colored_label(ui.visuals().error_fg_color, format!("{}: {e}", t.failed));
      }
      View::Loaded(loaded) => {
        action = loaded_view(ui, t, lang, loaded, self.server.as_ref(), self.busy)
      }
    }
    match action {
      Some(Action::Map { id, title }) => self.open_map_dialog(id, title),
      Some(Action::Unmap { key, title }) => self.unmap_dialog = Some(UnmapDialog { key, title }),
      Some(Action::Push(ids)) => self.push(ids),
      Some(Action::Pull(ids)) => self.pull(ids),
      Some(Action::Restore) => {
        if let Some(selected) = &self.selected {
          self.restore_dialog = Some(RestoreDialog::new());
          self.worker.send(Command::Backups {
            root: selected.root.clone(),
          });
        }
      }
      Some(Action::AddFromLibrary { kobo }) => {
        self.copy_dialog = Some(CopyDialog::new(kobo, copy::kepubify().is_some()));
        self.worker.send(Command::LibraryBooks);
      }
      Some(Action::PushUnreferenced { id, title }) => self.unreferenced = Some((id, title)),
      None => {}
    }
  }

  fn push(&mut self, ids: Vec<String>) {
    let Some(selected) = &self.selected else {
      return;
    };
    self.busy = true;
    self.notice = None;
    self.worker.send(Command::Push {
      root: selected.root.clone(),
      kind: selected.kind,
      ids,
    });
  }

  fn pull(&mut self, ids: Vec<String>) {
    let Some(selected) = &self.selected else {
      return;
    };
    self.busy = true;
    self.notice = None;
    self.worker.send(Command::Pull {
      root: selected.root.clone(),
      ids,
    });
  }

  fn ask_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let Some(dialog) = &mut self.ask_dialog else {
      return;
    };
    let mut answer = None;
    egui::Modal::new(egui::Id::unique("ask")).show(ctx, |ui| {
      ui.set_max_width(640.0);
      ui.heading(t.ask_title);
      egui::ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
          for (i, (q, send)) in dialog.questions.iter_mut().enumerate() {
            ui.add_space(6.0);
            ui.strong(&q.title);
            let read = q
              .local_time
              .as_deref()
              .map(|time| format!(", {} {time}", t.last_read))
              .unwrap_or_default();
            ui.label(format!(
              "{}: {:.0} %{read}",
              t.device,
              q.local_percent * 100.0
            ))
            .on_hover_text(&q.local_xpointer);
            ui.label(format!(
              "{}: {:.0} % ({}, {})",
              t.server,
              q.remote.percent * 100.0,
              q.remote.device,
              q.remote.time
            ))
            .on_hover_text(&q.remote.xpointer);
            let pulls = q.pulls();
            ui.weak(match (q.backwards, pulls) {
              (false, _) => t.positions_differ,
              (true, false) => t.device_behind,
              (true, true) => t.server_behind,
            });
            // Unchanged comes first and is chosen until changed.
            let (keep, change) = if pulls {
              (t.keep_device, t.take_server)
            } else {
              (t.keep_server, t.send_device)
            };
            ui.horizontal(|ui| {
              ui.push_id(i, |ui| {
                ui.radio_value(send, false, keep);
                ui.radio_value(send, true, change);
              });
            });
          }
        });
      ui.add_space(6.0);
      ui.horizontal(|ui| {
        if ui.button(t.apply).clicked() {
          answer = Some(true);
        }
        if ui.button(t.cancel).clicked() {
          answer = Some(false);
        }
      });
    });
    match answer {
      Some(true) => {
        let questions: Vec<Question> = self
          .ask_dialog
          .take()
          .map(|d| d.questions)
          .unwrap_or_default()
          .into_iter()
          .filter(|(_, send)| *send)
          .map(|(q, _)| q)
          .collect();
        if let (Some(selected), false) = (&self.selected, questions.is_empty()) {
          self.busy = true;
          self.worker.send(Command::Answer {
            root: selected.root.clone(),
            kind: selected.kind,
            questions,
          });
        }
      }
      Some(false) => self.ask_dialog = None,
      None => {}
    }
  }

  fn unreferenced_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let lang = self.lang();
    let Some((id, title)) = &self.unreferenced else {
      return;
    };
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("unreferenced")).show(ctx, |ui| {
      ui.set_max_width(480.0);
      ui.label(i18n::unreferenced_question(lang, title));
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        if ui.button(t.send_anyway).clicked() {
          answer = Some(true);
        }
        if ui.button(t.cancel).clicked() {
          answer = Some(false);
        }
      });
    });
    match answer {
      Some(true) => {
        let id = id.clone();
        self.unreferenced = None;
        self.push(vec![id]);
      }
      Some(false) => self.unreferenced = None,
      None if modal.should_close() => self.unreferenced = None,
      None => {}
    }
  }

  fn copy_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let lang = self.lang();
    let Some(dialog) = &mut self.copy_dialog else {
      return;
    };
    match copy_dialog::show(ctx, t, lang, dialog) {
      Some(copy_dialog::Request::Copy { books, kepub }) => {
        if let Some(selected) = &self.selected {
          dialog.busy = true;
          self.worker.send(Command::Copy {
            root: selected.root.clone(),
            books,
            kepub,
          });
        }
      }
      Some(copy_dialog::Request::Close) => self.copy_dialog = None,
      None => {}
    }
  }

  fn restore_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let Some(dialog) = &mut self.restore_dialog else {
      return;
    };
    match restore_dialog::show(ctx, t, dialog) {
      Some(restore_dialog::Request::Restore(backup)) => {
        if let Some(selected) = &self.selected {
          dialog.busy = true;
          dialog.error = None;
          self.worker.send(Command::Restore {
            root: selected.root.clone(),
            backup,
          });
        }
      }
      Some(restore_dialog::Request::Close) => self.restore_dialog = None,
      None => {}
    }
  }

  fn settings_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let Some(dialog) = &mut self.settings_dialog else {
      return;
    };
    match settings_dialog::show(ctx, t, dialog) {
      Some(Request::Save { edit, password }) => {
        dialog.busy = true;
        dialog.error = None;
        self.worker.send(Command::SaveSettings { edit, password });
      }
      Some(Request::MoveToKeychain) => {
        dialog.busy = true;
        dialog.error = None;
        self.worker.send(Command::MoveToKeychain);
      }
      Some(Request::Close) => self.settings_dialog = None,
      None => {}
    }
  }

  fn refresh_overview(&mut self) {
    self.overview = None;
    self.worker.send(Command::Overview {
      devices: self.devices.clone(),
    });
  }

  fn overview_window(&mut self, ctx: &egui::Context, t: &Texts) {
    let mut open = self.overview_open;
    let mut refresh = false;
    egui::Window::new(t.overview)
      .open(&mut open)
      .default_size([820.0, 420.0])
      .show(ctx, |ui| {
        ui.horizontal(|ui| {
          ui.weak(t.overview_hint);
          if ui
            .add_enabled(self.overview.is_some(), egui::Button::new(t.refresh))
            .clicked()
          {
            refresh = true;
          }
        });
        match &self.overview {
          None => {
            ui.spinner();
          }
          Some(overview) => overview_grid(ui, t, overview),
        }
      });
    self.overview_open = open;
    if refresh {
      self.refresh_overview();
    }
  }

  fn history_window(&mut self, ctx: &egui::Context, t: &Texts) {
    let mut open = self.history_open;
    egui::Window::new(t.history)
      .open(&mut open)
      .default_size([720.0, 400.0])
      .show(ctx, |ui| match &self.history {
        None => {
          ui.spinner();
        }
        Some(Err(e)) => {
          ui.colored_label(ui.visuals().error_fg_color, e);
        }
        Some(Ok(entries)) if entries.is_empty() => {
          ui.weak(t.history_empty);
        }
        Some(Ok(entries)) => {
          egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("history")
              .striped(true)
              .num_columns(6)
              .spacing([16.0, 4.0])
              .show(ui, |ui| {
                for head in [t.time, t.direction, t.device, t.title, t.position, t.by] {
                  ui.strong(head);
                }
                ui.end_row();
                for e in entries {
                  ui.label(format_timestamp(e.time));
                  ui.label(match e.direction {
                    history::Direction::Push => t.to_server,
                    history::Direction::Pull => t.to_device,
                  });
                  ui.label(&e.device);
                  ui.label(e.title.as_deref().unwrap_or("–"))
                    .on_hover_text(&e.document);
                  ui.label(format!("{:.0} %", e.percentage * 100.0))
                    .on_hover_text(&e.position);
                  ui.label(&e.by);
                  ui.end_row();
                }
              });
          });
        }
      });
    self.history_open = open;
  }

  fn open_map_dialog(&mut self, id: String, title: String) {
    let Some(selected) = &self.selected else {
      return;
    };
    self.worker.send(Command::Suggest {
      root: selected.root.clone(),
      id: id.clone(),
    });
    self.map_dialog = Some(MapDialog {
      id,
      title,
      candidates: Candidates::Loading,
      chosen: None,
      busy: false,
      error: None,
      typed: String::new(),
    });
  }

  fn map_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let lang = self.lang();
    let Some(dialog) = &mut self.map_dialog else {
      return;
    };
    let mut close = false;
    let mut confirm = false;
    let modal = egui::Modal::new(egui::Id::unique("map")).show(ctx, |ui| {
      ui.set_max_width(640.0);
      ui.heading(format!("{} „{}“", t.map_title, dialog.title));
      ui.add_space(4.0);
      match &dialog.candidates {
        Candidates::Loading => {
          ui.horizontal(|ui| {
            ui.spinner();
            ui.label(t.comparing);
          });
        }
        Candidates::Failed(e) => {
          ui.colored_label(ui.visuals().error_fg_color, e);
        }
        Candidates::Ready(s) => {
          if s.library.is_none() {
            ui.weak(t.no_library_for_suggestions);
          } else if let Some(same) = &s.same {
            ui.label(t.same_in_library);
            ui.monospace(same.display().to_string());
          } else if s.candidates.is_empty() {
            ui.weak(t.no_candidates);
          }
          egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
              for (i, c) in s.candidates.iter().enumerate() {
                let title = c.title.as_deref().unwrap_or("?");
                let label = match c.authors.as_slice() {
                  [] => title.to_string(),
                  authors => format!("{title} — {}", authors.join(", ")),
                };
                let selected = dialog.chosen.as_ref() == Some(&c.path);
                if ui.radio(selected, label).clicked() {
                  dialog.chosen = Some(c.path.clone());
                }
                ui.indent(i, |ui| {
                  ui.weak(c.path.display().to_string());
                  let mut score =
                    i18n::score(lang, c.score, c.title_score, c.author_score, c.chapters);
                  if i == 0 && c.score < suggest::LIKELY {
                    score = format!("{score} · {}", t.uncertain);
                  }
                  ui.weak(score);
                });
              }
            });
        }
      }
      ui.separator();
      epub_picker(ui, t, dialog);
      if let Some(chosen) = &dialog.chosen {
        ui.label(format!("{}: {}", t.chosen, chosen.display()));
      }
      if let Some(e) = &dialog.error {
        ui.colored_label(
          ui.visuals().error_fg_color,
          format!("{}: {e}", t.map_failed),
        );
      }
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        let ready = dialog.chosen.is_some() && !dialog.busy;
        if ui
          .add_enabled(ready, egui::Button::new(t.map_confirm))
          .clicked()
        {
          confirm = true;
        }
        if ui.button(t.cancel).clicked() {
          close = true;
        }
        if dialog.busy {
          ui.spinner();
        }
      });
    });
    if confirm {
      if let (Some(selected), Some(epub)) = (&self.selected, dialog.chosen.clone()) {
        dialog.busy = true;
        dialog.error = None;
        self.worker.send(Command::Map {
          root: selected.root.clone(),
          kind: selected.kind,
          id: dialog.id.clone(),
          epub,
        });
      }
    }
    if close || (modal.should_close() && !dialog.busy) {
      self.map_dialog = None;
    }
  }

  fn unmap_dialog(&mut self, ctx: &egui::Context, t: &Texts) {
    let lang = self.lang();
    let Some(dialog) = &self.unmap_dialog else {
      return;
    };
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("unmap")).show(ctx, |ui| {
      ui.set_max_width(480.0);
      ui.label(i18n::unmap_question(lang, &dialog.title));
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        if ui.button(t.unmap_button).clicked() {
          answer = Some(true);
        }
        if ui.button(t.cancel).clicked() {
          answer = Some(false);
        }
      });
    });
    match answer {
      Some(true) => {
        if let Some(selected) = &self.selected {
          // The dialog stays until the answer, for the notice's title.
          self.worker.send(Command::Unmap {
            root: selected.root.clone(),
            kind: selected.kind,
            key: dialog.key.clone(),
          });
        }
      }
      Some(false) => self.unmap_dialog = None,
      None if modal.should_close() => self.unmap_dialog = None,
      None => {}
    }
  }
}

/// Choosing the original EPUB by hand: a file dialog, starting in the
/// library.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn epub_picker(ui: &mut egui::Ui, t: &Texts, dialog: &mut MapDialog) {
  if ui
    .add_enabled(!dialog.busy, egui::Button::new(t.choose_epub))
    .clicked()
  {
    let mut picker = rfd::FileDialog::new().add_filter("EPUB", &["epub"]);
    if let Candidates::Ready(Suggestions {
      library: Some(dir), ..
    }) = &dialog.candidates
    {
      picker = picker.set_directory(dir);
    }
    if let Some(path) = picker.pick_file() {
      dialog.chosen = Some(path);
    }
  }
}

/// Without a file dialog (Linux): the path typed in.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn epub_picker(ui: &mut egui::Ui, t: &Texts, dialog: &mut MapDialog) {
  ui.label(t.epub_path);
  let edit = ui.add(egui::TextEdit::singleline(&mut dialog.typed).hint_text(t.choose_epub));
  if edit.changed() {
    let typed = dialog.typed.trim();
    dialog.chosen = (!typed.is_empty()).then(|| PathBuf::from(typed));
  }
}

fn loaded_view(
  ui: &mut egui::Ui,
  t: &Texts,
  lang: Lang,
  loaded: &Loaded,
  server: Option<&ServerState>,
  busy: bool,
) -> Option<Action> {
  ui.heading(&loaded.device);
  match &loaded.library {
    Some((path, kind, n)) => {
      let kind = match kind {
        library::Kind::Calibre => "Calibre",
        library::Kind::Folder => "",
      };
      ui.label(format!(
        "{}: {} {kind} ({})",
        t.library,
        path.display(),
        i18n::epubs(lang, *n)
      ));
    }
    None => {
      ui.colored_label(ui.visuals().warn_fg_color, t.no_library);
    }
  }
  ui.label(format!("{}: {}", t.mappings, loaded.mappings));
  ui.weak(match loaded.family {
    // Nothing is written with Kobalt installed.
    Some(_) if loaded.writable => t.writes_tolino,
    Some(_) => t.kobo_read_only,
    None => t.read_only,
  });
  if loaded.kobalt {
    ui.colored_label(ui.visuals().warn_fg_color, t.kobalt);
  }
  let mut action = None;
  if let Some(family) = loaded.family {
    ui.horizontal(|ui| {
      if ui
        .button(t.add_from_library)
        .on_hover_text(t.add_from_library_hint)
        .clicked()
      {
        action = Some(Action::AddFromLibrary {
          kobo: family == "Kobo",
        });
      }
      // Kobalt manages the positions; nothing goes back then either.
      if !loaded.kobalt
        && ui
          .button(t.restore_open)
          .on_hover_text(t.restore_hint)
          .clicked()
      {
        action = Some(Action::Restore);
      }
    });
  }
  let compared = match server {
    Some(ServerState::Compared(compared)) => Some(compared),
    _ => None,
  };
  ui.horizontal(|ui| match server {
    None => {
      ui.spinner();
      ui.label(t.server_comparing);
    }
    Some(ServerState::NotConfigured) => {
      ui.colored_label(ui.visuals().warn_fg_color, t.server_not_configured);
    }
    Some(ServerState::Failed(e)) => {
      ui.colored_label(
        ui.visuals().error_fg_color,
        format!("{}: {e}", t.server_failed),
      );
    }
    Some(ServerState::Compared(compared)) => {
      // Books without a reference EPUB are sent one by one, after a yes;
      // books already in sync are left out.
      let ids: Vec<String> = loaded
        .rows
        .iter()
        .filter(|r| matches!(r.group, Group::Mapped { .. } | Group::Same { .. }))
        .filter(|r| !in_sync(compared.get(&r.id)))
        .map(|r| r.id.clone())
        .collect();
      let enabled = !busy && !ids.is_empty();
      if ui
        .add_enabled(enabled, egui::Button::new(t.send_all))
        .clicked()
      {
        action = Some(Action::Push(ids.clone()));
      }
      // Not into a kepub yet (K4).
      let pull_ids: Vec<String> = ids
        .into_iter()
        .filter(|id| !loaded.rows.iter().any(|r| &r.id == id && is_kepub(r)))
        .collect();
      if loaded.writable
        && ui
          .add_enabled(!busy && !pull_ids.is_empty(), egui::Button::new(t.pull_all))
          .clicked()
      {
        action = Some(Action::Pull(pull_ids));
      }
      if busy {
        ui.spinner();
      }
    }
  });
  ui.add_space(8.0);
  if loaded.rows.is_empty() {
    ui.label(t.no_books);
    return action;
  }
  let pick =
    |f: fn(&Group) -> bool| -> Vec<&Row> { loaded.rows.iter().filter(|r| f(&r.group)).collect() };
  // Name, hint, books, and whether it is a warning: shown only with books,
  // its hint in the warning colour.
  let sections = [
    (
      t.not_mapped,
      t.not_mapped_hint,
      pick(|g| matches!(g, Group::NotMapped)),
      true,
    ),
    (
      t.no_reference,
      t.no_reference_hint,
      pick(|g| matches!(g, Group::NoReference)),
      false,
    ),
    (
      t.mapped,
      t.mapped_hint,
      pick(|g| matches!(g, Group::Mapped { .. })),
      true,
    ),
    (
      t.own_checksum,
      t.own_checksum_hint,
      pick(|g| matches!(g, Group::Same { .. })),
      true,
    ),
    (
      t.problems,
      "",
      pick(|g| matches!(g, Group::Problem { .. })),
      false,
    ),
  ];
  let can_send = compared.is_some() && !busy;
  egui::ScrollArea::both().show(ui, |ui| {
    for (i, (name, hint, rows, shown_empty)) in sections.iter().enumerate() {
      let warning = !shown_empty;
      if rows.is_empty() && warning {
        continue;
      }
      egui::CollapsingHeader::new(format!("{name} ({})", rows.len()))
        .id_salt(i)
        .default_open(!rows.is_empty())
        .show(ui, |ui| {
          if warning && !hint.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, *hint);
          } else if !hint.is_empty() {
            ui.weak(*hint);
          }
          if rows.is_empty() {
            ui.weak(t.none);
            return;
          }
          if let Some(a) = book_table(ui, t, i, rows, compared, can_send, loaded.writable) {
            action = Some(a);
          }
        });
    }
  });
  action
}

/// One line per book, one column per device, then the server.
fn overview_grid(ui: &mut egui::Ui, t: &Texts, overview: &Overview) {
  match &overview.server {
    Ok(true) => {}
    Ok(false) => {
      ui.colored_label(ui.visuals().warn_fg_color, t.server_not_configured);
    }
    Err(e) => {
      ui.colored_label(
        ui.visuals().error_fg_color,
        format!("{}: {e}", t.server_failed),
      );
    }
  }
  for column in &overview.columns {
    if let Some(e) = &column.error {
      ui.colored_label(ui.visuals().error_fg_color, format!("{}: {e}", column.name));
    }
  }
  if overview.lines.is_empty() {
    ui.weak(t.no_books);
    return;
  }
  egui::ScrollArea::both().show(ui, |ui| {
    egui::Grid::new("overview")
      .striped(true)
      .num_columns(overview.columns.len() + 2)
      .spacing([16.0, 4.0])
      .show(ui, |ui| {
        ui.strong(t.title);
        for column in &overview.columns {
          ui.strong(&column.name);
        }
        ui.strong(t.server);
        ui.end_row();
        for line in &overview.lines {
          let title = match &line.author {
            Some(author) => format!("{} — {author}", line.title),
            None => line.title.clone(),
          };
          ui.label(title).on_hover_text(&line.document);
          for cell in &line.cells {
            match cell {
              None => {
                ui.weak("–");
              }
              Some(cell) => {
                let text = match cell.percent {
                  Some(p) => format!("{:.0} %", p * 100.0),
                  None => t.never_opened.to_string(),
                };
                let hover = [cell.last_read.as_deref(), cell.xpointer.as_deref()]
                  .into_iter()
                  .flatten()
                  .collect::<Vec<_>>()
                  .join("\n");
                let response = ui.label(text);
                if !hover.is_empty() {
                  response.on_hover_text(hover);
                }
              }
            }
          }
          match &line.server {
            Some(r) => {
              ui.label(format!("{:.0} % · {}", r.percent * 100.0, r.device))
                .on_hover_text(format!("{}\n{}", r.time, r.xpointer));
            }
            None if overview.server == Ok(true) => {
              ui.weak(t.nothing_stored);
            }
            None => {
              ui.weak("–");
            }
          }
          ui.end_row();
        }
      });
  });
}

/// The file name of a path, for a narrow column; the full path goes in
/// the hover text.
fn file_name(path: &str) -> &str {
  path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// A kepub on a Kobo: positions are not translated back into it yet (K4).
fn is_kepub(row: &Row) -> bool {
  row.file.ends_with(".kepub.epub")
}

/// Device and server hold the same position.
fn in_sync(compared: Option<&Compared>) -> bool {
  compared.is_some_and(|c| c.outlook == Some(Outlook::InSync))
}

/// The server's side of a book: "57 % · KOReader · server newer".
fn server_cell(t: &Texts, compared: Option<&Compared>) -> (String, Option<String>) {
  let Some(c) = compared else {
    return ("–".into(), None);
  };
  let outlook = c.outlook.map(|o| match o {
    Outlook::InSync => t.outlook_same,
    Outlook::Push => t.outlook_device,
    Outlook::Pull => t.outlook_server,
    Outlook::Asks { .. } => t.outlook_asks,
  });
  let (text, hover) = match &c.remote {
    Some(r) => (
      format!("{:.0} % · {}", r.percent * 100.0, r.device),
      Some(format!("{}\n{}", r.time, r.xpointer)),
    ),
    None => (t.nothing_stored.to_string(), None),
  };
  match outlook {
    Some(o) => (format!("{text} · {o}"), hover),
    None => (text, hover),
  }
}

fn book_table(
  ui: &mut egui::Ui,
  t: &Texts,
  id: usize,
  rows: &[&Row],
  compared: Option<&HashMap<String, Compared>>,
  can_send: bool,
  writable: bool,
) -> Option<Action> {
  let mut action = None;
  egui::Grid::new(("books", id))
    .striped(true)
    .num_columns(6)
    .spacing([16.0, 4.0])
    .show(ui, |ui| {
      ui.strong(t.title);
      ui.label("");
      ui.strong(t.position);
      ui.strong(t.server);
      ui.strong(t.last_read);
      ui.strong(match rows[0].group {
        Group::Mapped { .. } => t.original,
        Group::NotMapped | Group::NoReference => t.file,
        _ => t.why,
      });
      ui.end_row();
      for row in rows {
        let title = match &row.author {
          Some(author) => format!("{} — {author}", row.title),
          None => row.title.clone(),
        };
        ui.label(title).on_hover_text(&row.file);
        // The buttons right after the title, so a long path never pushes
        // them out of sight.
        ui.horizontal(|ui| {
          let can_map = !row.id.is_empty();
          let map_label = match row.group {
            // Without a library nothing told whether it needs a mapping.
            Group::NotMapped | Group::NoReference => Some(t.map_button),
            Group::Mapped { .. } | Group::Problem { .. } if row.key.is_some() => {
              Some(t.remap_button)
            }
            _ => None,
          };
          if let Some(label) = map_label.filter(|_| can_map) {
            if ui.small_button(label).clicked() {
              action = Some(Action::Map {
                id: row.id.clone(),
                title: row.title.clone(),
              });
            }
          }
          if let Some(key) = &row.key {
            if ui.small_button(t.unmap_button).clicked() {
              action = Some(Action::Unmap {
                key: key.clone(),
                title: row.title.clone(),
              });
            }
          }
          let sendable = matches!(
            row.group,
            Group::Mapped { .. } | Group::Same { .. } | Group::NoReference
          );
          // Nothing to send or get once both sides agree.
          let can_send = can_send && !in_sync(compared.and_then(|c| c.get(&row.id)));
          if sendable && compared.is_some() {
            let send = ui.add_enabled(can_send, egui::Button::new(t.send_button).small());
            if send.clicked() {
              action = Some(match row.group {
                Group::NoReference => Action::PushUnreferenced {
                  id: row.id.clone(),
                  title: row.title.clone(),
                },
                _ => Action::Push(vec![row.id.clone()]),
              });
            }
          }
          // Like "Get all", not for a book without a reference EPUB.
          let pullable =
            matches!(row.group, Group::Mapped { .. } | Group::Same { .. }) && !is_kepub(row);
          if writable && pullable && compared.is_some() {
            let pull = ui.add_enabled(can_send, egui::Button::new(t.pull_button).small());
            if pull.clicked() {
              action = Some(Action::Pull(vec![row.id.clone()]));
            }
          }
        });
        let position = match row.percent {
          Some(p) => format!("{:.0} %", p * 100.0),
          None => t.never_opened.to_string(),
        };
        let response = ui.label(position);
        match &row.xpointer {
          Ok(x) => {
            response.on_hover_text(x);
          }
          Err(e) if !e.is_empty() => {
            response.on_hover_text(e);
          }
          Err(_) => {}
        }
        let (text, hover) = server_cell(t, compared.and_then(|c| c.get(&row.id)));
        let response = ui.label(text);
        if let Some(hover) = hover {
          response.on_hover_text(hover);
        }
        ui.label(row.last_read.as_deref().unwrap_or("–"));
        match &row.group {
          Group::Mapped { original } => ui.label(file_name(original)).on_hover_text(original),
          Group::NotMapped | Group::NoReference => {
            ui.label(file_name(&row.file)).on_hover_text(&row.file)
          }
          Group::Same { why } => ui.weak(match why {
            Own::InLibrary => t.own_in_library,
            Own::OriginalOfMapping => t.own_original,
            Own::NoLibrary => t.no_reference,
          }),
          Group::Problem { message } => ui.colored_label(ui.visuals().warn_fg_color, message),
        };
        ui.end_row();
      }
    });
  action
}

impl eframe::App for App {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    self.handle_events();
    if !self.scanning && self.last_scan.is_none_or(|t| t.elapsed() >= SCAN_EVERY) {
      self.scan();
    }
    ui.ctx().request_repaint_after(SCAN_EVERY);
    let t = i18n::texts(self.lang());
    egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, t));
    egui::Panel::left("devices")
      .resizable(true)
      .default_size(220.0)
      .show(ui, |ui| self.device_list(ui, t));
    egui::CentralPanel::default().show(ui, |ui| self.books(ui, t));
    let ctx = ui.ctx().clone();
    self.map_dialog(&ctx, t);
    self.unmap_dialog(&ctx, t);
    self.ask_dialog(&ctx, t);
    self.unreferenced_dialog(&ctx, t);
    self.history_window(&ctx, t);
    self.overview_window(&ctx, t);
    self.settings_dialog(&ctx, t);
    self.copy_dialog(&ctx, t);
    self.restore_dialog(&ctx, t);
  }
}

#[cfg(test)]
mod tests {
  use super::file_name;

  #[test]
  fn file_name_of_device_and_library_paths() {
    assert_eq!(
      file_name("Books/Acts of Contrition.kepub.epub"),
      "Acts of Contrition.kepub.epub"
    );
    assert_eq!(
      file_name("/Users/me/Calibre Library/A/B (1)/B.epub"),
      "B.epub"
    );
    assert_eq!(file_name("C:\\Books\\b.epub"), "b.epub");
    assert_eq!(file_name("b.epub"), "b.epub");
  }
}
