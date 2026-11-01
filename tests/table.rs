//! Translation tables (`nickelinksync::table`): on the device, a table
//! must give the same position as the desktop with the original EPUB at
//! hand, for a kepub (Book A, Book R) and for a calibre-like conversion
//! (a cover page in front, a chapter split in two, other whitespace).

mod common;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use common::fixtures;
use nickelinksync::config::PushPosition;
use nickelinksync::epub::Epub;
use nickelinksync::table::Table;
use nickelinksync::transfer::{step_back, transfer, How};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

fn epub(name: &str) -> PathBuf {
  fixtures().join("epub").join(name)
}

fn table(device: &Path, original: &Path) -> Table {
  Table::build(
    &mut Epub::open(device).unwrap(),
    "device",
    &mut Epub::open(original).unwrap(),
    "original",
  )
  .unwrap()
}

/// Positions in the original: every `step`-th text node of the book, at
/// each anchor, a few characters after one and in its middle.
fn originals(table: &Table, step: usize) -> Vec<(String, bool)> {
  let mut out = Vec::new();
  let nodes = table
    .parts
    .iter()
    .enumerate()
    .flat_map(|(i, part)| part.nodes.iter().map(move |node| (i, node)));
  for (i, node) in nodes.step_by(step) {
    let at = |offset: usize| format!("/body[1]/DocFragment[{}]{}.{offset}", i + 1, node.path);
    for &(_, c) in &node.anchors {
      out.push((at(c), true));
      out.push((at(c + 3), false));
    }
    out.push((at(node.collapsed / 2), false));
  }
  out
}

/// What the desktop sends for the device position `xpointer`: moved into
/// the original, then back to the sentence or paragraph start.
fn desktop_push(
  device: &Path,
  original: &Path,
  xpointer: &str,
  mode: PushPosition,
) -> (String, How) {
  let moved = transfer(
    &mut Epub::open(device).unwrap(),
    xpointer,
    &mut Epub::open(original).unwrap(),
    0.5,
  )
  .unwrap();
  let stepped = step_back(&mut Epub::open(original).unwrap(), &moved.xpointer, mode).unwrap();
  (stepped.map_or(moved.xpointer, |s| s.xpointer), moved.how)
}

/// The device and the desktop agree in both directions.
fn same_as_desktop(device: &Path, original: &Path, step: usize) -> usize {
  let table = table(device, original);
  let mut checked = 0;
  for (at, anchor) in originals(&table, step) {
    // Pull: the server's position into the device file.
    let desktop = transfer(
      &mut Epub::open(original).unwrap(),
      &at,
      &mut Epub::open(device).unwrap(),
      0.5,
    )
    .unwrap();
    let on_device = table
      .from_original(&mut Epub::open(device).unwrap(), &at, 0.5)
      .unwrap();
    if anchor {
      assert_eq!(on_device.transferred, desktop, "pull {at}");
      assert!(!on_device.approximate, "pull {at}");
    } else {
      // Between anchors: the same text node, maybe a few characters off.
      let node = |x: &str| x.rsplit_once('.').unwrap().0.to_string();
      assert_eq!(
        node(&on_device.transferred.xpointer),
        node(&desktop.xpointer),
        "pull {at}"
      );
    }

    // Push: from that place in the device file back to the original.
    for mode in [PushPosition::Paragraph, PushPosition::Sentence] {
      let (expected, how) = desktop_push(device, original, &desktop.xpointer, mode);
      let moved = table
        .to_original(
          &mut Epub::open(device).unwrap(),
          &desktop.xpointer,
          0.5,
          mode,
        )
        .unwrap();
      assert_eq!(moved.transferred.xpointer, expected, "push {at} ({mode:?})");
      assert_eq!(moved.transferred.how, how, "push {at} ({mode:?})");
    }
    let (expected, _) = desktop_push(device, original, &desktop.xpointer, PushPosition::Exact);
    let moved = table
      .to_original(
        &mut Epub::open(device).unwrap(),
        &desktop.xpointer,
        0.5,
        PushPosition::Exact,
      )
      .unwrap();
    if !moved.approximate {
      assert_eq!(moved.transferred.xpointer, expected, "push exact {at}");
    }
    checked += 1;
  }
  checked
}

#[test]
fn book_r_kepub_gives_what_the_desktop_gives() {
  let n = same_as_desktop(&epub("book-r.kepub.epub"), &epub("book-r.epub"), 40);
  assert!(n > 40, "{n}");
}

#[test]
fn book_a_kepub_gives_what_the_desktop_gives() {
  let n = same_as_desktop(&epub("book-a.kepub.epub"), &epub("book-a.epub"), 8);
  assert!(n > 40, "{n}");
}

/// Book R as calibre might convert it: a cover page in front, chapter 2
/// split into `_split_000` and `_split_001` after paragraph 100, other
/// line breaks between the paragraphs, and with `edit` a changed word.
fn calibre_like(to: &Path, edit: bool) {
  let mut zip = ZipArchive::new(fs::File::open(epub("book-r.epub")).unwrap()).unwrap();
  let mut out = ZipWriter::new(fs::File::create(to).unwrap());
  let options = SimpleFileOptions::default();
  let head = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Kapitel 2</title></head><body>";
  for i in 0..zip.len() {
    let mut entry = zip.by_index(i).unwrap();
    let name = entry.name().unwrap().to_string();
    let mut text = String::new();
    entry.read_to_string(&mut text).unwrap();
    match name.as_str() {
      "OEBPS/content.opf" => {
        text = text
          .replace(
            "<item id=\"s2\" href=\"text/chapter2.xhtml\" media-type=\"application/xhtml+xml\"/>",
            "<item id=\"cover\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\
             <item id=\"s2a\" href=\"text/chapter2_split_000.xhtml\" media-type=\"application/xhtml+xml\"/>\
             <item id=\"s2b\" href=\"text/chapter2_split_001.xhtml\" media-type=\"application/xhtml+xml\"/>",
          )
          .replace(
            "<itemref idref=\"s0\"/>",
            "<itemref idref=\"cover\"/><itemref idref=\"s0\"/>",
          )
          .replace(
            "<itemref idref=\"s2\"/>",
            "<itemref idref=\"s2a\"/><itemref idref=\"s2b\"/>",
          );
      }
      "OEBPS/text/chapter2.xhtml" => {
        let text = text.replace("</p>\n<p>", "</p>\n\n  <p>");
        let text = match edit {
          true => text.replace("(R2-50) ", "(R2-50) eingefügt "),
          false => text,
        };
        let split = text.find("<p>R2-101 ").unwrap();
        let start = text.find("<h1>").unwrap();
        let first = format!("{head}{}</body></html>", &text[start..split]);
        let end = text.find("</body>").unwrap();
        let second = format!("{head}{}</body></html>", &text[split..end]);
        for (file, data) in [
          ("chapter2_split_000", first),
          ("chapter2_split_001", second),
        ] {
          out
            .start_file(format!("OEBPS/text/{file}.xhtml"), options)
            .unwrap();
          out.write_all(data.as_bytes()).unwrap();
        }
        continue;
      }
      _ => {}
    }
    out.start_file(name, options).unwrap();
    out.write_all(text.as_bytes()).unwrap();
  }
  out.start_file("OEBPS/cover.xhtml", options).unwrap();
  out
    .write_all(b"<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body><img src=\"c.jpg\" alt=\"\"/></body></html>")
    .unwrap();
  out.finish().unwrap();
}

fn temp(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("nickelinksync-{}-table", std::process::id()));
  fs::create_dir_all(&dir).unwrap();
  dir.join(name)
}

#[test]
fn a_calibre_like_conversion_gives_what_the_desktop_gives() {
  let device = temp("calibre.epub");
  calibre_like(&device, false);
  assert_eq!(
    Epub::open(&device).unwrap().spine[3],
    "OEBPS/text/chapter2_split_000.xhtml"
  );
  let t = table(&device, &epub("book-r.epub"));
  assert!(t.differs.is_empty(), "{:?}", t.differs.keys());
  let n = same_as_desktop(&device, &epub("book-r.epub"), 40);
  assert!(n > 40, "{n}");
}

#[test]
fn a_changed_text_is_found_by_the_text_around_it() {
  let device = temp("calibre-edited.epub");
  calibre_like(&device, true);
  let original = epub("book-r.epub");
  let t = table(&device, &original);
  assert_eq!(
    t.differs.keys().collect::<Vec<_>>(),
    ["OEBPS/text/chapter2.xhtml"]
  );
  // KOReader in R2-120 and R2-20: after and before the change.
  for p in [120, 20] {
    let at = format!("/body[1]/DocFragment[3]/body[1]/p[{p}]/text()[1].0");
    let desktop = transfer(
      &mut Epub::open(&original).unwrap(),
      &at,
      &mut Epub::open(&device).unwrap(),
      0.5,
    )
    .unwrap();
    let on_device = t
      .from_original(&mut Epub::open(&device).unwrap(), &at, 0.5)
      .unwrap();
    assert_eq!(on_device.transferred, desktop, "pull {at}");
    assert_eq!(desktop.how, How::ByText);
    let (expected, how) = desktop_push(
      &device,
      &original,
      &desktop.xpointer,
      PushPosition::Paragraph,
    );
    let moved = t
      .to_original(
        &mut Epub::open(&device).unwrap(),
        &desktop.xpointer,
        0.5,
        PushPosition::Paragraph,
      )
      .unwrap();
    assert_eq!(moved.transferred.xpointer, expected, "push {at}");
    assert_eq!(expected, at);
    assert_eq!((moved.transferred.how, how), (How::ByText, How::ByText));
  }
}

#[test]
fn a_table_is_small_and_holds_no_text() {
  let t = table(&epub("book-r.kepub.epub"), &epub("book-r.epub"));
  let text = serde_json::to_string(&t).unwrap();
  assert!(!text.contains("Bäckerei"), "the table holds text");
  let original = fs::metadata(epub("book-r.epub")).unwrap().len() as usize;
  assert!(text.len() < original, "{} bytes", text.len());
}
