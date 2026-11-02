//! NickelInkSync desktop client: the devices, their books and how each
//! syncs, in a window. Built on the `nickelinksync` library like the CLI,
//! sharing its configuration and mappings.

// No console window next to the app on Windows (release builds only, so a
// debug build still prints its panics).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod copy_dialog;
mod devices;
mod i18n;
mod model;
mod overview;
mod release;
mod restore_dialog;
mod settings;
mod settings_dialog;
mod sync;
mod worker;

/// The CLI tests' fake devices and KOSync mock server.
#[cfg(test)]
#[path = "../../tests/common/mod.rs"]
mod common;
#[cfg(test)]
mod test_data;

/// The window's icon, rendered from `packaging/icon.svg` (`rsvg-convert -w
/// 256 -h 256 packaging/icon.svg -o gui/assets/icon.png`). Without it eframe
/// shows egui's own icon while the app runs, also in the macOS Dock.
const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result {
  let mut viewport = eframe::egui::ViewportBuilder::default();
  if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON) {
    viewport = viewport.with_icon(icon);
  }
  let options = eframe::NativeOptions {
    viewport: viewport
      .with_title("NickelInkSync")
      .with_inner_size([960.0, 640.0])
      .with_min_inner_size([640.0, 400.0]),
    ..Default::default()
  };
  eframe::run_native(
    "NickelInkSync",
    options,
    Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
  )
}
