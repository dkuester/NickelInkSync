//! The settings dialog: the server account and the sync settings of
//! `config.ini`, shared with the CLI. A typed password goes to the worker
//! once, which keeps only its key (in the keychain where there is one); the
//! dialog never shows a stored password or key.

use eframe::egui;
use nickelinksync::config::{Editable, PushPosition, Secret};
use nickelinksync::credentials;
use nickelinksync::sync::Confirm;

use crate::i18n::Texts;

pub struct SettingsDialog {
  /// The file's settings once read, or why they could not be.
  pub edit: Option<Result<Editable, String>>,
  /// Typed here only to be sent with "Save"; empty keeps the stored key.
  pub password: String,
  /// Saving or moving was sent and not answered yet.
  pub busy: bool,
  pub error: Option<String>,
}

/// What the dialog asks for.
pub enum Request {
  Save {
    edit: Editable,
    password: Option<String>,
  },
  MoveToKeychain,
  Close,
}

impl SettingsDialog {
  pub fn new() -> Self {
    SettingsDialog {
      edit: None,
      password: String::new(),
      busy: false,
      error: None,
    }
  }
}

pub fn show(ctx: &egui::Context, t: &Texts, dialog: &mut SettingsDialog) -> Option<Request> {
  let mut request = None;
  let modal = egui::Modal::new(egui::Id::unique("settings")).show(ctx, |ui| {
    ui.set_max_width(560.0);
    ui.heading(t.settings);
    ui.weak(t.settings_hint);
    ui.add_space(6.0);
    match &mut dialog.edit {
      None => {
        ui.spinner();
      }
      Some(Err(e)) => {
        ui.colored_label(ui.visuals().error_fg_color, e.as_str());
      }
      Some(Ok(edit)) => {
        if let Some(r) = form(ui, t, edit, &mut dialog.password, dialog.busy) {
          request = Some(r);
        }
      }
    }
    if let Some(e) = &dialog.error {
      ui.colored_label(ui.visuals().error_fg_color, e);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
      let ready = matches!(dialog.edit, Some(Ok(_))) && !dialog.busy;
      if ui.add_enabled(ready, egui::Button::new(t.save)).clicked() {
        if let Some(Ok(edit)) = &dialog.edit {
          let password = std::mem::take(&mut dialog.password);
          request = Some(Request::Save {
            edit: edit.clone(),
            password: Some(password).filter(|p| !p.is_empty()),
          });
        }
      }
      if ui.button(t.cancel).clicked() {
        request = Some(Request::Close);
      }
      if dialog.busy {
        ui.spinner();
      }
    });
  });
  if request.is_none() && modal.should_close() && !dialog.busy {
    request = Some(Request::Close);
  }
  request
}

fn form(
  ui: &mut egui::Ui,
  t: &Texts,
  edit: &mut Editable,
  password: &mut String,
  busy: bool,
) -> Option<Request> {
  let mut request = None;
  egui::Grid::new("settings")
    .num_columns(2)
    .spacing([12.0, 6.0])
    .show(ui, |ui| {
      ui.label(t.server);
      ui.add(egui::TextEdit::singleline(&mut edit.server).hint_text("https://kosync.eu"));
      ui.end_row();
      ui.label(t.user);
      ui.text_edit_singleline(&mut edit.user);
      ui.end_row();
      ui.label(t.password);
      ui.vertical(|ui| {
        let hint = match edit.secret {
          Secret::InKeychain => t.password_in_keychain,
          Secret::InFile => t.password_in_file,
          Secret::Missing => "",
        };
        ui.add(
          egui::TextEdit::singleline(password)
            .password(true)
            .hint_text(hint),
        );
        match edit.secret {
          Secret::InFile if credentials::available() => {
            ui.horizontal_wrapped(|ui| {
              ui.colored_label(ui.visuals().warn_fg_color, t.password_file_warning);
              if ui
                .add_enabled(!busy, egui::Button::new(t.move_to_keychain).small())
                .clicked()
              {
                request = Some(Request::MoveToKeychain);
              }
            });
          }
          Secret::Missing => {
            ui.colored_label(ui.visuals().warn_fg_color, t.password_missing);
          }
          _ => {}
        }
        if !credentials::available() {
          ui.weak(t.no_keychain);
        }
      });
      ui.end_row();
      ui.label(t.device_name);
      ui.add(egui::TextEdit::singleline(&mut edit.device).hint_text(t.device_name_hint));
      ui.end_row();
      ui.label(t.confirm_label);
      egui::ComboBox::from_id_salt("confirm")
        .selected_text(confirm_text(t, edit.confirm))
        .show_ui(ui, |ui| {
          for c in [Confirm::Never, Confirm::Backwards, Confirm::Always] {
            ui.selectable_value(&mut edit.confirm, c, confirm_text(t, c));
          }
        });
      ui.end_row();
      ui.label(t.position_label);
      egui::ComboBox::from_id_salt("position")
        .selected_text(position_text(t, edit.position))
        .show_ui(ui, |ui| {
          for p in [
            PushPosition::Paragraph,
            PushPosition::Sentence,
            PushPosition::Exact,
          ] {
            ui.selectable_value(&mut edit.position, p, position_text(t, p));
          }
        });
      ui.end_row();
      ui.label(t.library);
      ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut edit.library).hint_text("~/Calibre-Bibliothek"));
        library_picker(ui, t, edit);
      });
      ui.end_row();
    });
  request
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn library_picker(ui: &mut egui::Ui, t: &Texts, edit: &mut Editable) {
  if ui.button(t.choose).clicked() {
    if let Some(path) = rfd::FileDialog::new().pick_folder() {
      edit.library = path.display().to_string();
    }
  }
}

/// Without a folder dialog (Linux) the path is typed in.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn library_picker(_: &mut egui::Ui, _: &Texts, _: &mut Editable) {}

fn confirm_text(t: &Texts, c: Confirm) -> &'static str {
  match c {
    Confirm::Never => t.confirm_never,
    Confirm::Backwards => t.confirm_backwards,
    Confirm::Always => t.confirm_always,
  }
}

fn position_text(t: &Texts, p: PushPosition) -> &'static str {
  match p {
    PushPosition::Exact => t.position_exact,
    PushPosition::Sentence => t.position_sentence,
    PushPosition::Paragraph => t.position_paragraph,
  }
}
