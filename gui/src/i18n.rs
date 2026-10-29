//! German and English texts. Every text is a field of [`Texts`], so a
//! missing translation does not compile.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
  De,
  En,
}

/// The language setting: the system's, or one chosen in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
  #[default]
  Auto,
  De,
  En,
}

impl Choice {
  pub fn lang(self) -> Lang {
    match self {
      Choice::Auto => from_locale(sys_locale::get_locale().as_deref()),
      Choice::De => Lang::De,
      Choice::En => Lang::En,
    }
  }
}

/// German for a German locale (`de`, `de-DE`, `de_AT`, …), else English.
pub fn from_locale(locale: Option<&str>) -> Lang {
  match locale {
    Some(l) if l.to_ascii_lowercase().starts_with("de") => Lang::De,
    _ => Lang::En,
  }
}

pub struct Texts {
  pub language: &'static str,
  pub language_auto: &'static str,
  pub rescan: &'static str,
  pub devices: &'static str,
  pub no_devices: &'static str,
  pub kindle_folder: &'static str,
  pub choose_kindle_folder: &'static str,
  pub kindle_folder_hint: &'static str,
  pub kindle_folder_path: &'static str,
  pub remove_kindle_folder: &'static str,
  pub not_a_kindle_folder: &'static str,
  pub no_auto_sync: &'static str,
  pub choose_device: &'static str,
  pub loading: &'static str,
  pub failed: &'static str,
  pub library: &'static str,
  pub no_library: &'static str,
  pub mappings: &'static str,
  pub same_file: &'static str,
  pub same_file_hint: &'static str,
  pub mapped: &'static str,
  pub mapped_hint: &'static str,
  pub not_mapped: &'static str,
  pub not_mapped_hint: &'static str,
  pub problems: &'static str,
  pub none: &'static str,
  pub title: &'static str,
  pub file: &'static str,
  pub position: &'static str,
  pub last_read: &'static str,
  pub original: &'static str,
  pub why: &'static str,
  pub never_opened: &'static str,
  pub no_books: &'static str,
  pub read_only: &'static str,
}

pub fn texts(lang: Lang) -> &'static Texts {
  match lang {
    Lang::De => &DE,
    Lang::En => &EN,
  }
}

/// "3 of 12 books".
pub fn progress(lang: Lang, done: usize, total: usize) -> String {
  match lang {
    Lang::De => format!("{done} von {total} Büchern"),
    Lang::En => format!("{done} of {total} books"),
  }
}

/// "12 EPUBs" after the library's path.
pub fn epubs(lang: Lang, n: usize) -> String {
  match lang {
    Lang::De | Lang::En => format!("{n} EPUBs"),
  }
}

static DE: Texts = Texts {
  language: "Sprache",
  language_auto: "Systemsprache",
  rescan: "Neu suchen",
  devices: "Geräte",
  no_devices: "Kein Kobo oder Tolino angesteckt.",
  kindle_folder: "Kindle",
  choose_kindle_folder: "Kindle-Ordner wählen …",
  kindle_folder_hint: "Ein Kindle bietet am Mac nur MTP: Kopiere seine Bücher (Ordner \
                       documents) z. B. mit OpenMTP und wähle die Kopie.",
  kindle_folder_path: "Pfad der Kopie",
  remove_kindle_folder: "Kindle-Ordner entfernen",
  not_a_kindle_folder: "Kein Kindle-Ordner (kein documents/ und keine .kfx mit .sdr).",
  no_auto_sync: "Beim Anstecken wird nichts synchronisiert.",
  choose_device: "Wähle links ein Gerät.",
  loading: "Lese Bücher …",
  failed: "Konnte das Gerät nicht lesen",
  library: "Bibliothek",
  no_library: "Keine Bibliothek eingestellt (library = in config.ini): Bücher werden unter \
               ihrer eigenen Prüfsumme synchronisiert.",
  mappings: "Zuordnungen",
  same_file: "Gleiche Datei",
  same_file_hint: "Synchronisiert unter der eigenen Prüfsumme.",
  mapped: "Zugeordnet",
  mapped_hint: "Synchronisiert unter der Prüfsumme der Original-EPUB.",
  not_mapped: "Nicht zugeordnet",
  not_mapped_hint: "Wird nicht synchronisiert, bis es einer Original-EPUB zugeordnet ist.",
  problems: "Nicht lesbar",
  none: "keine",
  title: "Titel",
  file: "Datei",
  position: "Stelle",
  last_read: "Zuletzt gelesen",
  original: "Original",
  why: "Grund",
  never_opened: "nie geöffnet",
  no_books: "Keine Bücher auf dem Gerät.",
  read_only: "Nur lesend: In dieser Version schreibt die App nichts aufs Gerät.",
};

static EN: Texts = Texts {
  language: "Language",
  language_auto: "System language",
  rescan: "Rescan",
  devices: "Devices",
  no_devices: "No Kobo or Tolino connected.",
  kindle_folder: "Kindle",
  choose_kindle_folder: "Choose Kindle folder …",
  kindle_folder_hint: "On a Mac a Kindle only offers MTP: copy its books (the documents \
                       folder), e.g. with OpenMTP, and choose the copy.",
  kindle_folder_path: "Path of the copy",
  remove_kindle_folder: "Remove Kindle folder",
  not_a_kindle_folder: "Not a Kindle folder (no documents/ and no .kfx with .sdr).",
  no_auto_sync: "Nothing is synced when a device is connected.",
  choose_device: "Choose a device on the left.",
  loading: "Reading books …",
  failed: "Could not read the device",
  library: "Library",
  no_library: "No library set up (library = in config.ini): books sync under their own \
               checksum.",
  mappings: "Mappings",
  same_file: "Same file",
  same_file_hint: "Synced under its own checksum.",
  mapped: "Mapped",
  mapped_hint: "Synced under the checksum of the original EPUB.",
  not_mapped: "Not mapped",
  not_mapped_hint: "Not synced until it is mapped to an original EPUB.",
  problems: "Not readable",
  none: "none",
  title: "Title",
  file: "File",
  position: "Position",
  last_read: "Last read",
  original: "Original",
  why: "Reason",
  never_opened: "never opened",
  no_books: "No books on the device.",
  read_only: "Read-only: this version writes nothing to the device.",
};

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn german_locales_get_german() {
    for l in ["de", "de-DE", "de_AT", "DE-ch"] {
      assert_eq!(from_locale(Some(l)), Lang::De, "{l}");
    }
    for l in ["en-US", "fr-FR", "nl"] {
      assert_eq!(from_locale(Some(l)), Lang::En, "{l}");
    }
    assert_eq!(from_locale(None), Lang::En);
    assert_eq!(Choice::De.lang(), Lang::De);
    assert_eq!(Choice::En.lang(), Lang::En);
  }
}
