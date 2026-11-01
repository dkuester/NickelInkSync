//! Access to the Nickel device database (`.kobo/KoboReader.sqlite`).
//!
//! Lists sideloaded books and reads their stored reading position. The
//! database is opened read-only unless a write is explicitly requested
//! ([`Device::write_positions`]), and then only after a backup copy.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

/// `ContentID` prefix of sideloaded books; store books have UUIDs instead.
pub const ONBOARD: &str = "file:///mnt/onboard/";

#[derive(Debug)]
pub enum Error {
  /// The directory has no `.kobo/KoboReader.sqlite`.
  NotADevice(PathBuf),
  Io(PathBuf, io::Error),
  Sqlite(rusqlite::Error),
  /// Writing was refused before anything was changed.
  Refused(String),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::NotADevice(root) => write!(
        f,
        "{}: no .kobo/KoboReader.sqlite, not a Kobo or Tolino (nor a Kindle: no documents \
         folder or .kfx files)",
        root.display()
      ),
      Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
      Error::Sqlite(e) => write!(f, "device database: {e}"),
      Error::Refused(e) => write!(f, "not written: {e}"),
    }
  }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
  fn from(e: rusqlite::Error) -> Self {
    Error::Sqlite(e)
  }
}

/// A Kobo or Tolino whose storage is mounted (USB) or a directory with the
/// same layout.
pub struct Device {
  pub root: PathBuf,
  /// `None` if `.kobo/version` is missing or unreadable.
  pub firmware: Option<Firmware>,
  pub db: DeviceDb,
  /// Opened while Nickel runs: never written.
  pub live: bool,
}

impl Device {
  /// Opens the device at `root`. The database is opened read-only, as a
  /// snapshot (see [`DeviceDb::open_snapshot`]) or, with `live`, while
  /// Nickel runs and writes it ([`DeviceDb::open_live`]).
  pub fn open(root: &Path, live: bool) -> Result<Self, Error> {
    let db_path = root.join(".kobo/KoboReader.sqlite");
    if !db_path.is_file() {
      return Err(Error::NotADevice(root.to_path_buf()));
    }
    let firmware = std::fs::read_to_string(root.join(".kobo/version"))
      .ok()
      .and_then(|line| Firmware::parse(&line));
    Ok(Device {
      root: root.to_path_buf(),
      firmware,
      db: if live {
        DeviceDb::open_live(&db_path)?
      } else {
        DeviceDb::open_snapshot(&db_path)?
      },
      live,
    })
  }

  /// Where the book file is on the mounted storage.
  pub fn book_file(&self, book: &Book) -> PathBuf {
    self.root.join(book.onboard_path())
  }

  pub fn db_path(&self) -> PathBuf {
    self.root.join(".kobo/KoboReader.sqlite")
  }

  /// Writes new positions for books, all in one transaction, after copying
  /// the database to `backup` (a file that must not exist yet). Only on a
  /// mounted device (a readable `.kobo/version`), and never with a rollback
  /// journal (`-journal`) next to the database: that means someone else is
  /// writing, or a write was cut off and SQLite has to roll it back first.
  /// Afterwards no journal may be left over.
  ///
  /// Since Tolino 5.18.279364, Nickel leaves its `-wal` and `-shm` files on
  /// the partition it hands over by USB (seen on 31.10.: 4 MB, also after
  /// restarts). Nickel no longer has the database open then, so SQLite reads
  /// the WAL as part of the database: the backup is made through SQLite and
  /// holds it, the write checkpoints it into the database file, and the then
  /// empty files are removed.
  ///
  /// Nickel keeps the database in WAL mode, and that stays as it is. The
  /// connection takes an exclusive lock first, so SQLite needs no `-shm`
  /// file, and after the commit a checkpoint moves the change from the
  /// `-wal` file into the database, so the device holds one file again.
  ///
  /// Returns how many of Nickel's cached pages it dropped (see
  /// [`drop_stale_pages`]); Nickel only lets go of them after a restart.
  pub fn write_positions(&self, writes: &[Write], backup: &Path) -> Result<usize, Error> {
    if self.live {
      return Err(Error::Refused(
        "the database is in use by Nickel; write only to a device mounted over USB".into(),
      ));
    }
    if self.firmware.is_none() {
      return Err(Error::Refused(format!(
        "{} has no readable .kobo/version, so it does not look like a mounted device",
        self.root.display()
      )));
    }
    let db_path = self.db_path();
    let journals = journal_files(&db_path);
    if let Some(journal) = journals
      .iter()
      .find(|j| j.to_string_lossy().ends_with("-journal"))
    {
      return Err(Error::Refused(format!(
        "{} exists; eject the device properly and mount it again",
        journal.display()
      )));
    }
    let left_wal = !journals.is_empty();
    if !left_wal {
      copy_backup(&db_path, backup)?;
    }
    let mut conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    // Before the first access, so WAL never sets up shared memory.
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
    if left_wal {
      backup_through_sqlite(&conn, backup)?;
    }
    let dropped = update_rows(&mut conn, writes)?;
    let wal: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if wal.eq_ignore_ascii_case("wal") {
      // (busy, frames in the WAL, frames checkpointed)
      let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
          Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
      if busy != 0 {
        return Err(Error::Io(
          db_path.clone(),
          io::Error::other("checkpoint did not finish; do not eject yet, run again"),
        ));
      }
    }
    conn.close().map_err(|(_, e)| e)?;
    if left_wal {
      remove_empty_wal(&db_path)?;
    }
    if let Some(journal) = journal_files(&db_path).into_iter().next() {
      return Err(Error::Io(
        journal,
        io::Error::other("left over after writing; do not eject yet, run again"),
      ));
    }
    Ok(dropped)
  }
}

impl Device {
  /// Puts the database back to the copy at `source` (a backup of this
  /// device's database), after a backup of the database as it is now to
  /// `backup`. Like [`Device::write_positions`] only on a mounted device and
  /// never with a rollback journal; `source` must pass SQLite's check and
  /// have the same `DbVersion`, so a backup from before a firmware update
  /// never goes back. Goes through SQLite's backup API, so a `-wal` Nickel
  /// left behind is folded into `backup` and cannot mix with the restored
  /// pages.
  pub fn restore(&self, source: &Path, backup: &Path) -> Result<(), Error> {
    if self.live {
      return Err(Error::Refused(
        "the database is in use by Nickel; restore only to a device mounted over USB".into(),
      ));
    }
    if self.firmware.is_none() {
      return Err(Error::Refused(format!(
        "{} has no readable .kobo/version, so it does not look like a mounted device",
        self.root.display()
      )));
    }
    let db_path = self.db_path();
    if let Some(journal) = journal_files(&db_path)
      .iter()
      .find(|j| j.to_string_lossy().ends_with("-journal"))
    {
      return Err(Error::Refused(format!(
        "{} exists; eject the device properly and mount it again",
        journal.display()
      )));
    }
    let copy = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let check: String = copy.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check != "ok" {
      return Err(Error::Refused(format!(
        "{} fails SQLite's check: {check}",
        source.display()
      )));
    }
    let theirs = DeviceDb::from_connection(copy).version()?;
    let ours = self.db.version()?;
    if theirs != ours {
      return Err(Error::Refused(format!(
        "{} has database version {theirs}, the device {ours} (a firmware update since): not restored",
        source.display()
      )));
    }
    let mut conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
    backup_through_sqlite(&conn, backup)?;
    conn.restore(
      rusqlite::MAIN_DB,
      source,
      None::<fn(rusqlite::backup::Progress)>,
    )?;
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if mode.eq_ignore_ascii_case("wal") {
      let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
          Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
      if busy != 0 {
        return Err(Error::Io(
          db_path.clone(),
          io::Error::other("checkpoint did not finish; do not eject yet, run again"),
        ));
      }
    }
    conn.close().map_err(|(_, e)| e)?;
    remove_empty_wal(&db_path)?;
    if let Some(journal) = journal_files(&db_path).into_iter().next() {
      return Err(Error::Io(
        journal,
        io::Error::other("left over after restoring; do not eject yet, run again"),
      ));
    }
    Ok(())
  }

  /// Writes new positions while Nickel runs and holds the database (the
  /// plugin, just before Nickel opens the book): Nickel reads the book row
  /// again when it opens a book, so a consistent row is what makes it open
  /// at the position. Unlike [`Device::write_positions`] this shares the
  /// database with Nickel through SQLite's normal locking, leaves its WAL
  /// files alone and keeps one backup per day in `backups` (the two newest
  /// stay), made with SQLite's backup API, which is consistent while Nickel
  /// writes. `today` names it (`20261024`). Returns the backup it made, if
  /// any, and how many cached pages it dropped.
  pub fn write_live(
    &self,
    writes: &[Write],
    backups: &Path,
    today: &str,
  ) -> Result<(Option<PathBuf>, usize), Error> {
    if !self.live {
      return Err(Error::Refused(
        "use write_positions for a device mounted over USB".into(),
      ));
    }
    let db_path = self.db_path();
    let mut conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let made = daily_backup(&conn, backups, today)?;
    let dropped = update_rows(&mut conn, writes)?;
    conn.close().map_err(|(_, e)| e)?;
    Ok((made, dropped))
  }

  /// The day's backup that [`Device::write_live`] makes, without writing:
  /// for the plugin on a Kobo, which writes the position through Nickel
  /// itself right after. Returns the backup if it made one.
  pub fn backup_live(&self, backups: &Path, today: &str) -> Result<Option<PathBuf>, Error> {
    if !self.live {
      return Err(Error::Refused(
        "only while Nickel runs (--live)".into(),
      ));
    }
    daily_backup(&self.db.conn, backups, today)
  }
}

/// Keeps one backup per day in `backups` (the two newest stay), made with
/// SQLite's backup API, which is consistent while Nickel writes. `today`
/// names it (`20261024`). Returns the backup if it made one.
fn daily_backup(conn: &Connection, backups: &Path, today: &str) -> Result<Option<PathBuf>, Error> {
  let backup = backups.join(format!("KoboReader-{today}.sqlite"));
  if backup.exists() {
    return Ok(None);
  }
  std::fs::create_dir_all(backups).map_err(|e| Error::Io(backups.to_path_buf(), e))?;
  let partial = backups.join(format!("KoboReader-{today}.sqlite.part"));
  conn.backup(rusqlite::MAIN_DB, &partial, None)?;
  std::fs::rename(&partial, &backup).map_err(|e| Error::Io(backup.clone(), e))?;
  prune_backups(backups, 2)?;
  Ok(Some(backup))
}

/// Sets the position of each book in one transaction, all or nothing, and
/// drops the cached pages that would make Nickel open elsewhere. Returns how
/// many pages it dropped.
fn update_rows(conn: &mut Connection, writes: &[Write]) -> Result<usize, Error> {
  let tx = conn.transaction()?;
  let mut dropped = 0;
  for write in writes {
    let changed = tx.execute(
      "UPDATE content SET ChapterIDBookmarked = ?1, ___PercentRead = ?2,
              DateLastRead = coalesce(?3, DateLastRead)
       WHERE ContentID = ?4 AND ContentType = '6'",
      rusqlite::params![
        write.position,
        write.percent,
        write.last_read,
        write.content_id
      ],
    )?;
    if changed != 1 {
      // Dropping the transaction rolls everything back.
      return Err(Error::Refused(format!(
        "{} is not a book in the database",
        write.content_id
      )));
    }
    dropped += drop_stale_pages(&tx, &write.content_id, &write.position)?;
  }
  tx.commit()?;
  Ok(dropped)
}

/// Since Kobo 6.0 and Tolino 5.18.279364 Nickel opens a book from its cache
/// of rendered pages (`CachePersistence*` tables, see
/// docs/nickel-5.18-symbols.md): if no cached page holds the bookmarked
/// place, it takes the first cached page of that chapter
/// (`getByChapterInPersistence` in the log), which is the chapter start in
/// practice. So when no cached page holds `position`, this drops the cached
/// pages of its chapter, and Nickel renders the page at the place. The other
/// chapters keep theirs. Returns how many pages it dropped; 0 without those
/// tables (older firmware).
fn drop_stale_pages(tx: &Connection, content_id: &str, position: &str) -> Result<usize, Error> {
  let chapter = stale_pages(tx, content_id, position)?;
  for id in &chapter {
    tx.execute(
      "DELETE FROM CachePersistenceRenderedPageChapterData WHERE cachePersistenceRenderedPageDataId = ?1",
      [id],
    )?;
    tx.execute(
      "DELETE FROM CachePersistenceRenderedPageData WHERE id = ?1",
      [id],
    )?;
  }
  Ok(chapter.len())
}

/// The ids (`CachePersistenceRenderedPageData.id`) of the cached pages
/// [`drop_stale_pages`] drops for `position`: those of its chapter if none
/// of them holds it, else none.
fn stale_pages(conn: &Connection, content_id: &str, position: &str) -> Result<Vec<String>, Error> {
  let has_cache: bool = conn.query_row(
    "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table'
                    AND name = 'CachePersistenceRenderedPageData')",
    [],
    |row| row.get(0),
  )?;
  let Some((file, target)) = split_location(position) else {
    return Ok(Vec::new());
  };
  if !has_cache {
    return Ok(Vec::new());
  }
  let mut stmt = conn.prepare(
    "SELECT id, pageStartLocation, pageEndLocation FROM CachePersistenceRenderedPageData
     WHERE contentId = ?1",
  )?;
  let mut chapter = Vec::new();
  for row in stmt.query_map([content_id], |row| {
    Ok((
      row.get::<_, String>(0)?,
      row.get::<_, String>(1)?,
      row.get::<_, String>(2)?,
    ))
  })? {
    let (id, start, end) = row?;
    let (Some((start_file, start)), Some((_, end))) =
      (split_location(&start), split_location(&end))
    else {
      continue;
    };
    if start_file != file {
      continue;
    }
    if start <= target && target <= end {
      // Nickel finds this page by the place itself.
      return Ok(Vec::new());
    }
    chapter.push(id);
  }
  Ok(chapter)
}

/// A location's chapter file and the numbers after it, to compare places in
/// one chapter: `OEBPS/text/c2.xhtml#point(/1/4/128/1:0)` gives
/// `OEBPS/text/c2.xhtml` and `[1, 4, 128, 1, 0]`, a kepub span
/// `…#kobo.17.1:204` gives `[17, 1, 204]`.
fn split_location(location: &str) -> Option<(&str, Vec<i64>)> {
  let (file, fragment) = location.split_once('#')?;
  let numbers: Vec<i64> = fragment
    .split(|c: char| !c.is_ascii_digit())
    .filter(|part| !part.is_empty())
    .map(|part| part.parse().ok())
    .collect::<Option<_>>()?;
  if numbers.is_empty() {
    return None;
  }
  Some((file, numbers))
}

/// Keeps the `keep` newest `KoboReader-*.sqlite` in `dir` (by name, which
/// starts with the date).
fn prune_backups(dir: &Path, keep: usize) -> Result<(), Error> {
  let io = |e| Error::Io(dir.to_path_buf(), e);
  let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
    .map_err(io)?
    .filter_map(|e| e.ok().map(|e| e.path()))
    .filter(|p| {
      p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("KoboReader-") && n.ends_with(".sqlite"))
    })
    .collect();
  names.sort();
  let old = names.len().saturating_sub(keep);
  for path in &names[..old] {
    std::fs::remove_file(path).map_err(|e| Error::Io(path.clone(), e))?;
  }
  Ok(())
}

/// A new position for one book.
#[derive(Debug, Clone, PartialEq)]
pub struct Write {
  pub content_id: String,
  /// For `ChapterIDBookmarked`.
  pub position: String,
  /// For `___PercentRead`.
  pub percent: i64,
  /// A new `DateLastRead` (ISO 8601), or `None` to keep it.
  pub last_read: Option<String>,
}

/// SQLite's journal files for `db`, if any exist.
fn journal_files(db: &Path) -> Vec<PathBuf> {
  ["-journal", "-wal", "-shm"]
    .iter()
    .map(|suffix| {
      let mut name = db.as_os_str().to_owned();
      name.push(suffix);
      PathBuf::from(name)
    })
    .filter(|p| p.exists())
    .collect()
}

/// Backs up the database as `conn` sees it, with what its WAL holds, through
/// SQLite's backup API, and checks the copy.
fn backup_through_sqlite(conn: &Connection, backup: &Path) -> Result<(), Error> {
  if backup.exists() {
    return Err(Error::Refused(format!(
      "{} already exists",
      backup.display()
    )));
  }
  if let Some(dir) = backup.parent() {
    std::fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
  }
  conn.backup(rusqlite::MAIN_DB, backup, None)?;
  // Read-write, so closing the last connection removes the `-wal` and
  // `-shm` SQLite makes for a copy in WAL mode: the backup is one file.
  let copy = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
  let check: String = copy.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
  copy.close().map_err(|(_, e)| e)?;
  if check != "ok" {
    return Err(Error::Refused(format!(
      "the backup {} fails SQLite's check: {check}",
      backup.display()
    )));
  }
  Ok(())
}

/// After a checkpoint with no connection left: removes a `-wal` that is
/// empty and the `-shm` next to it. A `-wal` that still holds frames stays.
fn remove_empty_wal(db: &Path) -> Result<(), Error> {
  let file = |suffix: &str| {
    let mut name = db.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
  };
  let (wal, shm) = (file("-wal"), file("-shm"));
  if wal.exists() {
    let len = std::fs::metadata(&wal)
      .map_err(|e| Error::Io(wal.clone(), e))?
      .len();
    if len != 0 {
      return Ok(());
    }
    std::fs::remove_file(&wal).map_err(|e| Error::Io(wal.clone(), e))?;
  }
  if shm.exists() {
    std::fs::remove_file(&shm).map_err(|e| Error::Io(shm.clone(), e))?;
  }
  Ok(())
}

/// Copies `db` to `backup` and checks the copy byte for byte.
fn copy_backup(db: &Path, backup: &Path) -> Result<(), Error> {
  if backup.exists() {
    return Err(Error::Refused(format!(
      "{} already exists",
      backup.display()
    )));
  }
  if let Some(dir) = backup.parent() {
    std::fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
  }
  std::fs::copy(db, backup).map_err(|e| Error::Io(backup.to_path_buf(), e))?;
  let read = |p: &Path| std::fs::read(p).map_err(|e| Error::Io(p.to_path_buf(), e));
  if read(db)? != read(backup)? {
    return Err(Error::Refused(format!(
      "the backup {} differs from the database",
      backup.display()
    )));
  }
  Ok(())
}

/// From `.kobo/version`: serial, kernel, firmware, …, model ID.
pub struct Firmware {
  pub version: String,
  pub model_id: String,
}

impl Firmware {
  /// The third field is the firmware version, the last one the model ID. The
  /// first field (the serial number) is not kept.
  pub fn parse(line: &str) -> Option<Self> {
    let fields: Vec<_> = line.trim().split(',').collect();
    if fields.len() < 4 {
      return None;
    }
    Some(Firmware {
      version: fields[2].to_string(),
      model_id: fields[fields.len() - 1].to_string(),
    })
  }

  /// Model name for the IDs seen on the reference devices.
  pub fn model_name(&self) -> Option<&'static str> {
    match self.model_id.rsplit('-').next()?.trim_start_matches('0') {
      "391" => Some("Kobo Clara BW"),
      "693" => Some("Tolino Shine Color"),
      _ => None,
    }
  }
}

/// A sideloaded book and its stored position.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
  pub content_id: String,
  pub title: Option<String>,
  pub author: Option<String>,
  pub mime_type: String,
  /// `ChapterIDBookmarked`: chapter path from the EPUB root, optionally with
  /// `#kobo.N.M` (kepub on a Kobo) or `#point(...)` (Adobe point). `None`
  /// if the book was never opened.
  pub position: Option<String>,
  /// `___PercentRead`, whole percent.
  pub percent_read: i64,
  /// `DateLastRead`, as stored (ISO 8601, UTC).
  pub last_read: Option<String>,
  /// `ReadStatus`: 0 unread, 1 reading, 2 finished.
  pub read_status: i64,
}

impl Book {
  /// Path relative to the root of the device storage.
  pub fn onboard_path(&self) -> &str {
    self
      .content_id
      .strip_prefix(ONBOARD)
      .unwrap_or(&self.content_id)
  }

  /// Nickel reads the file as a kepub. A Tolino reads a `.kepub.epub` as a
  /// plain EPUB and stores `application/epub+zip`.
  pub fn is_kepub(&self) -> bool {
    self.mime_type == "application/x-kobo-epub+zip"
  }
}

/// The opened `KoboReader.sqlite`.
pub struct DeviceDb {
  conn: Connection,
}

impl DeviceDb {
  /// Opens a database that nothing else writes while it is open: a copy, or
  /// the database of a device mounted over USB (Nickel is not running then).
  /// `immutable=1` makes SQLite read it without locking and without creating
  /// journal files next to it.
  pub fn open_snapshot(path: &Path) -> Result<Self, rusqlite::Error> {
    let conn = Connection::open_with_flags(
      format!("{}?immutable=1", file_uri(path)),
      OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    Ok(DeviceDb { conn })
  }

  /// Opens the database of the running device, read-only, while Nickel
  /// keeps writing it (the plugin). Nickel uses WAL mode, so its latest
  /// changes may still be in `-wal`: unlike [`DeviceDb::open_snapshot`] this
  /// reads them, through SQLite's normal locking.
  pub fn open_live(path: &Path) -> Result<Self, rusqlite::Error> {
    let conn = Connection::open_with_flags(
      format!("{}?mode=ro", file_uri(path)),
      OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(DeviceDb { conn })
  }

  /// The cached pages Nickel would open `content_id` from instead of at
  /// `position` (see [`drop_stale_pages`]), for the plugin to drop inside
  /// the transaction it writes the position in.
  pub fn stale_pages(&self, content_id: &str, position: &str) -> Result<Vec<String>, Error> {
    stale_pages(&self.conn, content_id, position)
  }

  /// Wraps an open connection, for tests with an in-memory database.
  pub fn from_connection(conn: Connection) -> Self {
    DeviceDb { conn }
  }

  /// `DbVersion`: 176 on Kobo 4.46, 220 on Tolino 5.18.
  pub fn version(&self) -> Result<i64, rusqlite::Error> {
    self
      .conn
      .query_row("SELECT version FROM DbVersion", [], |row| row.get(0))
  }

  /// Tolinos have extra `tolino_*` tables (cloud, DRM, Skoobe).
  pub fn is_tolino(&self) -> Result<bool, rusqlite::Error> {
    self.conn.query_row(
      "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name LIKE 'tolino\\_%' ESCAPE '\\')",
      [],
      |row| row.get(0),
    )
  }

  /// Books with a `ContentID` under `file:///mnt/onboard/`, by path.
  pub fn sideloaded_books(&self) -> Result<Vec<Book>, rusqlite::Error> {
    let mut stmt = self.conn.prepare(
      "SELECT ContentID, Title, Attribution, MimeType, ChapterIDBookmarked,
              ___PercentRead, DateLastRead, ReadStatus
       FROM content
       WHERE ContentType = '6' AND ContentID LIKE ?1 || '%'
       ORDER BY ContentID",
    )?;
    let books = stmt.query_map([ONBOARD], |row| {
      Ok(Book {
        content_id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        mime_type: row.get(3)?,
        position: row.get::<_, Option<String>>(4)?.filter(|p| !p.is_empty()),
        percent_read: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
        last_read: row.get(6)?,
        read_status: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
      })
    })?;
    books.collect()
  }
}

/// `file:` URI for SQLite, with the characters that have a meaning in a URI
/// percent-encoded.
fn file_uri(path: &Path) -> String {
  let mut uri = String::from("file:");
  for b in path.to_string_lossy().bytes() {
    match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
        uri.push(b as char)
      }
      _ => uri.push_str(&format!("%{b:02X}")),
    }
  }
  uri
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn firmware_from_version_line() {
    let fw = Firmware::parse(
      "N000000000000,4.9.77,5.18.264769,4.9.77,4.9.77,00000000-0000-0000-0000-000000000693\n",
    )
    .unwrap();
    assert_eq!(fw.version, "5.18.264769");
    assert_eq!(fw.model_name(), Some("Tolino Shine Color"));
    assert!(Firmware::parse("garbage").is_none());
  }

  #[test]
  fn uri_escapes_spaces_and_question_marks() {
    assert_eq!(
      file_uri(Path::new("/Volumes/My Kobo/a?b#c.sqlite")),
      "file:/Volumes/My%20Kobo/a%3Fb%23c.sqlite"
    );
  }

  /// A kepub's cached pages start and end at spans, in the form of
  /// `ChapterIDBookmarked`: a span with an offset compares inside them.
  #[test]
  fn kepub_spans_against_cached_pages() {
    let conn = Connection::open_in_memory().unwrap();
    conn
      .execute_batch(
        "CREATE TABLE CachePersistenceRenderedPageData (id TEXT, pageStartLocation TEXT,
           pageEndLocation TEXT, contentId TEXT);
         CREATE TABLE CachePersistenceRenderedPageChapterData
           (cachePersistenceRenderedPageDataId TEXT);
         INSERT INTO CachePersistenceRenderedPageData VALUES
           ('a', 'c2.xhtml#kobo.1.1', 'c2.xhtml#kobo.3.1:120', 'book'),
           ('b', 'c2.xhtml#kobo.3.1:121', 'c2.xhtml#kobo.6.1:15', 'book'),
           ('c', 'c3.xhtml#kobo.1.1', 'c3.xhtml#kobo.4.1:7', 'book');",
      )
      .unwrap();
    // In page b: nothing goes.
    assert_eq!(
      drop_stale_pages(&conn, "book", "c2.xhtml#kobo.6.1:3").unwrap(),
      0
    );
    assert_eq!(
      drop_stale_pages(&conn, "book", "c2.xhtml#kobo.3.1").unwrap(),
      0
    );
    // Behind both pages of c2: they go, c3 stays.
    assert_eq!(
      drop_stale_pages(&conn, "book", "c2.xhtml#kobo.64.1:13").unwrap(),
      2
    );
    let left: Vec<String> = conn
      .prepare("SELECT id FROM CachePersistenceRenderedPageData")
      .unwrap()
      .query_map([], |r| r.get(0))
      .unwrap()
      .collect::<Result<_, _>>()
      .unwrap();
    assert_eq!(left, ["c"]);
  }
}
