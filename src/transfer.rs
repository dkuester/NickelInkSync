//! A KOReader position in one file of a book, moved to another file of the
//! same book: from a kepub or a differently converted EPUB on the device to
//! the original EPUB the mapping names ([`crate::mapping`]), and back.
//!
//! The two files need not have the same structure. A kepub adds wrapper
//! `div`s and `koboSpan` spans, and calibre's conversion replaces the cover
//! page and splits large chapters (`ch1.xhtml` becomes
//! `ch1_split_000.xhtml`, `ch1_split_001.xhtml`, …). So the position is
//! moved by chapter and text, not by node path or spine index:
//!
//! 1. **Chapter**: by the spine item's path, with calibre's `_split_NNN`
//!    taken off, so the parts of a split chapter are one chapter.
//! 2. **Text**: the chapter's visible characters in document order, without
//!    whitespace and invisible characters (soft hyphen, zero-width
//!    characters), so whitespace that a conversion adds or drops between
//!    elements does not count. The position is the number of visible
//!    characters before it. If the two chapters have the same visible text,
//!    that number is the position in the other file ([`How::SameText`]).
//!    If not, the 32 characters after the position (or before it, at the
//!    end of a chapter) are looked up in the other chapter, the occurrence
//!    nearest to the same relative place wins ([`How::ByText`]).
//! 3. **Fallbacks**, shown to the user: the chapter is there but the text is
//!    not found: its start ([`How::ChapterStart`]); the chapter is not in
//!    the other file at all: the place at the book's percentage, counted in
//!    visible characters over the whole book ([`How::Percentage`]).
//!
//! In the target file the character becomes KOReader's XPointer (node path
//! and collapsed offset), so a position that falls on whitespace moves to
//! the next visible character.
//!
//! [`step_back`] moves a position to the start of its sentence or
//! paragraph before it is sent (`position` in the configuration).

use std::fmt;

use crate::config::PushPosition;
use crate::dom::{Element, Node};
use crate::epub::Epub;
use crate::position::{koreader_path, node_for, split_xpointer, Error};

/// How a position was moved to the other file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum How {
  /// Same chapter, same visible text: the same character.
  SameText,
  /// Same chapter, other text: found by the text around it.
  ByText,
  /// Same chapter, but the place in it is unknown: the chapter start.
  ChapterStart,
  /// The chapter is not in the other file: the place at the percentage.
  Percentage,
}

impl fmt::Display for How {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      How::SameText => "same text",
      How::ByText => "found by its text",
      How::ChapterStart => "chapter start only",
      How::Percentage => "from the percentage only",
    })
  }
}

/// A position moved to the other file.
#[derive(Debug, Clone, PartialEq)]
pub struct Transferred {
  /// KOReader XPointer in the other file.
  pub xpointer: String,
  /// 0-based spine index in the other file.
  pub spine_index: usize,
  pub how: How,
}

/// Characters looked up in the other chapter when the texts differ.
const ANCHOR: usize = 32;
/// Shorter anchors are not trusted.
const MIN_ANCHOR: usize = 8;

/// Moves the KOReader position `xpointer` in `from` to `to`, two files of
/// the same book. `percentage` (0 to 1) is only used when the chapter is
/// not in `to`. A position that names only the chapter
/// (`/body[1]/DocFragment[n]`) gives at best [`How::ChapterStart`].
pub fn transfer(
  from: &mut Epub,
  xpointer: &str,
  to: &mut Epub,
  percentage: f64,
) -> Result<Transferred, Error> {
  let bad = || Error::BadXPointer(xpointer.to_string());
  let (spine_index, below) = split_xpointer(xpointer)
    .filter(|&(i, _)| i < from.spine.len())
    .ok_or_else(bad)?;
  let key = chapter_key(&from.spine[spine_index]);

  // The source chapter, all its parts, and the place in them.
  let from_parts = parts(from, &key);
  let mut source = Vec::new();
  let mut base = 0;
  let mut local = None;
  for &part in &from_parts {
    let html = from.chapter(part)?;
    let text = ChapterText::of(&html);
    if part == spine_index {
      base = source.len();
      local = Some(
        Some(below)
          .filter(|b| !b.is_empty())
          .and_then(|b| node_for(&html, b))
          .map(|(path, chars)| text.index_of(&path, chars)),
      );
    }
    source.extend_from_slice(&text.chars);
  }
  let known = local.flatten();
  let k = base + known.unwrap_or(0);

  let to_parts = parts(to, &key);
  if to_parts.is_empty() {
    return at_percentage(to, percentage);
  }
  let mut target = Vec::new();
  let mut texts = Vec::new();
  for &part in &to_parts {
    let html = to.chapter(part)?;
    let text = ChapterText::of(&html);
    target.extend_from_slice(&text.chars);
    texts.push((part, html, text));
  }

  let (found, how) = match known {
    None => (0, How::ChapterStart),
    Some(_) if source == target => (k, How::SameText),
    Some(_) => match find(&source, k, &target) {
      Some(i) => (i, How::ByText),
      None => (0, How::ChapterStart),
    },
  };
  Ok(locate(&texts, found, how))
}

/// Visible characters a paragraph start may lie before the position for
/// `position = paragraph`, about half a page; further back, the start of
/// the sentence is taken.
pub const MAX_BACK: usize = 600;

/// Where [`step_back`] moved a position.
#[derive(Debug, Clone, PartialEq)]
pub struct SteppedBack {
  pub xpointer: String,
  /// Visible characters it moved back.
  pub chars: usize,
  /// `true` for the paragraph start, `false` for a sentence start.
  pub paragraph: bool,
}

impl fmt::Display for SteppedBack {
  /// `moved back 125 characters to the paragraph start`
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "moved back {} characters to the {} start",
      self.chars,
      if self.paragraph {
        "paragraph"
      } else {
        "sentence"
      }
    )
  }
}

/// Moves the KOReader position `xpointer` in `epub` back to the start of
/// its sentence or paragraph, as `mode` says, never more than [`MAX_BACK`]
/// visible characters. `None` if it stays: `exact`, a chapter-only
/// position, one already at such a start, or one whose sentence starts
/// further back.
///
/// The paragraph is the innermost block element around the position (`p`,
/// `div`, `li`, a heading, …). A sentence starts after `.`, `!`, `?` or `…`
/// and any closing quotes or brackets; an abbreviation like "Dr." also
/// counts as one, which only moves the position back less.
pub fn step_back(
  epub: &mut Epub,
  xpointer: &str,
  mode: PushPosition,
) -> Result<Option<SteppedBack>, Error> {
  if mode == PushPosition::Exact {
    return Ok(None);
  }
  let (spine_index, below) = split_xpointer(xpointer)
    .filter(|&(i, _)| i < epub.spine.len())
    .ok_or_else(|| Error::BadXPointer(xpointer.to_string()))?;
  if below.is_empty() {
    return Ok(None);
  }
  let html = epub.chapter(spine_index)?;
  let Some((path, chars)) = node_for(&html, below) else {
    return Ok(None);
  };
  let text = ChapterText::of(&html);
  let k = text.index_of(&path, chars);
  let block = block_of(&html, &path);
  let start = text
    .at
    .iter()
    .position(|&(node, _)| text.nodes[node].starts_with(&block))
    .unwrap_or(k)
    .min(k);
  let sentence = (start..=k)
    .rev()
    .find(|&i| i == start || sentence_starts(&text.chars, i))
    .unwrap_or(start);
  // Never further back than about half a page.
  let target = match mode {
    PushPosition::Paragraph if k - start <= MAX_BACK => start,
    _ if k - sentence <= MAX_BACK => sentence,
    _ => k,
  };
  if target == k {
    return Ok(None);
  }
  let moved = locate(&[(spine_index, html, text)], target, How::SameText);
  Ok(Some(SteppedBack {
    xpointer: moved.xpointer,
    chars: k - target,
    paragraph: target == start,
  }))
}

/// The child path of the innermost block element on the way to the node at
/// `path`; `[]` (all of the chapter) if there is none.
fn block_of(html: &Element, path: &[usize]) -> Vec<usize> {
  let mut block = Vec::new();
  let mut element = html;
  for (depth, &i) in path.iter().enumerate() {
    let Some(Node::Element(child)) = element.children.get(i) else {
      break;
    };
    if is_block(&child.name) {
      block = path[..=depth].to_vec();
    }
    element = child;
  }
  block
}

fn is_block(name: &str) -> bool {
  matches!(
    name,
    "p"
      | "div"
      | "li"
      | "blockquote"
      | "h1"
      | "h2"
      | "h3"
      | "h4"
      | "h5"
      | "h6"
      | "dd"
      | "dt"
      | "td"
      | "th"
      | "pre"
      | "figcaption"
      | "section"
      | "article"
      | "aside"
  )
}

/// A sentence starts at visible character `i`: after `.`, `!`, `?` or `…`
/// and any closing quotes or brackets, and `i` is none of those itself.
fn sentence_starts(chars: &[char], i: usize) -> bool {
  let ends = |c: char| matches!(c, '.' | '!' | '?' | '…');
  let closes = |c: char| {
    matches!(
      c,
      '"' | '\'' | '”' | '’' | '“' | '»' | '«' | '›' | '‹' | ')' | ']'
    )
  };
  if chars.get(i).is_some_and(|&c| ends(c) || closes(c)) {
    return false;
  }
  let mut j = i;
  while j > 0 && closes(chars[j - 1]) {
    j -= 1;
  }
  j > 0 && ends(chars[j - 1])
}

/// The XPointer of visible character `index` in the parts of a chapter.
fn locate(texts: &[(usize, Element, ChapterText)], index: usize, how: How) -> Transferred {
  let mut left = index;
  for (n, (part, html, text)) in texts.iter().enumerate() {
    let last = n + 1 == texts.len();
    if left < text.chars.len() || (last && !text.chars.is_empty()) {
      let i = left.min(text.chars.len() - 1);
      let (node, chars) = text.at[i];
      if let Some(below) = koreader_path(html, &text.nodes[node], chars) {
        return Transferred {
          xpointer: format!("/body[1]/DocFragment[{}]{below}", part + 1),
          spine_index: *part,
          how,
        };
      }
    }
    left = left.saturating_sub(text.chars.len());
  }
  // No visible text at all (a cover or image page): its start.
  let part = texts.first().map_or(0, |t| t.0);
  Transferred {
    xpointer: format!("/body[1]/DocFragment[{}]", part + 1),
    spine_index: part,
    how: how.max(How::ChapterStart),
  }
}

/// The place at `percentage` of the book's visible text.
fn at_percentage(epub: &mut Epub, percentage: f64) -> Result<Transferred, Error> {
  let mut texts = Vec::new();
  for part in 0..epub.spine.len() {
    let html = epub.chapter(part)?;
    let text = ChapterText::of(&html);
    texts.push((part, html, text));
  }
  let total: usize = texts.iter().map(|t| t.2.chars.len()).sum();
  let index = (percentage.clamp(0.0, 1.0) * total as f64).floor() as usize;
  Ok(locate(&texts, index, How::Percentage))
}

/// The spine indices of a chapter's parts, in reading order.
fn parts(epub: &Epub, key: &str) -> Vec<usize> {
  (0..epub.spine.len())
    .filter(|&i| chapter_key(&epub.spine[i]) == key)
    .collect()
}

/// A spine item's path without calibre's `_split_NNN` before the extension.
pub fn chapter_key(path: &str) -> String {
  let (stem, ext) = match path.rfind('.') {
    Some(dot) if !path[dot..].contains('/') => path.split_at(dot),
    _ => (path, ""),
  };
  match stem.rsplit_once("_split_") {
    Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
      format!("{base}{ext}")
    }
    _ => path.to_string(),
  }
}

/// Where the `ANCHOR` characters at `k` in `source` are in `target`: the
/// occurrence nearest to the same relative place.
fn find(source: &[char], k: usize, target: &[char]) -> Option<usize> {
  if source.is_empty() || target.is_empty() {
    return None;
  }
  let k = k.min(source.len());
  let expected = k * target.len() / source.len();
  let nearest = |hits: Vec<usize>| hits.into_iter().min_by_key(|&i| i.abs_diff(expected));
  // After the position, else (near the end) before it.
  let after = &source[k..(k + ANCHOR).min(source.len())];
  if after.len() >= MIN_ANCHOR {
    if let Some(i) = nearest(occurrences(target, after)) {
      return Some(i);
    }
  }
  let before = &source[k.saturating_sub(ANCHOR)..k];
  if before.len() >= MIN_ANCHOR {
    return nearest(occurrences(target, before)).map(|i| i + before.len());
  }
  None
}

fn occurrences(haystack: &[char], needle: &[char]) -> Vec<usize> {
  if needle.is_empty() || needle.len() > haystack.len() {
    return Vec::new();
  }
  haystack
    .windows(needle.len())
    .enumerate()
    .filter(|(_, w)| *w == needle)
    .map(|(i, _)| i)
    .collect()
}

/// Neither whitespace nor invisible.
fn visible(c: char) -> bool {
  !c.is_whitespace()
    && !matches!(
      c,
      '\u{ad}' | '\u{200b}'..='\u{200d}' | '\u{2060}' | '\u{feff}'
    )
}

/// The visible characters of a chapter's `body`, in document order, each
/// with the text node it is in and its character index there.
struct ChapterText {
  chars: Vec<char>,
  /// For each character: index into `nodes`, character index in the node.
  at: Vec<(usize, usize)>,
  /// Text and CDATA nodes as child paths from `html`.
  nodes: Vec<Vec<usize>>,
}

impl ChapterText {
  fn of(html: &Element) -> Self {
    let mut text = ChapterText {
      chars: Vec::new(),
      at: Vec::new(),
      nodes: Vec::new(),
    };
    for (i, child) in html.children.iter().enumerate() {
      if let Node::Element(e) = child {
        if e.name == "body" {
          text.walk(e, &mut vec![i]);
        }
      }
    }
    text
  }

  fn walk(&mut self, element: &Element, path: &mut Vec<usize>) {
    for (i, child) in element.children.iter().enumerate() {
      path.push(i);
      match child {
        Node::Text(t) | Node::CData(t) => {
          let node = self.nodes.len();
          self.nodes.push(path.clone());
          for (n, c) in t.chars().enumerate() {
            if visible(c) {
              self.chars.push(c);
              self.at.push((node, n));
            }
          }
        }
        Node::Element(e) => self.walk(e, path),
        _ => {}
      }
      path.pop();
    }
  }

  /// Visible characters before character `chars` of the node at `path`.
  fn index_of(&self, path: &[usize], chars: usize) -> usize {
    let Some(node) = self.nodes.iter().position(|n| n == path) else {
      return 0;
    };
    self
      .at
      .iter()
      .take_while(|&&(n, c)| n < node || (n == node && c < chars))
      .count()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn split_chapters_share_a_key() {
    assert_eq!(
      chapter_key("OEBPS/Text/ch1_split_001.xhtml"),
      "OEBPS/Text/ch1.xhtml"
    );
    assert_eq!(chapter_key("OEBPS/Text/ch1.xhtml"), "OEBPS/Text/ch1.xhtml");
    assert_eq!(chapter_key("a_split_x.html"), "a_split_x.html");
    assert_eq!(chapter_key("dir.v2/a_split_3"), "dir.v2/a");
  }

  #[test]
  fn finds_by_text_nearest_to_the_same_place() {
    let source: Vec<char> = "aaaaXYZbbbbbbbbbbbbbbXYZcccccccc".chars().collect();
    let target: Vec<char> = "##aaaaXYZbbbbbbbbbbbbbbXYZcccccccc".chars().collect();
    // "XYZcccccccc" occurs once: after the second XYZ.
    let k = 21;
    assert_eq!(find(&source, k, &target), Some(k + 2));
    // At the very end only the text before helps.
    let k = source.len();
    assert_eq!(find(&source, k, &target), Some(target.len()));
    assert_eq!(
      find(&source, 3, &"nothing alike".chars().collect::<Vec<_>>()),
      None
    );
  }

  #[test]
  fn sentence_starts_after_closing_quotes() {
    let chars: Vec<char> = "Er ging.„Nein!“Sie lachte…Dann".chars().collect();
    let starts: Vec<usize> = (0..chars.len())
      .filter(|&i| sentence_starts(&chars, i))
      .collect();
    let words: Vec<String> = starts
      .iter()
      .map(|&i| chars[i..].iter().take(3).collect())
      .collect();
    assert_eq!(words, ["„Ne", "Sie", "Dan"]);
  }

  #[test]
  fn block_is_the_innermost_paragraph() {
    let html =
      crate::dom::parse("<html><body><div><p>a <span><em>b</em></span></p></div></body></html>")
        .unwrap();
    // html > body[0] > div[0] > p[0] > span[1] > em[0] > text[0]
    assert_eq!(block_of(&html, &[0, 0, 0, 1, 0, 0]), [0, 0, 0]);
    assert_eq!(block_of(&html, &[0]), Vec::<usize>::new());
  }

  #[test]
  fn visible_text_skips_whitespace_and_soft_hyphens() {
    let html = crate::dom::parse(
      "<html><head><title>t</title></head><body>\n<p>a b\u{ad}c</p>\n<p><![CDATA[d]]> e</p></body></html>",
    )
    .unwrap();
    let text = ChapterText::of(&html);
    assert_eq!(text.chars.iter().collect::<String>(), "abcde");
    // "c" is character 4 of the first p's text.
    assert_eq!(text.at[2], (1, 4));
    assert_eq!(text.index_of(&text.nodes[1], 4), 2);
  }
}
