//! Choosing a backup of the device database to put back. The list holds
//! this device's backups and those made before backups named their device;
//! the state before goes into a new backup first.

use std::path::PathBuf;

use eframe::egui;
use nickelinksync::restore::{Backup, Origin};
use nickelinksync::sync::format_timestamp;

use crate::i18n::Texts;

pub struct RestoreDialog {
  pub backups: Option<Result<Vec<Backup>, String>>,
  pub chosen: Option<PathBuf>,
  pub busy: bool,
  pub error: Option<String>,
}

pub enum Request {
  Restore(PathBuf),
  Close,
}

impl RestoreDialog {
  pub fn new() -> Self {
    RestoreDialog {
      backups: None,
      chosen: None,
      busy: false,
      error: None,
    }
  }
}

pub fn show(ctx: &egui::Context, t: &Texts, dialog: &mut RestoreDialog) -> Option<Request> {
  let mut request = None;
  let modal = egui::Modal::new(egui::Id::unique("restore")).show(ctx, |ui| {
    ui.set_max_width(640.0);
    ui.heading(t.restore_title);
    ui.colored_label(ui.visuals().warn_fg_color, t.restore_warning);
    ui.add_space(6.0);
    match &dialog.backups {
      None => {
        ui.spinner();
      }
      Some(Err(e)) => {
        ui.colored_label(ui.visuals().error_fg_color, e.as_str());
      }
      Some(Ok(backups)) if backups.is_empty() => {
        ui.weak(t.no_backups);
      }
      Some(Ok(backups)) => {
        egui::ScrollArea::vertical()
          .max_height(320.0)
          .show(ui, |ui| {
            for b in backups {
              let name = b
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
              let mut text = format!("{}  {name}", format_timestamp(b.time));
              if b.origin == Origin::Unknown {
                text.push_str(&format!("  ({})", t.backup_device_unknown));
              }
              let chosen = dialog.chosen.as_ref() == Some(&b.path);
              if ui
                .radio(chosen, text)
                .on_hover_text(b.path.display().to_string())
                .clicked()
              {
                dialog.chosen = Some(b.path.clone());
              }
            }
          });
      }
    }
    if let Some(e) = &dialog.error {
      ui.colored_label(ui.visuals().error_fg_color, e);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
      let ready = dialog.chosen.is_some() && !dialog.busy;
      if ui
        .add_enabled(ready, egui::Button::new(t.restore_button))
        .clicked()
      {
        if let Some(path) = dialog.chosen.clone() {
          request = Some(Request::Restore(path));
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
