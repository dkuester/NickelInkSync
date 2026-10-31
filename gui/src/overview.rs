//! Every connected device's position of each book side by side, with the
//! server's (O5): one column per device, one line per KOSync document, so
//! a book on the Tolino, the Kobo and the Kindle shows in one line.

use std::collections::HashMap;

use crate::devices::Found;
use crate::model;
use crate::sync::{self, Remote};

/// A device's column.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
  /// "Tolino Shine Color", "Kindle".
  pub name: String,
  /// Why the device could not be read.
  pub error: Option<String>,
}

/// A book on one device.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
  /// 0.0 to 1.0; `None` if never opened.
  pub percent: Option<f64>,
  pub last_read: Option<String>,
  /// KOReader's XPointer for the position, if it has one.
  pub xpointer: Option<String>,
}

/// One book (KOSync document) on all devices.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
  pub document: String,
  pub title: String,
  pub author: Option<String>,
  /// By column; `None` where the device does not have the book.
  pub cells: Vec<Option<Cell>>,
  /// `None`: nothing stored, or the server was not asked.
  pub server: Option<Remote>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
  pub columns: Vec<Column>,
  /// By title.
  pub lines: Vec<Line>,
  /// Whether the server was asked (`false`: none in `config.ini`), or why
  /// it could not be.
  pub server: Result<bool, String>,
}

/// Reads every device in `devices` (only the books that sync) and asks the
/// server about their books.
pub fn build(devices: &[Found]) -> Overview {
  let mut columns = Vec::new();
  let mut lines: Vec<Line> = Vec::new();
  let mut by_document: HashMap<String, usize> = HashMap::new();
  for (i, found) in devices.iter().enumerate() {
    let loaded = match model::load(&found.root, found.kind, &mut |_, _| {}) {
      Ok(loaded) => loaded,
      Err(e) => {
        columns.push(Column {
          name: found.name.clone(),
          error: Some(e),
        });
        continue;
      }
    };
    // The model, without the firmware.
    let name = loaded
      .device
      .split(", firmware")
      .next()
      .unwrap_or(&loaded.device)
      .to_string();
    columns.push(Column { name, error: None });
    for row in loaded.rows {
      let Some(document) = row.document else {
        continue;
      };
      let line = *by_document.entry(document.clone()).or_insert_with(|| {
        lines.push(Line {
          document,
          title: row.title.clone(),
          author: row.author.clone(),
          cells: Vec::new(),
          server: None,
        });
        lines.len() - 1
      });
      let cells = &mut lines[line].cells;
      cells.resize(i + 1, None);
      cells[i] = Some(Cell {
        percent: row.percent,
        last_read: row.last_read,
        xpointer: row.xpointer.ok(),
      });
    }
  }
  for line in &mut lines {
    line.cells.resize(columns.len(), None);
  }
  // Two devices of the same model keep apart by their volume name.
  let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
  for (column, found) in columns.iter_mut().zip(devices) {
    if names.iter().filter(|n| **n == column.name).count() > 1 {
      column.name = format!("{} ({})", column.name, found.name);
    }
  }
  let server = match devices.first() {
    Some(first) => {
      let documents: Vec<String> = lines.iter().map(|l| l.document.clone()).collect();
      match sync::remotes(&model::options(&first.root), &documents) {
        Ok(Some(mut found)) => {
          for line in &mut lines {
            line.server = found.remove(&line.document);
          }
          Ok(true)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e),
      }
    }
    None => Ok(false),
  };
  lines.sort_by_key(|l| l.title.to_lowercase());
  Overview {
    columns,
    lines,
    server,
  }
}
