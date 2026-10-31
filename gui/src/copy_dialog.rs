//! Choosing books of the library to copy onto a Kobo or Tolino; on a Kobo
//! optionally as kepubs made by kepubify, mapped to their EPUB.

use std::collections::HashSet;
use std::path::PathBuf;

use eframe::egui;
use nickelinksync::library::LibraryBook;

use crate::i18n::{self, Lang, Texts};

/// How many matches are listed at once.
const SHOWN: usize = 300;

pub struct CopyDialog {
  /// The library's books once read, or why they could not be.
  pub books: Option<Result<Vec<LibraryBook>, String>>,
  pub search: String,
  pub chosen: HashSet<PathBuf>,
  /// As kepubs (Kobo with kepubify).
  pub kepub: bool,
  /// The device is a Kobo.
  pub kobo: bool,
  /// kepubify was found.
  pub kepubify: bool,
  pub busy: bool,
}

pub enum Request {
  Copy { books: Vec<PathBuf>, kepub: bool },
  Close,
}

impl CopyDialog {
  pub fn new(kobo: bool, kepubify: bool) -> Self {
    CopyDialog {
      books: None,
      search: String::new(),
      chosen: HashSet::new(),
      kepub: kobo && kepubify,
      kobo,
      kepubify,
      busy: false,
    }
  }
}

fn label(book: &LibraryBook) -> String {
  let title = book.title.clone().unwrap_or_else(|| {
    book
      .path
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_default()
  });
  match book.authors.is_empty() {
    true => title,
    false => format!("{title} — {}", book.authors.join(", ")),
  }
}

pub fn show(
  ctx: &egui::Context,
  t: &Texts,
  lang: Lang,
  dialog: &mut CopyDialog,
) -> Option<Request> {
  let mut request = None;
  let modal = egui::Modal::new(egui::Id::unique("copy")).show(ctx, |ui| {
    ui.set_max_width(640.0);
    ui.heading(t.copy_title);
    ui.weak(t.copy_hint);
    ui.add_space(6.0);
    match &dialog.books {
      None => {
        ui.spinner();
      }
      Some(Err(e)) => {
        ui.colored_label(ui.visuals().error_fg_color, e.as_str());
      }
      Some(Ok(books)) => {
        ui.add(egui::TextEdit::singleline(&mut dialog.search).hint_text(t.copy_search));
        let words: Vec<String> = dialog
          .search
          .to_lowercase()
          .split_whitespace()
          .map(str::to_string)
          .collect();
        let matches: Vec<&LibraryBook> = books
          .iter()
          .filter(|b| {
            let text = format!("{} {}", label(b), b.path.display()).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
          })
          .collect();
        egui::ScrollArea::vertical()
          .max_height(360.0)
          .show(ui, |ui| {
            for book in matches.iter().take(SHOWN) {
              let mut on = dialog.chosen.contains(&book.path);
              if ui
                .checkbox(&mut on, label(book))
                .on_hover_text(book.path.display().to_string())
                .changed()
              {
                if on {
                  dialog.chosen.insert(book.path.clone());
                } else {
                  dialog.chosen.remove(&book.path);
                }
              }
            }
          });
        if matches.len() > SHOWN {
          ui.weak(i18n::more_matches(lang, matches.len() - SHOWN));
        }
      }
    }
    if dialog.kobo {
      ui.add_space(4.0);
      ui.add_enabled(
        dialog.kepubify,
        egui::Checkbox::new(&mut dialog.kepub, t.copy_as_kepub),
      )
      .on_hover_text(t.copy_as_kepub_hint)
      .on_disabled_hover_text(t.no_kepubify);
      if !dialog.kepubify {
        ui.weak(t.no_kepubify);
      }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
      let ready = !dialog.chosen.is_empty() && !dialog.busy;
      let button = egui::Button::new(i18n::copy_button(lang, dialog.chosen.len()));
      if ui.add_enabled(ready, button).clicked() {
        let mut books: Vec<PathBuf> = dialog.chosen.iter().cloned().collect();
        books.sort();
        request = Some(Request::Copy {
          books,
          kepub: dialog.kepub,
        });
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
