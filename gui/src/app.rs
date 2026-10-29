//! The window: devices on the left, the chosen device's books in the
//! middle, grouped by how they sync.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use nickelinksync::library;

use crate::devices::{Found, Kind};
use crate::i18n::{self, Choice, Lang, Texts};
use crate::model::{Group, Loaded, Row};
use crate::settings::Settings;
use crate::worker::{Command, Event, Worker};

/// How often the drives are searched again.
const SCAN_EVERY: Duration = Duration::from_secs(2);

enum View {
  Nothing,
  Loading { done: usize, total: usize },
  Failed(String),
  Loaded(Loaded),
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
  #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
  kindle_path: String,
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

  fn handle_events(&mut self) {
    while let Ok(event) = self.worker.events.try_recv() {
      let current = |root: &PathBuf| self.selected.as_ref().is_some_and(|s| &s.root == root);
      match event {
        Event::Devices(found) => {
          self.scanning = false;
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
        // For a device no longer shown.
        Event::Progress { .. } | Event::Loaded { .. } => {}
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
    if let Some(dir) = &self.settings.kindle_folder {
      ui.weak(format!("{}: {}", t.kindle_folder_path, dir.display()));
    }
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
    if self.kindle_error {
      ui.colored_label(ui.visuals().error_fg_color, t.not_a_kindle_folder);
    }
  }

  fn books(&self, ui: &mut egui::Ui, t: &Texts) {
    let lang = self.lang();
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
      View::Loaded(loaded) => loaded_view(ui, t, lang, loaded),
    }
  }
}

fn loaded_view(ui: &mut egui::Ui, t: &Texts, lang: Lang, loaded: &Loaded) {
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
    None if loaded.device == "Kindle" => {}
    None => {
      ui.weak(t.no_library);
    }
  }
  ui.label(format!("{}: {}", t.mappings, loaded.mappings));
  ui.weak(t.read_only);
  ui.add_space(8.0);
  if loaded.rows.is_empty() {
    ui.label(t.no_books);
    return;
  }
  let pick =
    |f: fn(&Group) -> bool| -> Vec<&Row> { loaded.rows.iter().filter(|r| f(&r.group)).collect() };
  let sections = [
    (
      t.not_mapped,
      t.not_mapped_hint,
      pick(|g| matches!(g, Group::NotMapped)),
    ),
    (
      t.mapped,
      t.mapped_hint,
      pick(|g| matches!(g, Group::Mapped { .. })),
    ),
    (
      t.same_file,
      t.same_file_hint,
      pick(|g| matches!(g, Group::Same { .. })),
    ),
    (t.problems, "", pick(|g| matches!(g, Group::Problem { .. }))),
  ];
  egui::ScrollArea::vertical().show(ui, |ui| {
    for (i, (name, hint, rows)) in sections.iter().enumerate() {
      if rows.is_empty() && i == 3 {
        continue;
      }
      egui::CollapsingHeader::new(format!("{name} ({})", rows.len()))
        .id_salt(i)
        .default_open(!rows.is_empty())
        .show(ui, |ui| {
          if !hint.is_empty() {
            ui.weak(*hint);
          }
          if rows.is_empty() {
            ui.weak(t.none);
            return;
          }
          book_table(ui, t, i, rows);
        });
    }
  });
}

fn book_table(ui: &mut egui::Ui, t: &Texts, id: usize, rows: &[&Row]) {
  egui::Grid::new(("books", id))
    .striped(true)
    .num_columns(4)
    .spacing([16.0, 4.0])
    .show(ui, |ui| {
      ui.strong(t.title);
      ui.strong(t.position);
      ui.strong(t.last_read);
      ui.strong(match rows[0].group {
        Group::Mapped { .. } => t.original,
        Group::NotMapped => t.file,
        _ => t.why,
      });
      ui.end_row();
      for row in rows {
        let title = match &row.author {
          Some(author) => format!("{} — {author}", row.title),
          None => row.title.clone(),
        };
        ui.label(title).on_hover_text(&row.file);
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
        ui.label(row.last_read.as_deref().unwrap_or("–"));
        match &row.group {
          Group::Mapped { original } => ui.label(original),
          Group::NotMapped => ui.label(&row.file),
          Group::Same { why } => ui.weak(why),
          Group::Problem { message } => ui.colored_label(ui.visuals().warn_fg_color, message),
        };
        ui.end_row();
      }
    });
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
  }
}
