//! NickelInkSync desktop client: the devices, their books and how each
//! syncs, in a window. Built on the `nickelinksync` library like the CLI,
//! sharing its configuration and mappings.

mod app;
mod devices;
mod i18n;
mod model;
mod overview;
mod settings;
mod sync;
mod worker;

/// The CLI tests' fake devices and KOSync mock server.
#[cfg(test)]
#[path = "../../tests/common/mod.rs"]
mod common;

fn main() -> eframe::Result {
  let options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
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
