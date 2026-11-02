//! A translation table: what [`crate::transfer`] needs of a mapping's
//! original EPUB to move positions between it and the device file, without
//! the original itself. The desktop builds one per mapped book when it
//! prepares a device ([`crate::prepare`]) and puts it on the device under
//! `.adds/NickelInkSync/tables/<device checksum>.json`; the CLI on the
//! device (`--live`) translates with it. The original EPUB never goes onto
//! the device, and neither does its text.
//!
//! The table holds, for each spine item of the original:
//!
//! - its path and how many visible characters it has (as
//!   [`crate::transfer`] counts them);
//! - each text node with visible characters: its KOReader path
//!   (`/body[1]/div[1]/p[3]/text()[1]`), where its visible characters start
//!   in the spine item, how many there are, its length in collapsed code
//!   points (KOReader's offsets), where its innermost block element (the
//!   paragraph) starts, and **anchors**: the collapsed offset of its first
//!   visible character and of every sentence start in it;
//! - where its sentences start.
//!
//! That is exactly what `position = paragraph` and `sentence` need: the
//! place sent is a paragraph or sentence start, which is an anchor, so the
//! device sends the same XPointer the desktop sends with the original
//! EPUB. A place between anchors (`position = exact`, a place whose
//! sentence starts too far back, and every position from the server) is
//! placed between the two anchors around it in proportion, which may be a
//! few characters off; [`Moved::approximate`] says so.
//!
//! How the two files' texts line up is decided when the table is built,
//! with both files at hand: for each chapter (spine items with calibre's
//! `_split_NNN` taken off) whose visible text differs between the device
//! file and the original, `differs` lists pairs of visible-character
//! indices (device, original) found by the text around the start of each
//! text node of the device file. A chapter not listed has the same text.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::PushPosition;
use crate::dom::{Element, Node};
use crate::epub::Epub;
use crate::position::{collapsed_len, koreader_path, split_xpointer, Error};
use crate::transfer::{
  at_percentage, block_of, chapter_key, find, group, group_at, locate, sentence_starts,
  ChapterText, How, SteppedBack, Transferred, MAX_BACK,
};

/// The format of a table; a newer one is not read.
pub const FORMAT: u32 = 1;

/// The folder of the tables below `.adds/NickelInkSync`.
pub const DIR: &str = "tables";

/// A text node of the original, as the table keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "NodeRow", into = "NodeRow")]
pub struct TextNode {
  /// KOReader's path below `DocFragment[n]`, without the offset.
  pub path: String,
  /// Visible characters in the spine item before this node.
  pub start: usize,
  /// Visible characters in this node (at least one).
  pub len: usize,
  /// Length in collapsed code points.
  pub collapsed: usize,
  /// Where the innermost block element around it starts (visible
  /// characters in the spine item).
  pub block: usize,
  /// `(visible character in the node, collapsed offset)`: the first
  /// visible character and every sentence start.
  pub anchors: Vec<(usize, usize)>,
}

/// A [`TextNode`] in the file: an array, as there are many of them.
#[derive(Serialize, Deserialize)]
struct NodeRow(String, usize, usize, usize, usize, Vec<(usize, usize)>);

impl From<NodeRow> for TextNode {
  fn from(r: NodeRow) -> Self {
    TextNode {
      path: r.0,
      start: r.1,
      len: r.2,
      collapsed: r.3,
      block: r.4,
      anchors: r.5,
    }
  }
}

impl From<TextNode> for NodeRow {
  fn from(n: TextNode) -> Self {
    NodeRow(n.path, n.start, n.len, n.collapsed, n.block, n.anchors)
  }
}

/// A spine item of the original.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
  /// Path from the EPUB root.
  pub path: String,
  /// Visible characters.
  pub chars: usize,
  /// Visible characters before each sentence start.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub sentences: Vec<usize>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub nodes: Vec<TextNode>,
}

/// What the device needs of a mapping's original EPUB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
  pub format: u32,
  /// KOReader partial MD5 of the device file.
  pub device: String,
  /// KOReader partial MD5 of the original EPUB.
  pub original: String,
  /// The original's spine.
  pub parts: Vec<Part>,
  /// Chapters whose text differs: `(device, original)` visible-character
  /// indices in the whole chapter that are the same place.
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub differs: BTreeMap<String, Vec<(usize, usize)>>,
}

/// A position moved with a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Moved {
  pub transferred: Transferred,
  /// Moved back to the start of the sentence or paragraph (towards the
  /// original only).
  pub stepped: Option<SteppedBack>,
  /// Placed between two anchors, not on one.
  pub approximate: bool,
}

#[derive(Debug)]
pub enum LoadError {
  Io(PathBuf, io::Error),
  Parse(PathBuf, String),
  NewerFormat(PathBuf, u32),
}

impl fmt::Display for LoadError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LoadError::Io(p, e) => write!(f, "{}: {e}", p.display()),
      LoadError::Parse(p, e) => write!(f, "{}: {e}", p.display()),
      LoadError::NewerFormat(p, v) => write!(
        f,
        "{}: format {v} is newer than this version understands ({FORMAT}); update NickelInkSync",
        p.display()
      ),
    }
  }
}

impl std::error::Error for LoadError {}

impl Table {
  /// The file of the table for the device file with checksum `device`, in
  /// the folder `.adds/NickelInkSync` of a device.
  pub fn path(dir: &Path, device: &str) -> PathBuf {
    dir.join(DIR).join(format!("{device}.json"))
  }

  pub fn load(path: &Path) -> Result<Self, LoadError> {
    let text = fs::read_to_string(path).map_err(|e| LoadError::Io(path.to_path_buf(), e))?;
    #[derive(Deserialize)]
    struct Version {
      format: u32,
    }
    let parse = |e: serde_json::Error| LoadError::Parse(path.to_path_buf(), e.to_string());
    let version: Version = serde_json::from_str(&text).map_err(parse)?;
    if version.format > FORMAT {
      return Err(LoadError::NewerFormat(path.to_path_buf(), version.format));
    }
    serde_json::from_str(&text).map_err(parse)
  }

  /// The table for moving positions between `device` and `original`, two
  /// files of the same book, with their checksums.
  pub fn build(
    device: &mut Epub,
    device_hash: &str,
    original: &mut Epub,
    original_hash: &str,
  ) -> Result<Self, Error> {
    let mut parts = Vec::new();
    for i in 0..original.spine.len() {
      let html = original.chapter(i)?;
      let text = ChapterText::of(&html);
      parts.push(part(&original.spine[i], &html, &text));
    }
    let mut differs = BTreeMap::new();
    let mut keys: Vec<String> = device.spine.iter().map(|p| chapter_key(p)).collect();
    keys.dedup();
    for key in keys {
      if differs.contains_key(&key) {
        continue;
      }
      let target = group(original, &key)?;
      if target.is_empty() {
        continue;
      }
      let source = group(device, &key)?;
      let chars = |texts: &crate::transfer::Parts| -> Vec<char> {
        texts
          .iter()
          .flat_map(|t| t.2.chars.iter().copied())
          .collect()
      };
      let (from, to) = (chars(&source), chars(&target));
      if from == to {
        continue;
      }
      // At the start of each text node of the device file.
      let mut pairs: Vec<(usize, usize)> = Vec::new();
      let mut base = 0;
      for (_, _, text) in &source {
        let mut last = None;
        for (i, &(node, _)) in text.at.iter().enumerate() {
          if last == Some(node) {
            continue;
          }
          last = Some(node);
          let d = base + i;
          if let Some(o) = find(&from, d, &to) {
            if pairs.last().is_none_or(|&(_, po)| o >= po) {
              pairs.push((d, o));
            }
          }
        }
        base += text.chars.len();
      }
      differs.insert(key, pairs);
    }
    Ok(Table {
      format: FORMAT,
      device: device_hash.to_string(),
      original: original_hash.to_string(),
      parts,
      differs,
    })
  }

  /// Moves the KOReader position `xpointer` in the device file `device`
  /// into the original, then back to the start of its sentence or
  /// paragraph as `mode` says, like [`crate::transfer::transfer`] and
  /// [`crate::transfer::step_back`] with the original at hand.
  pub fn to_original(
    &self,
    device: &mut Epub,
    xpointer: &str,
    percentage: f64,
    mode: PushPosition,
  ) -> Result<Moved, Error> {
    let source = group_at(device, xpointer)?;
    let parts = self.parts_of(&source.key);
    let (part, k, how) = if parts.is_empty() {
      let total: usize = self.parts.iter().map(|p| p.chars).sum();
      let index = (percentage.clamp(0.0, 1.0) * total as f64).floor() as usize;
      match self.locate(&(0..self.parts.len()).collect::<Vec<_>>(), index) {
        Some((part, k)) => (part, k, How::Percentage),
        None => return Ok(self.chapter_only(0, How::Percentage)),
      }
    } else {
      let (index, how) = match source.index {
        None => (0, How::ChapterStart),
        Some(d) => match self.differs.get(&source.key) {
          None => (d, How::SameText),
          Some(pairs) => match along(pairs, d, |p| p.0, |p| p.1) {
            Some(o) => (o, How::ByText),
            None => (0, How::ChapterStart),
          },
        },
      };
      match self.locate(&parts, index) {
        Some((part, k)) => (part, k, how),
        None => return Ok(self.chapter_only(parts[0], how.max(How::ChapterStart))),
      }
    };
    let (target, mut stepped) = self.step_back(part, k, mode);
    let (xpointer, approximate) = self.xpointer_at(part, target);
    if let Some(s) = &mut stepped {
      s.xpointer = xpointer.clone();
    }
    Ok(Moved {
      transferred: Transferred {
        xpointer,
        spine_index: part,
        how,
      },
      stepped,
      approximate,
    })
  }

  /// Moves the KOReader position `xpointer` in the original into the
  /// device file `device`, like [`crate::transfer::transfer`] with the
  /// original at hand.
  pub fn from_original(
    &self,
    device: &mut Epub,
    xpointer: &str,
    percentage: f64,
  ) -> Result<Moved, Error> {
    let (spine_index, below) = split_xpointer(xpointer)
      .filter(|&(i, _)| i < self.parts.len())
      .ok_or_else(|| Error::BadXPointer(xpointer.to_string()))?;
    let key = chapter_key(&self.parts[spine_index].path);
    let texts = group(device, &key)?;
    if texts.is_empty() {
      return Ok(Moved {
        transferred: at_percentage(device, percentage)?,
        stepped: None,
        approximate: false,
      });
    }
    let place = self.index_of(spine_index, below);
    let (index, how) = match place {
      None => (0, How::ChapterStart),
      Some((o, _)) => match self.differs.get(&key) {
        None => (o, How::SameText),
        Some(pairs) => match along(pairs, o, |p| p.1, |p| p.0) {
          Some(d) => (d, How::ByText),
          None => (0, How::ChapterStart),
        },
      },
    };
    Ok(Moved {
      transferred: locate(&texts, index, how),
      stepped: None,
      approximate: place.is_some_and(|(_, exact)| !exact),
    })
  }

  /// Spine indices of the original's parts of chapter `key`.
  fn parts_of(&self, key: &str) -> Vec<usize> {
    (0..self.parts.len())
      .filter(|&i| chapter_key(&self.parts[i].path) == key)
      .collect()
  }

  /// The part and visible character for visible character `index` in
  /// `parts`, as [`crate::transfer`]'s `locate` finds it; `None` if they
  /// have no visible text.
  fn locate(&self, parts: &[usize], index: usize) -> Option<(usize, usize)> {
    let mut left = index;
    for (n, &part) in parts.iter().enumerate() {
      let chars = self.parts[part].chars;
      let last = n + 1 == parts.len();
      if left < chars || (last && chars > 0) {
        return Some((part, left.min(chars - 1)));
      }
      left = left.saturating_sub(chars);
    }
    None
  }

  fn chapter_only(&self, part: usize, how: How) -> Moved {
    Moved {
      transferred: Transferred {
        xpointer: format!("/body[1]/DocFragment[{}]", part + 1),
        spine_index: part,
        how,
      },
      stepped: None,
      approximate: false,
    }
  }

  /// [`crate::transfer::step_back`] for visible character `k` of `part`:
  /// the character to send instead, and how far back that is.
  fn step_back(&self, part: usize, k: usize, mode: PushPosition) -> (usize, Option<SteppedBack>) {
    if mode == PushPosition::Exact {
      return (k, None);
    }
    let p = &self.parts[part];
    let Some(node) = node_at(&p.nodes, k) else {
      return (k, None);
    };
    let start = node.block.min(k);
    let sentence = p
      .sentences
      .iter()
      .rev()
      .find(|&&s| s <= k && s >= start)
      .copied()
      .unwrap_or(start);
    let target = match mode {
      PushPosition::Paragraph if k - start <= MAX_BACK => start,
      _ if k - sentence <= MAX_BACK => sentence,
      _ => k,
    };
    if target == k {
      return (k, None);
    }
    (
      target,
      Some(SteppedBack {
        // Set by the caller, from `xpointer_at`.
        xpointer: String::new(),
        chars: k - target,
        paragraph: target == start,
      }),
    )
  }

  /// KOReader's XPointer of visible character `k` in `part`, and whether
  /// it lies between anchors.
  fn xpointer_at(&self, part: usize, k: usize) -> (String, bool) {
    let p = &self.parts[part];
    let chapter = format!("/body[1]/DocFragment[{}]", part + 1);
    let Some(node) = node_at(&p.nodes, k) else {
      return (chapter, false);
    };
    let v = k - node.start;
    let (offset, exact) = between(&node.anchors, node.len, node.collapsed, v, |a| a.0, |a| a.1);
    (format!("{chapter}{}.{offset}", node.path), !exact)
  }

  /// Visible characters before the place `below` (KOReader's path below
  /// `DocFragment[n]`) in the chapter of `part`, counted over all its
  /// parts, and whether it is exact; `None` if the place is not found.
  fn index_of(&self, part: usize, below: &str) -> Option<(usize, bool)> {
    if below.is_empty() {
      return None;
    }
    let (steps, offset) = match below.rsplit_once('.') {
      Some((steps, offset)) if offset.bytes().all(|b| b.is_ascii_digit()) => {
        (steps, offset.parse::<usize>().ok()?)
      }
      _ => (below, 0),
    };
    let steps = normalize(steps);
    let nodes = &self.parts[part].nodes;
    let (node, offset) = match nodes.iter().find(|n| n.path == steps) {
      Some(node) => (node, offset),
      // A path to an element: its first text.
      None => {
        let prefix = format!("{steps}/");
        (nodes.iter().find(|n| n.path.starts_with(&prefix))?, 0)
      }
    };
    let (v, exact) = between(
      &node.anchors,
      node.collapsed,
      node.len,
      offset,
      |a| a.1,
      |a| a.0,
    );
    let key = chapter_key(&self.parts[part].path);
    let before: usize = self
      .parts_of(&key)
      .into_iter()
      .take_while(|&i| i < part)
      .map(|i| self.parts[i].chars)
      .sum();
    Some((before + node.start + v.min(node.len), exact))
  }
}

/// The table's record of one spine item of the original.
fn part(path: &str, html: &Element, text: &ChapterText) -> Part {
  let sentences: Vec<usize> = (0..text.chars.len())
    .filter(|&i| sentence_starts(&text.chars, i))
    .collect();
  let mut nodes: Vec<TextNode> = Vec::new();
  // The text node of the last character, and whether it has a record.
  let mut current: Option<(usize, bool)> = None;
  let mut next_sentence = sentences.iter().peekable();
  for (i, &(node, raw)) in text.at.iter().enumerate() {
    let starts_sentence = next_sentence.next_if(|&&s| s == i).is_some();
    let path = &text.nodes[node];
    let Some(Node::Text(t) | Node::CData(t)) = html.node_at(path) else {
      continue;
    };
    let collapsed = || collapsed_len(&t.chars().take(raw).collect::<String>());
    if current.map(|c| c.0) != Some(node) {
      let record = koreader_path(html, path, 0).map(|full| {
        let block = block_of(html, path);
        TextNode {
          path: full
            .rsplit_once('.')
            .map_or(full.as_str(), |(p, _)| p)
            .to_string(),
          start: i,
          len: 0,
          collapsed: collapsed_len(t),
          // The first visible character of the block, as `step_back`
          // finds it.
          block: text
            .at
            .iter()
            .position(|&(n, _)| text.nodes[n].starts_with(&block))
            .unwrap_or(i),
          anchors: vec![(0, collapsed())],
        }
      });
      current = Some((node, record.is_some()));
      nodes.extend(record);
    } else if starts_sentence && current.is_some_and(|c| c.1) {
      let n = nodes.last_mut().expect("the current node");
      n.anchors.push((i - n.start, collapsed()));
    }
    if current.is_some_and(|c| c.1) {
      let n = nodes.last_mut().expect("the current node");
      n.len = i + 1 - n.start;
    }
  }
  Part {
    path: path.to_string(),
    chars: text.chars.len(),
    sentences,
    nodes,
  }
}

/// The node that holds visible character `k`, or the last one before it.
fn node_at(nodes: &[TextNode], k: usize) -> Option<&TextNode> {
  let i = nodes.partition_point(|n| n.start <= k);
  nodes.get(i.checked_sub(1)?)
}

/// `x` (from 0 to `len_x`) in another count (from 0 to `len_y`) along
/// anchors that are known in both, in proportion between them; and
/// whether `x` is an anchor.
fn between<A>(
  anchors: &[A],
  len_x: usize,
  len_y: usize,
  x: usize,
  ax: impl Fn(&A) -> usize,
  ay: impl Fn(&A) -> usize,
) -> (usize, bool) {
  let i = anchors.partition_point(|a| ax(a) <= x);
  let Some(a) = i.checked_sub(1).map(|i| &anchors[i]) else {
    // Before the first visible character: that one.
    return (anchors.first().map_or(x.min(len_y), &ay), false);
  };
  if ax(a) == x {
    return (ay(a), true);
  }
  let (end_x, end_y) = anchors.get(i).map_or((len_x, len_y), |b| (ax(b), ay(b)));
  let span_x = end_x.saturating_sub(ax(a)).max(1);
  let span_y = end_y.saturating_sub(ay(a));
  let y = ay(a) + ((x - ax(a)) * span_y + span_x / 2) / span_x;
  (y.min(end_y.max(ay(a))), false)
}

/// `x` in the other file along the pairs of a chapter whose text differs:
/// from the pair at or before it on, or before the first pair, back from
/// it. `None` without pairs (the text was not found).
fn along(
  pairs: &[(usize, usize)],
  x: usize,
  from: impl Fn(&(usize, usize)) -> usize,
  to: impl Fn(&(usize, usize)) -> usize,
) -> Option<usize> {
  let i = pairs.partition_point(|p| from(p) <= x);
  match i.checked_sub(1) {
    Some(i) => Some(to(&pairs[i]) + (x - from(&pairs[i]))),
    None => pairs.first().map(|p| to(p).saturating_sub(from(p) - x)),
  }
}

/// KOReader's path with `[1]` where a step has no index (`/body/p` is
/// `/body[1]/p[1]`), as the table stores it.
fn normalize(steps: &str) -> String {
  steps
    .split('/')
    .map(|s| {
      if s.is_empty() || s.ends_with(']') {
        s.to_string()
      } else {
        format!("{s}[1]")
      }
    })
    .collect::<Vec<_>>()
    .join("/")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn between_anchors_in_proportion() {
    let anchors = [(0, 0), (10, 12)];
    assert_eq!(between(&anchors, 20, 24, 10, |a| a.0, |a| a.1), (12, true));
    assert_eq!(between(&anchors, 20, 24, 5, |a| a.0, |a| a.1), (6, false));
    assert_eq!(between(&anchors, 20, 24, 15, |a| a.0, |a| a.1), (18, false));
    // The other way round.
    assert_eq!(between(&anchors, 24, 20, 12, |a| a.1, |a| a.0), (10, true));
  }

  #[test]
  fn along_the_pairs() {
    let pairs = [(5, 7), (20, 25)];
    assert_eq!(along(&pairs, 6, |p| p.0, |p| p.1), Some(8));
    assert_eq!(along(&pairs, 3, |p| p.0, |p| p.1), Some(5));
    assert_eq!(along(&pairs, 30, |p| p.1, |p| p.0), Some(25));
    assert_eq!(along(&[], 3, |p| p.0, |p| p.1), None);
  }

  #[test]
  fn normalizes_steps() {
    assert_eq!(
      normalize("/body/div[2]/p/text()"),
      "/body[1]/div[2]/p[1]/text()[1]"
    );
  }
}
