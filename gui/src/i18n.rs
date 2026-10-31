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
  pub own_checksum: &'static str,
  pub own_checksum_hint: &'static str,
  pub own_in_library: &'static str,
  pub own_original: &'static str,
  pub no_reference: &'static str,
  pub no_reference_hint: &'static str,
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
  pub writes_tolino: &'static str,
  pub map_button: &'static str,
  pub remap_button: &'static str,
  pub unmap_button: &'static str,
  pub map_title: &'static str,
  pub comparing: &'static str,
  pub no_library_for_suggestions: &'static str,
  pub same_in_library: &'static str,
  pub no_candidates: &'static str,
  pub uncertain: &'static str,
  pub choose_epub: &'static str,
  /// Only where the EPUB path is typed in (no file dialog).
  #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
  pub epub_path: &'static str,
  pub chosen: &'static str,
  pub map_confirm: &'static str,
  pub cancel: &'static str,
  pub map_failed: &'static str,
  pub server_comparing: &'static str,
  pub server_not_configured: &'static str,
  pub server_failed: &'static str,
  pub server: &'static str,
  pub send_button: &'static str,
  pub send_all: &'static str,
  pub pull_button: &'static str,
  pub pull_all: &'static str,
  pub nothing_stored: &'static str,
  pub outlook_same: &'static str,
  pub outlook_device: &'static str,
  pub outlook_server: &'static str,
  pub outlook_asks: &'static str,
  pub ask_title: &'static str,
  pub device: &'static str,
  pub device_behind: &'static str,
  pub positions_differ: &'static str,
  pub send_device: &'static str,
  pub keep_server: &'static str,
  pub server_behind: &'static str,
  pub keep_device: &'static str,
  pub take_server: &'static str,
  pub apply: &'static str,
  pub history: &'static str,
  pub history_empty: &'static str,
  pub to_server: &'static str,
  pub to_device: &'static str,
  pub send_anyway: &'static str,
  pub time: &'static str,
  pub direction: &'static str,
  pub by: &'static str,
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

/// "92 %: title 100 %, author 100 %, chapters 30/30".
pub fn score(
  lang: Lang,
  score: f64,
  title: f64,
  author: Option<f64>,
  chapters: Option<(usize, usize)>,
) -> String {
  let p = |x: f64| format!("{:.0} %", x * 100.0);
  let (t, a, none, c) = match lang {
    Lang::De => ("Titel", "Autor", "Autor nicht verglichen", "Kapitel"),
    Lang::En => ("title", "author", "author not compared", "chapters"),
  };
  let mut s = format!("{}: {t} {}", p(score), p(title));
  match author {
    Some(author) => s.push_str(&format!(", {a} {}", p(author))),
    None => s.push_str(&format!(", {none}")),
  }
  if let Some((found, total)) = chapters {
    s.push_str(&format!(", {c} {found}/{total}"));
  }
  s
}

pub fn unmap_question(lang: Lang, title: &str) -> String {
  match lang {
    Lang::De => {
      format!("Zuordnung von „{title}“ entfernen? Das Buch wird dann nicht mehr synchronisiert.")
    }
    Lang::En => format!("Remove the mapping of “{title}”? The book is then no longer synced."),
  }
}

pub fn mapped_notice(lang: Lang, title: &str, original: &str) -> String {
  match lang {
    Lang::De => format!("„{title}“ ist zugeordnet: {original}"),
    Lang::En => format!("“{title}” is mapped: {original}"),
  }
}

pub fn same_file_notice(lang: Lang, title: &str) -> String {
  match lang {
    Lang::De => {
      format!("„{title}“ ist dieselbe Datei wie die gewählte EPUB und braucht keine Zuordnung.")
    }
    Lang::En => format!("“{title}” is the same file as the chosen EPUB and needs no mapping."),
  }
}

pub fn unmapped_notice(lang: Lang, title: &str) -> String {
  match lang {
    Lang::De => format!("Zuordnung von „{title}“ entfernt."),
    Lang::En => format!("Removed the mapping of “{title}”."),
  }
}

/// Asked before a book without a reference EPUB is sent.
pub fn unreferenced_question(lang: Lang, title: &str) -> String {
  match lang {
    Lang::De => format!(
      "Zu „{title}“ ist keine Referenz-EPUB bekannt: Es ist unklar, ob KOReader das Buch kennt. \
       Trotzdem unter der Prüfsumme der Datei auf dem Gerät senden?"
    ),
    Lang::En => format!(
      "No reference EPUB is known for “{title}”: it is unclear whether KOReader knows the book. \
       Send it under the checksum of the file on the device anyway?"
    ),
  }
}

/// What a push or pull did, in one line: "2 gesendet, 1 aktuell, …", the
/// titles of books that failed or were left out, and where the backup of
/// the device database is.
pub fn report(lang: Lang, report: &crate::sync::Report) -> String {
  use crate::sync::Outcome;
  let count = |f: fn(&Outcome) -> bool| report.books.iter().filter(|(_, s)| f(s)).count();
  let parts: [(usize, &str, &str); 9] = [
    (count(|s| *s == Outcome::Sent), "gesendet", "sent"),
    (
      count(|s| *s == Outcome::Written),
      "aufs Gerät geschrieben",
      "written to the device",
    ),
    (
      count(|s| *s == Outcome::InSync),
      "schon aktuell",
      "already in sync",
    ),
    (
      count(|s| *s == Outcome::ServerNewer),
      "Server neuer, nicht gesendet",
      "server newer, not sent",
    ),
    (
      count(|s| *s == Outcome::DeviceNewer),
      "Gerät neuer, nicht geholt",
      "device newer, not taken",
    ),
    (
      count(|s| *s == Outcome::NothingStored),
      "nichts auf dem Server",
      "nothing on the server",
    ),
    (
      count(|s| *s == Outcome::NotInLine),
      "vom Plugin gesperrt (nicht in Einklang)",
      "held by the plugin (not in line)",
    ),
    (
      count(|s| matches!(s, Outcome::Skipped(_))),
      "übersprungen",
      "skipped",
    ),
    (
      count(|s| matches!(s, Outcome::Failed(_))),
      "fehlgeschlagen",
      "failed",
    ),
  ];
  let mut text: Vec<String> = parts
    .iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, de, en)| match lang {
      Lang::De => format!("{n} {de}"),
      Lang::En => format!("{n} {en}"),
    })
    .collect();
  if !report.questions.is_empty() {
    let n = report.questions.len();
    text.push(match lang {
      Lang::De => format!("{n} mit Rückfrage"),
      Lang::En => format!("{n} to answer"),
    });
  }
  if text.is_empty() {
    text.push(match lang {
      Lang::De => "nichts zu tun".into(),
      Lang::En => "nothing to do".into(),
    });
  }
  let mut line = text.join(", ");
  for (title, sent) in &report.books {
    if let Outcome::Failed(e) | Outcome::Skipped(e) = sent {
      line.push_str(&format!("\n{title}: {e}"));
    }
  }
  if let Some(backup) = &report.backup {
    let n = report.pages_dropped;
    line.push_str(&match lang {
      Lang::De => format!("\nSicherung der Gerätedatenbank: {}", backup.display()),
      Lang::En => format!("\nBackup of the device database: {}", backup.display()),
    });
    // Nickel keeps the dropped pages in memory until it restarts.
    line.push_str(&match (lang, n) {
      (Lang::De, 0) => "\nGerät auswerfen, bevor du das Buch öffnest.".to_string(),
      (Lang::En, 0) => "\nEject the device before you open the book.".to_string(),
      (Lang::De, n) => format!(
        "\n{n} zwischengespeicherte Seiten von Nickel verdeckten die Stelle und wurden entfernt. \
         Gerät auswerfen und neu starten, bevor du das Buch öffnest: Nickel hält diese Seiten \
         bis zum Neustart im Speicher."
      ),
      (Lang::En, n) => format!(
        "\nDropped {n} of Nickel's cached pages that hid the place. Eject the device and \
         restart it before you open the book: Nickel keeps those pages in memory until it \
         restarts."
      ),
    });
  }
  line
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
  no_library: "Keine Bibliothek mit Referenz-EPUBs eingestellt (library = in config.ini): Die \
               App kann nicht prüfen, ob KOReader diese Bücher kennt. Trag die Bibliothek ein \
               oder ordne die Bücher selbst zu.",
  mappings: "Zuordnungen",
  own_checksum: "Eigene Prüfsumme",
  own_checksum_hint: "Synchronisiert unter der Prüfsumme der Datei auf dem Gerät. KOReader \
                      findet die Stelle nur, wenn er genau diese Datei hat.",
  own_in_library: "gleiche Datei wie in der Bibliothek",
  own_original: "Original einer Zuordnung",
  no_reference: "Ohne Referenz-EPUB",
  no_reference_hint: "Keine Referenz-EPUB bekannt, also unklar, ob KOReader das Buch kennt: \
                      Es würde unter der Prüfsumme der Datei auf dem Gerät gesendet. Ein kepub \
                      oder umgewandeltes EPUB braucht eine Zuordnung.",
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
  read_only: "Die App schreibt nichts aufs Gerät. Zuordnungen liegen auf diesem Rechner \
              (mappings.json), gemeinsam mit der CLI.",
  writes_tolino: "„Holen“ schreibt die Stelle vom Server in die Datenbank des Tolino; vorher \
                  legt die App eine Sicherung auf diesem Rechner an. Zuordnungen liegen auf \
                  diesem Rechner (mappings.json), gemeinsam mit der CLI.",
  map_button: "Zuordnen …",
  remap_button: "Ändern …",
  unmap_button: "Entfernen",
  map_title: "Zuordnung für",
  comparing: "Vergleiche mit der Bibliothek …",
  no_library_for_suggestions: "Keine Referenz-EPUB gefunden: Es ist keine Bibliothek \
                               eingestellt (library = in config.ini). Wähle die Original-EPUB \
                               selbst.",
  same_in_library: "Dieselbe Datei liegt in der Bibliothek, es braucht keine Zuordnung:",
  no_candidates: "Keine Referenz-EPUB gefunden: kein passendes Buch in der Bibliothek. Wähle \
                  die Original-EPUB selbst.",
  uncertain: "unsicher, bitte prüfen",
  choose_epub: "EPUB wählen …",
  epub_path: "Pfad der EPUB",
  chosen: "Gewählt",
  map_confirm: "Zuordnen",
  cancel: "Abbrechen",
  map_failed: "Zuordnen fehlgeschlagen",
  server_comparing: "Vergleiche mit dem Server …",
  server_not_configured: "Kein Server eingerichtet (server, user, password in config.ini): Die Positionen werden nicht verglichen.",
  server_failed: "Server nicht erreichbar",
  server: "Server",
  send_button: "Senden",
  send_all: "Alle senden",
  pull_button: "Holen",
  pull_all: "Alle holen",
  nothing_stored: "nichts gespeichert",
  outlook_same: "gleich",
  outlook_device: "Gerät neuer",
  outlook_server: "Server neuer",
  outlook_asks: "fragt vorher",
  ask_title: "Rückfrage (confirm in config.ini)",
  device: "Gerät",
  device_behind: "Die Stelle auf dem Gerät liegt vor der auf dem Server.",
  positions_differ: "Die Stellen weichen voneinander ab.",
  send_device: "Gerät senden",
  keep_server: "Server behalten",
  server_behind: "Die Stelle auf dem Server liegt vor der auf dem Gerät.",
  keep_device: "Gerät behalten",
  take_server: "Server übernehmen",
  apply: "Übernehmen",
  history: "Verlauf",
  history_empty: "Noch nichts synchronisiert.",
  to_server: "zum Server",
  to_device: "aufs Gerät",
  send_anyway: "Trotzdem senden",
  time: "Zeit",
  direction: "Richtung",
  by: "von",
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
  no_library: "No library with reference EPUBs set up (library = in config.ini): the app \
               cannot check whether KOReader knows these books. Set up the library or map the \
               books yourself.",
  mappings: "Mappings",
  own_checksum: "Own checksum",
  own_checksum_hint: "Synced under the checksum of the file on the device. KOReader finds the \
                      position only if it has exactly this file.",
  own_in_library: "the same file as in the library",
  own_original: "the original of a mapping",
  no_reference: "No reference EPUB",
  no_reference_hint: "No reference EPUB known, so it is unclear whether KOReader knows the \
                      book: it would be sent under the checksum of the file on the device. A \
                      kepub or a converted EPUB needs a mapping.",
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
  read_only: "The app writes nothing to the device. Mappings are kept on this computer \
              (mappings.json), shared with the CLI.",
  writes_tolino: "“Get” writes the server’s position into the Tolino’s database, after a \
                  backup of it on this computer. Mappings are kept on this computer \
                  (mappings.json), shared with the CLI.",
  map_button: "Map …",
  remap_button: "Change …",
  unmap_button: "Remove",
  map_title: "Mapping for",
  comparing: "Comparing with the library …",
  no_library_for_suggestions: "No reference EPUB found: no library is set up (library = \
                               in config.ini). Choose the original EPUB yourself.",
  same_in_library: "The same file is in the library, no mapping needed:",
  no_candidates: "No reference EPUB found: no matching book in the library. Choose the \
                  original EPUB yourself.",
  uncertain: "uncertain, please check",
  choose_epub: "Choose EPUB …",
  epub_path: "Path of the EPUB",
  chosen: "Chosen",
  map_confirm: "Map",
  cancel: "Cancel",
  map_failed: "Mapping failed",
  server_comparing: "Comparing with the server …",
  server_not_configured:
    "No server set up (server, user, password in config.ini): positions are not compared.",
  server_failed: "Server not reachable",
  server: "Server",
  send_button: "Send",
  send_all: "Send all",
  pull_button: "Get",
  pull_all: "Get all",
  nothing_stored: "nothing stored",
  outlook_same: "same",
  outlook_device: "device newer",
  outlook_server: "server newer",
  outlook_asks: "asks first",
  ask_title: "Question (confirm in config.ini)",
  device: "Device",
  device_behind: "The position on the device lies before the one on the server.",
  positions_differ: "The positions differ.",
  send_device: "Send the device’s",
  keep_server: "Keep the server’s",
  server_behind: "The position on the server lies before the one on the device.",
  keep_device: "Keep the device’s",
  take_server: "Take the server’s",
  apply: "Apply",
  history: "History",
  history_empty: "Nothing synced yet.",
  to_server: "to the server",
  to_device: "to the device",
  send_anyway: "Send anyway",
  time: "Time",
  direction: "Direction",
  by: "by",
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
