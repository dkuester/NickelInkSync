//! Positions moved between two files of the same book
//! (`nickelinksync::transfer`): kepub ↔ EPUB with KOReader's own highlights
//! of the same words, the Kobo 6.0 reading position in Book R, and a
//! calibre-like conversion of Book R (cover page replaced, a chapter split,
//! text changed) that the test builds itself.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use nickelinksync::epub::Epub;
use nickelinksync::position::{to_xpointer, PointFlavor, Position};
use nickelinksync::transfer::{transfer, How, Transferred};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn epub(file: &str) -> Epub {
  Epub::open(&fixtures().join("epub").join(file)).unwrap()
}

/// `["key"] = "value",` lines of a KOReader metadata.epub.lua, in order.
fn lua_values(sdr: &str, key: &str) -> Vec<String> {
  let lua = fs::read_to_string(
    fixtures()
      .join("koreader")
      .join(sdr)
      .join("metadata.epub.lua"),
  )
  .unwrap();
  let prefix = format!("[\"{key}\"] = \"");
  lua
    .lines()
    .filter_map(|l| l.trim().strip_prefix(&prefix)?.strip_suffix("\","))
    .map(str::to_string)
    .collect()
}

/// KOReader's highlight starts by word.
fn highlights(sdr: &str) -> Vec<(String, String)> {
  lua_values(sdr, "text")
    .into_iter()
    .zip(lua_values(sdr, "pos0"))
    .collect()
}

fn moved(from: &mut Epub, xpointer: &str, to: &mut Epub) -> Transferred {
  transfer(from, xpointer, to, 0.5).unwrap()
}

/// KOReader highlighted the same words in the kepub and the EPUB of Book P
/// and Book Q; each start moves to exactly the other one, both ways.
#[test]
fn koreader_highlights_kepub_and_epub() {
  for book in ["book-p", "book-q"] {
    let mut kepub = epub(&format!("{book}.kepub.epub"));
    let mut plain = epub(&format!("{book}.epub"));
    let in_kepub = highlights(&format!("{book}.kepub.sdr"));
    let in_epub = highlights(&format!("{book}.sdr"));
    assert_eq!(in_kepub.len(), 8, "{book}");
    for (word, kepub_pos) in &in_kepub {
      let epub_pos = &in_epub.iter().find(|(w, _)| w == word).unwrap().1;
      let there = moved(&mut kepub, kepub_pos, &mut plain);
      assert_eq!(&there.xpointer, epub_pos, "{word} kepub → EPUB");
      assert_eq!(there.how, How::SameText, "{word}");
      let back = moved(&mut plain, epub_pos, &mut kepub);
      assert_eq!(&back.xpointer, kepub_pos, "{word} EPUB → kepub");
    }
  }
}

/// The reading position the Kobo 6.0 stored in the Book R kepub
/// (`#kobo.17.1:204`, the page start in R2-16) lands on the same character
/// of the EPUB.
#[test]
fn kobo_6_position_in_the_epub() {
  let mut kepub = epub("book-r.kepub.epub");
  let mut plain = epub("book-r.epub");
  let stored = Position::parse("OEBPS/text/chapter2.xhtml#kobo.17.1:204");
  let x = to_xpointer(&mut kepub, &stored, PointFlavor::AllNodes).unwrap();
  let there = moved(&mut kepub, &x.xpointer, &mut plain);
  assert_eq!(
    there.xpointer,
    "/body[1]/DocFragment[3]/body[1]/p[16]/text()[1].204"
  );
  assert_eq!(there.how, How::SameText);
  // A chapter-only position stays one.
  let start = moved(&mut kepub, "/body[1]/DocFragment[3]", &mut plain);
  assert_eq!(start.how, How::ChapterStart);
  assert_eq!(start.spine_index, 2);
}

/// Book R as calibre's conversion might leave it: the title page replaced
/// by `titlepage.xhtml` (an image only), chapter 2 split after paragraph
/// 100, and in chapter 3 a paragraph added in front and every `ß` written
/// `ss` in paragraphs 1 to 9.
fn converted_book_r(name: &str) -> PathBuf {
  let source = fixtures().join("epub/book-r.epub");
  let mut zip = ZipArchive::new(fs::File::open(&source).unwrap()).unwrap();
  let mut read = |name: &str| {
    let mut text = String::new();
    zip
      .by_name(name)
      .unwrap()
      .read_to_string(&mut text)
      .unwrap();
    text
  };
  let container = read("META-INF/container.xml");
  let chapter1 = read("OEBPS/text/chapter1.xhtml");
  let chapter2 = read("OEBPS/text/chapter2.xhtml");
  let chapter3 = read("OEBPS/text/chapter3.xhtml");
  let end = read("OEBPS/text/end.xhtml");

  let (head, body) = chapter2.split_once("<body>").unwrap();
  let cut = body.find("<p>R2-101 ").unwrap();
  let split0 = format!("{head}<body>{}</body>\n</html>\n", &body[..cut]);
  let split1 = format!("{head}<body>\n{}", &body[cut..]);
  let (head3, body3) = chapter3.split_once("<h1>").unwrap();
  let mut body3 = format!("<p>Ein neuer Absatz vorweg.</p>\n<h1>{body3}");
  let cut3 = body3.find("<p>R3-10 ").unwrap();
  let early = body3[..cut3].replace('ß', "ss");
  body3.replace_range(..cut3, &early);
  let chapter3 = format!("{head3}{body3}");

  let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:test:book-r-converted</dc:identifier>
    <dc:title>Book R</dc:title>
    <dc:creator>Author R</dc:creator>
  </metadata>
  <manifest>
    <item id="titlepage" href="../titlepage.xhtml" media-type="application/xhtml+xml"/>
    <item id="c1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2a" href="text/chapter2_split_000.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2b" href="text/chapter2_split_001.xhtml" media-type="application/xhtml+xml"/>
    <item id="c3" href="text/chapter3.xhtml" media-type="application/xhtml+xml"/>
    <item id="end" href="text/end.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="titlepage"/>
    <itemref idref="c1"/>
    <itemref idref="c2a"/>
    <itemref idref="c2b"/>
    <itemref idref="c3"/>
    <itemref idref="end"/>
  </spine>
</package>
"#;
  let titlepage = "<?xml version=\"1.0\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Cover</title></head><body><div><img src=\"cover.jpg\" alt=\"\"/></div></body></html>\n";

  let path = std::env::temp_dir().join(format!("nickelinksync-{}-{name}.epub", std::process::id()));
  let mut out = ZipWriter::new(fs::File::create(&path).unwrap());
  let options = SimpleFileOptions::default();
  for (name, text) in [
    ("mimetype", "application/epub+zip".to_string()),
    ("META-INF/container.xml", container),
    ("OEBPS/content.opf", opf.to_string()),
    ("titlepage.xhtml", titlepage.to_string()),
    ("OEBPS/text/chapter1.xhtml", chapter1),
    ("OEBPS/text/chapter2_split_000.xhtml", split0),
    ("OEBPS/text/chapter2_split_001.xhtml", split1),
    ("OEBPS/text/chapter3.xhtml", chapter3),
    ("OEBPS/text/end.xhtml", end),
  ] {
    out.start_file(name, options).unwrap();
    out.write_all(text.as_bytes()).unwrap();
  }
  out.finish().unwrap();
  path
}

#[test]
fn split_chapters_and_a_replaced_cover() {
  let mut original = epub("book-r.epub");
  let mut converted = Epub::open(&converted_book_r("split")).unwrap();
  assert_eq!(converted.spine[3], "OEBPS/text/chapter2_split_001.xhtml");

  // R2-150 is paragraph 50 of the second part, spine item 3.
  let there = moved(
    &mut original,
    "/body[1]/DocFragment[3]/body[1]/p[150]/text()[1].5",
    &mut converted,
  );
  assert_eq!(
    there.xpointer,
    "/body[1]/DocFragment[4]/body[1]/p[50]/text()[1].5"
  );
  assert_eq!(there.how, How::SameText);
  let back = moved(&mut converted, &there.xpointer, &mut original);
  assert_eq!(
    back.xpointer,
    "/body[1]/DocFragment[3]/body[1]/p[150]/text()[1].5"
  );
  // In the first part nothing moves but the spine index.
  let first = moved(
    &mut original,
    "/body[1]/DocFragment[3]/body[1]/p[7]/text()[1].0",
    &mut converted,
  );
  assert_eq!(
    first.xpointer,
    "/body[1]/DocFragment[3]/body[1]/p[7]/text()[1].0"
  );

  // The title page is not in the conversion and its titlepage.xhtml not
  // in the original: the percentage decides.
  let title = transfer(
    &mut original,
    "/body[1]/DocFragment[1]/body[1]/h1[1]/text()[1].0",
    &mut converted,
    0.0,
  )
  .unwrap();
  assert_eq!(title.how, How::Percentage);
  assert_eq!(title.spine_index, 1, "the first text: chapter 1");
  let cover = transfer(
    &mut converted,
    "/body[1]/DocFragment[1]",
    &mut original,
    0.5,
  )
  .unwrap();
  assert_eq!(cover.how, How::Percentage);
  assert_eq!(cover.spine_index, 2, "half way: in chapter 2");
}

#[test]
fn changed_text_is_found_by_its_words() {
  let mut original = epub("book-r.epub");
  let mut converted = Epub::open(&converted_book_r("text")).unwrap();
  // "Stuhl Garten Seite" in R3-85, as the Kindle stored it: one paragraph
  // later in the conversion.
  let there = moved(
    &mut original,
    "/body[1]/DocFragment[4]/body[1]/p[85]/text()[1].187",
    &mut converted,
  );
  assert_eq!(
    there.xpointer,
    "/body[1]/DocFragment[5]/body[1]/p[86]/text()[1].187"
  );
  assert_eq!(there.how, How::ByText);
  // In R3-5 the ß became ss: the text before the position differs, the
  // 32 characters after it still match.
  let there = moved(
    &mut original,
    "/body[1]/DocFragment[4]/body[1]/p[5]/text()[1].100",
    &mut converted,
  );
  assert_eq!(there.how, How::ByText);
  assert!(
    there
      .xpointer
      .starts_with("/body[1]/DocFragment[5]/body[1]/p[6]/text()[1]."),
    "{}",
    there.xpointer
  );
}

#[test]
fn text_that_is_nowhere_falls_back_to_the_chapter_start() {
  // Book P and Book Q share the path of their probe chapter but not a
  // word of its text.
  let mut p = epub("book-p.epub");
  let mut q = epub("book-q.epub");
  let start = moved(
    &mut p,
    "/body[1]/DocFragment[2]/body[1]/p[3]/text()[1].14",
    &mut q,
  );
  assert_eq!(start.how, How::ChapterStart);
  assert_eq!(start.spine_index, 1);
  // And a bad XPointer is an error, not a guess.
  assert!(transfer(&mut p, "/body[1]/DocFragment[99]", &mut q, 0.5).is_err());
}

/// A one-chapter EPUB with this body, written by the test.
fn one_chapter(name: &str, body: &str) -> PathBuf {
  let path = std::env::temp_dir().join(format!("nickelinksync-{}-{name}.epub", std::process::id()));
  let mut out = ZipWriter::new(fs::File::create(&path).unwrap());
  let options = SimpleFileOptions::default();
  for (name, text) in [
    ("mimetype", "application/epub+zip".to_string()),
    (
      "META-INF/container.xml",
      r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_string(),
    ),
    (
      "content.opf",
      r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata/><manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c"/></spine></package>"#.to_string(),
    ),
    (
      "c.xhtml",
      format!("<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body>{body}</body></html>"),
    ),
  ] {
    out.start_file(name, options).unwrap();
    out.write_all(text.as_bytes()).unwrap();
  }
  out.finish().unwrap();
  path
}

#[test]
fn steps_back_to_the_paragraph_or_sentence_start() {
  use nickelinksync::config::PushPosition::{Exact, Paragraph, Sentence};
  use nickelinksync::transfer::step_back;
  // Book R, R2-16 at character 204, where the Kobo 6.0's page started:
  // the paragraph is one sentence, so both go to its start.
  let mut r = epub("book-r.epub");
  let at = "/body[1]/DocFragment[3]/body[1]/p[16]/text()[1].204";
  let back = step_back(&mut r, at, Paragraph).unwrap().unwrap();
  assert_eq!(
    back.xpointer,
    "/body[1]/DocFragment[3]/body[1]/p[16]/text()[1].0"
  );
  assert!(back.paragraph);
  assert_eq!(
    back.to_string(),
    format!(
      "moved back {} characters to the paragraph start",
      back.chars
    )
  );
  assert_eq!(
    step_back(&mut r, at, Sentence).unwrap().unwrap().xpointer,
    back.xpointer
  );
  assert_eq!(step_back(&mut r, at, Exact).unwrap(), None);
  // Already at the start, or only the chapter: nothing to do.
  assert_eq!(step_back(&mut r, &back.xpointer, Paragraph).unwrap(), None);
  assert_eq!(
    step_back(&mut r, "/body[1]/DocFragment[3]", Paragraph).unwrap(),
    None
  );

  // A paragraph of 40 sentences of 29 visible characters each: a place in
  // sentence 3 goes to the paragraph start, one in sentence 30 (too far
  // in) only to its sentence start, which is also where `sentence` goes.
  let sentence = |n: usize| format!("Satz {n:02} hat genau dreißig Zeichen. ");
  let paragraph: String = (1..=40).map(sentence).collect();
  let mut long = Epub::open(&one_chapter(
    "long-paragraph",
    &format!("<p>Kurz.</p><p><em>Lang:</em> {paragraph}</p>"),
  ))
  .unwrap();
  // Character offsets in the second text node of p[2]: " Satz 01 …".
  let offset_of = |n: usize| 1 + (n - 1) * sentence(1).chars().count();
  let near = format!(
    "/body[1]/DocFragment[1]/body[1]/p[2]/text()[1].{}",
    offset_of(3) + 5
  );
  let back = step_back(&mut long, &near, Paragraph).unwrap().unwrap();
  assert_eq!(
    back.xpointer,
    "/body[1]/DocFragment[1]/body[1]/p[2]/em[1]/text()[1].0"
  );
  let far = format!(
    "/body[1]/DocFragment[1]/body[1]/p[2]/text()[1].{}",
    offset_of(30) + 5
  );
  let back = step_back(&mut long, &far, Paragraph).unwrap().unwrap();
  assert!(!back.paragraph);
  let start_of_30 = format!(
    "/body[1]/DocFragment[1]/body[1]/p[2]/text()[1].{}",
    offset_of(30)
  );
  assert_eq!(back.xpointer, start_of_30);
  assert_eq!(back.chars, 4, "\"Satz\" before the place");
  let back = step_back(&mut long, &near, Sentence).unwrap().unwrap();
  assert_eq!(
    back.xpointer,
    format!(
      "/body[1]/DocFragment[1]/body[1]/p[2]/text()[1].{}",
      offset_of(3)
    )
  );
}
