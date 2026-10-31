//! Suggestions for a book mapping: which original EPUB in the library a
//! device book most likely is.
//!
//! The score is meant to be read, not just ranked by, so each part is shown
//! on its own:
//!
//! - **title**: word overlap (Dice coefficient: twice the shared words over
//!   all words) between the device title and the EPUB's title, or its file
//!   name if that fits better. A Send-to-Kindle book's `title` is the stem
//!   of the file it was sent as, often Calibre's `Title - Author`.
//! - **author**: the same over the author names, in any order; left out if
//!   either side has none.
//! - **chapters**: for a kepub or EPUB, the share of the device file's spine
//!   items found by path in the EPUB; for a Kindle KFX, sections against
//!   spine items. Only for the best candidates, as it opens each EPUB.
//!
//! Words are lower-cased, with common accents and umlauts folded (`ö` = `o`,
//! `ß` = `ss`); numbers stay words, so "Volume 1" and "Volume 2" differ.
//!
//! Score: title × 0.7 + author × 0.3 (title alone without authors), and with
//! chapters 0.8 × that + 0.2 × chapters. Candidates below [`MIN_SCORE`] are
//! not shown; from [`LIKELY`] on, a candidate is offered as the mapping.

use std::collections::BTreeSet;
use std::fmt;

use crate::library::LibraryBook;

/// Below this the candidate is not shown.
pub const MIN_SCORE: f64 = 0.5;

/// From this on the best candidate is offered as the mapping; below, the
/// owner has to check it.
pub const LIKELY: f64 = 0.8;

/// How a device book compares to one library book.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate<'a> {
  pub book: &'a LibraryBook,
  pub title: f64,
  /// `None` if the device book or the EPUB has no author.
  pub author: Option<f64>,
  /// Matching chapters and how many there are, if compared.
  pub chapters: Option<(usize, usize)>,
}

impl Candidate<'_> {
  /// From title and author only.
  pub fn metadata_score(&self) -> f64 {
    match self.author {
      Some(author) => 0.7 * self.title + 0.3 * author,
      None => self.title,
    }
  }

  pub fn score(&self) -> f64 {
    match self.chapters {
      Some((_, 0)) | None => self.metadata_score(),
      Some((found, total)) => 0.8 * self.metadata_score() + 0.2 * found as f64 / total as f64,
    }
  }
}

impl fmt::Display for Candidate<'_> {
  /// `title 100 %, author 100 %, chapters 109/109`
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "title {}", percent(self.title))?;
    match self.author {
      Some(author) => write!(f, ", author {}", percent(author))?,
      None => write!(f, ", author not compared")?,
    }
    if let Some((found, total)) = self.chapters {
      write!(f, ", chapters {found}/{total}")?;
    }
    Ok(())
  }
}

pub fn percent(x: f64) -> String {
  format!("{:.0} %", x * 100.0)
}

/// Library books ranked against a device book's title and author (several
/// authors may be in one string, as Nickel stores them), best first. Left
/// out: those that stay below [`MIN_SCORE`] even if every chapter matches.
pub fn rank<'a>(
  books: &'a [LibraryBook],
  title: Option<&str>,
  author: Option<&str>,
) -> Vec<Candidate<'a>> {
  let title = words(title.unwrap_or(""));
  let author = words(author.unwrap_or(""));
  let mut candidates: Vec<Candidate> = books
    .iter()
    .map(|book| {
      let stem = book
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
      let by_title = dice(&title, &words(book.title.as_deref().unwrap_or("")));
      let by_name = dice(&title, &words(&stem));
      let theirs = words(&book.authors.join(" "));
      Candidate {
        book,
        title: by_title.max(by_name),
        author: (!author.is_empty() && !theirs.is_empty()).then(|| dice(&author, &theirs)),
        chapters: None,
      }
    })
    .filter(|c| 0.8 * c.metadata_score() + 0.2 >= MIN_SCORE)
    .collect();
  sort(&mut candidates);
  candidates
}

/// Best first; equal scores by path, so the order is stable.
pub fn sort(candidates: &mut [Candidate]) {
  candidates.sort_by(|a, b| {
    b.score()
      .total_cmp(&a.score())
      .then_with(|| a.book.path.cmp(&b.book.path))
  });
}

/// How many of the device file's spine items are in the EPUB's spine.
pub fn spine_overlap(device: &[String], epub: &[String]) -> (usize, usize) {
  let found = device.iter().filter(|item| epub.contains(item)).count();
  (found, device.len().max(epub.len()))
}

/// KFX sections against EPUB spine items: as many as match in number.
pub fn section_overlap(sections: usize, spine: usize) -> (usize, usize) {
  (sections.min(spine), sections.max(spine))
}

/// The set of folded, lower-case words.
pub fn words(s: &str) -> BTreeSet<String> {
  let mut folded = String::new();
  for c in s.chars().flat_map(char::to_lowercase) {
    match c {
      'ä' | 'à' | 'á' | 'â' | 'ã' | 'å' => folded.push('a'),
      'æ' => folded.push_str("ae"),
      'ç' => folded.push('c'),
      'è' | 'é' | 'ê' | 'ë' => folded.push('e'),
      'ì' | 'í' | 'î' | 'ï' => folded.push('i'),
      'ñ' => folded.push('n'),
      'ö' | 'ò' | 'ó' | 'ô' | 'õ' | 'ø' => folded.push('o'),
      'ß' => folded.push_str("ss"),
      'ù' | 'ú' | 'û' | 'ü' => folded.push('u'),
      'ý' | 'ÿ' => folded.push('y'),
      c if c.is_alphanumeric() => folded.push(c),
      _ => folded.push(' '),
    }
  }
  folded.split_whitespace().map(str::to_string).collect()
}

/// Twice the shared words over all words; 0 if either is empty.
fn dice(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
  if a.is_empty() || b.is_empty() {
    return 0.0;
  }
  2.0 * a.intersection(b).count() as f64 / (a.len() + b.len()) as f64
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn book(path: &str, title: &str, authors: &[&str]) -> LibraryBook {
    LibraryBook {
      path: PathBuf::from(path),
      title: Some(title.into()),
      authors: authors.iter().map(|a| a.to_string()).collect(),
      calibre_id: None,
    }
  }

  #[test]
  fn folds_words() {
    let w: Vec<_> = words("Größe – Über_Café, Vol. 2").into_iter().collect();
    assert_eq!(w, ["2", "cafe", "grosse", "uber", "vol"]);
  }

  #[test]
  fn ranks_by_title_and_author() {
    let books = [
      book(
        "/l/Flying Witch 2.epub",
        "Flying Witch 2",
        &["Chihiro Ishizuka"],
      ),
      book(
        "/l/Flying Witch 1.epub",
        "Flying Witch 1",
        &["Chihiro Ishizuka"],
      ),
      book("/l/Synners.epub", "Synners", &["Pat Cadigan"]),
    ];
    let ranked = rank(&books, Some("Flying Witch 1"), Some("Ishizuka, Chihiro"));
    // Synners: no shared word.
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].book.title.as_deref(), Some("Flying Witch 1"));
    assert_eq!(ranked[0].title, 1.0);
    assert_eq!(ranked[0].author, Some(1.0));
    assert!(ranked[1].score() < 0.8, "{}", ranked[1].score());
    assert_eq!(ranked[0].to_string(), "title 100 %, author 100 %");
  }

  #[test]
  fn kindle_title_is_the_file_stem() {
    let books = [book(
      "/l/Pat Cadigan/Synners (12)/Synners - Pat Cadigan.epub",
      "Synners: A Novel",
      &["Pat Cadigan"],
    )];
    let ranked = rank(&books, Some("Synners - Pat Cadigan"), Some("Pat Cadigan"));
    assert_eq!(ranked[0].title, 1.0);
  }

  #[test]
  fn chapters_weigh_in() {
    let books = [book("/l/a.epub", "Book A", &[])];
    let mut c = rank(&books, Some("Book A"), None).remove(0);
    assert_eq!(c.author, None);
    assert_eq!(c.score(), 1.0);
    c.chapters = Some(spine_overlap(&["a".into(), "b".into()], &["a".into()]));
    assert_eq!(c.chapters, Some((1, 2)));
    assert!((c.score() - 0.9).abs() < 1e-9);
    assert_eq!(section_overlap(5, 5), (5, 5));
  }
}
