use std::error::Error;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use nickelinksync::config::{self, Config};
use nickelinksync::db::{Book, Device, Write};
use nickelinksync::epub::Epub;
use nickelinksync::hash::partial_md5;
use nickelinksync::kindle_device::{self, KindleBook};
use nickelinksync::kosync::{self, Client, Progress, Update};
use nickelinksync::mapping::{DeviceBook, EpubState, Mapping, Original, Store};
use nickelinksync::not_in_line::NotInLine;
use nickelinksync::position::{
  from_xpointer, to_xpointer, PointFlavor, Position, Translated, XPointer,
};
use nickelinksync::sync::{
  self, decide, format_timestamp, iso_timestamp, parse_timestamp, Confirm, Decision, Direction,
  Local, Side,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Sync the reading progress of sideloaded ebooks between Kobo, Tolino and
/// KOReader through a KOSync server.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// Show the position of each sideloaded book on the device and the server
  Status(Target),
  /// Send the device position to the KOSync server
  Push {
    #[command(flatten)]
    target: Target,
    /// Send the device position without asking, also if the server's is newer
    #[arg(long)]
    force: bool,
    /// Only show what would be sent
    #[arg(long)]
    dry_run: bool,
    /// For the plugin: print the outcome as lines `NICKELINKSYNC_SENT<tab>
    /// percent`, `NICKELINKSYNC_IN_SYNC`, `NICKELINKSYNC_NOT_SENT<tab>reason`,
    /// `NICKELINKSYNC_UNANSWERED<tab>n` and `NICKELINKSYNC_ERROR<tab>kind<tab>
    /// message`
    #[arg(long)]
    emit: bool,
  },
  /// Show what a newer position on the KOSync server would change, and with
  /// --write write it to the device
  Pull {
    #[command(flatten)]
    target: Target,
    /// Take the server position without asking, also if the device's is newer
    #[arg(long)]
    force: bool,
    /// Write the positions into the device database (Tolino only so far),
    /// after a backup copy of it
    #[arg(long)]
    write: bool,
    /// For the plugin: print each position to take as one line
    /// `NICKELINKSYNC_POSITION<tab>percent<tab>ChapterIDBookmarked` followed
    /// by `NICKELINKSYNC_FROM<tab>device`, `NICKELINKSYNC_UNANSWERED<tab>n`
    /// if positions need an answer, `NICKELINKSYNC_NOT_TAKEN<tab>reason<tab>
    /// message` for one that cannot be taken, and `NICKELINKSYNC_ERROR<tab>
    /// kind<tab>message` if the server could not be asked
    #[arg(long)]
    emit: bool,
  },
  /// Link books on a device to their original EPUB, so they sync under its
  /// checksum (kepub, a differently converted EPUB, a Kindle KFX)
  Map {
    #[command(subcommand)]
    command: MapCommand,
  },
}

#[derive(Subcommand)]
enum MapCommand {
  /// List every mapping and check its EPUB, also broken ones
  List,
  /// Map one book on the device to its original EPUB
  Add {
    /// Mount point of the device (Kobo, Tolino or Kindle)
    #[arg(long)]
    device: PathBuf,
    /// The book whose title or file path contains this text
    #[arg(long)]
    book: Option<String>,
    /// The book with exactly this ContentID (Kobo, Tolino) or content_id
    /// (Kindle)
    #[arg(long)]
    content_id: Option<String>,
    /// The original EPUB, the file KOReader opens
    #[arg(long)]
    epub: PathBuf,
  },
  /// Remove a mapping, given by its key (or the start of it) from `map list`
  Remove { key: String },
}

#[derive(Args)]
struct Target {
  /// Mount point of the device, e.g. /Volumes/KOBOeReader (/mnt/onboard on
  /// the device itself)
  #[arg(long)]
  device: PathBuf,
  /// Configuration file [default: ~/.config/nickelinksync/config.ini]
  #[arg(long)]
  config: Option<PathBuf>,
  /// Only books whose title or file path contains this text
  #[arg(long)]
  book: Option<String>,
  /// Only the book with exactly this ContentID (file:///mnt/onboard/…)
  #[arg(long)]
  content_id: Option<String>,
  /// Nickel is running (on the device): read its database as it is being
  /// written, never write it
  #[arg(long)]
  live: bool,
  /// Seconds each request to the server may take
  #[arg(long, default_value_t = 30)]
  timeout: u64,
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  let (result, emit) = match cli.command {
    Command::Status(target) => (status(&target), false),
    Command::Push {
      target,
      force,
      dry_run,
      emit,
    } => (push(&target, force, dry_run, emit), emit),
    Command::Pull {
      target,
      force,
      write,
      emit,
    } => (pull(&target, force, write, emit), emit),
    Command::Map { command } => (map(command), false),
  };
  match result {
    Ok(true) => ExitCode::SUCCESS,
    Ok(false) => ExitCode::FAILURE,
    Err(e) => {
      if emit {
        emit_line("ERROR", &["other", &e.to_string()]);
      }
      eprintln!("nickelinksync: {e}");
      ExitCode::FAILURE
    }
  }
}

/// For the plugin: one line `NICKELINKSYNC_<name>` with tab-separated
/// fields; tabs and line breaks inside a field become spaces.
fn emit_line(name: &str, fields: &[&str]) {
  let mut line = format!("NICKELINKSYNC_{name}");
  for field in fields {
    line.push('\t');
    line.extend(field.chars().map(|c| if c.is_control() { ' ' } else { c }));
  }
  println!("{line}");
}

/// For the plugin: the server could not be asked.
fn emit_error(emit: bool, e: &kosync::Error) {
  if emit {
    emit_line("ERROR", &[e.kind(), &e.to_string()]);
  }
}

/// The opened device and its sideloaded books, with the header printed.
struct Session {
  device: Device,
  /// Model name, or "Kobo" / "Tolino".
  model: String,
  flavor: PointFlavor,
  books: Vec<Book>,
}

impl Session {
  fn open(target: &Target) -> Result<Self> {
    let device = Device::open(&target.device, target.live)?;
    let family = if device.db.is_tolino()? {
      "Tolino"
    } else {
      "Kobo"
    };
    let model = device
      .firmware
      .as_ref()
      .and_then(|fw| fw.model_name())
      .unwrap_or(family)
      .to_string();
    match &device.firmware {
      Some(fw) => println!(
        "{model}, firmware {} (database version {})",
        fw.version,
        device.db.version()?
      ),
      None => println!("{model}, database version {}", device.db.version()?),
    }
    // How Nickel counts nodes in an Adobe point depends on the firmware line.
    let flavor = match &device.firmware {
      Some(fw) => PointFlavor::for_firmware(&fw.version),
      None if family == "Tolino" => PointFlavor::AllNodes,
      None => PointFlavor::JoinText,
    };
    let books = select(
      device.db.sideloaded_books()?,
      target.book.as_deref(),
      target.content_id.as_deref(),
    );
    if books.is_empty() {
      println!("\nNo sideloaded books.");
    }
    Ok(Session {
      device,
      model,
      flavor,
      books,
    })
  }

  fn hash(&self, book: &Book) -> std::result::Result<String, String> {
    partial_md5(&self.device.book_file(book)).map_err(|e| format!("file not readable: {e}"))
  }

  /// The stored position as a KOReader XPointer.
  fn xpointer(&self, book: &Book, stored: &str) -> std::result::Result<XPointer, String> {
    Epub::open(&self.device.book_file(book))
      .map_err(|e| e.to_string())
      .and_then(|mut epub| {
        to_xpointer(&mut epub, &Position::parse(stored), self.flavor).map_err(|e| e.to_string())
      })
  }

  /// A server XPointer as the device would store it.
  fn translate(&self, book: &Book, xpointer: &str) -> std::result::Result<Translated, String> {
    Epub::open(&self.device.book_file(book))
      .map_err(|e| e.to_string())
      .and_then(|mut epub| {
        from_xpointer(&mut epub, xpointer, self.flavor).map_err(|e| e.to_string())
      })
  }

  /// The device position for KOSync, or why there is none.
  fn local(&self, book: &Book) -> std::result::Result<(Local, bool), String> {
    let stored = book.position.as_deref().ok_or("never opened")?;
    let x = self
      .xpointer(book, stored)
      .map_err(|e| format!("not translated: {e}"))?;
    let local = Local {
      xpointer: x.xpointer,
      percentage: book.percent_read as f64 / 100.0,
      last_read: book.last_read.as_deref().and_then(parse_timestamp),
    };
    Ok((local, x.exact))
  }
}

/// The books whose title or path contains `filter` (any case), and the one
/// with exactly `content_id`.
fn select(mut books: Vec<Book>, filter: Option<&str>, content_id: Option<&str>) -> Vec<Book> {
  if let Some(filter) = filter {
    let filter = filter.to_lowercase();
    books.retain(|b| {
      b.onboard_path().to_lowercase().contains(&filter)
        || b
          .title
          .as_deref()
          .is_some_and(|t| t.to_lowercase().contains(&filter))
    });
  }
  if let Some(id) = content_id {
    books.retain(|b| b.content_id == id);
  }
  books
}

fn heading(book: &Book) {
  println!();
  let title = book.title.as_deref().unwrap_or("(no title)");
  match &book.author {
    Some(author) => println!("{title} — {author}"),
    None => println!("{title}"),
  }
}

/// The configuration and a client for its server.
struct Server {
  config: Config,
  client: Client,
}

impl Server {
  /// With `--config` the file must exist; the default file may be missing,
  /// then `None`.
  fn open(target: &Target) -> Result<Option<Self>> {
    let path = target.config.as_deref();
    let (path, required) = match path {
      Some(path) => (path.to_path_buf(), true),
      None => match Config::default_path() {
        Some(path) => (path, false),
        None => return Ok(None),
      },
    };
    if !required && !path.exists() {
      return Ok(None);
    }
    let config = Config::load(&path)?;
    // On the device's FAT storage every file looks world-readable.
    if !target.live {
      if let Some(warning) = config::permission_warning(&path) {
        eprintln!("nickelinksync: warning: {warning}");
      }
    }
    let timeout = std::time::Duration::from_secs(target.timeout.max(1));
    let client = Client::with_timeout(&config.server, &config.user, &config.key, timeout);
    Ok(Some(Server { config, client }))
  }

  /// Like [`Server::open`], but a configuration is required.
  fn require(target: &Target) -> Result<Self> {
    Server::open(target)?.ok_or_else(|| {
      let path = Config::default_path().unwrap_or_else(|| "config.ini".into());
      format!(
        "no configuration at {}; see README.md for the format",
        path.display()
      )
      .into()
    })
  }
}

fn percent(p: f64) -> String {
  format!("{:.0} %", p * 100.0)
}

fn describe(p: &Progress) -> String {
  format!(
    "{} ({}), from {} at {}",
    p.progress,
    percent(p.percentage),
    if p.device.is_empty() {
      "(unknown device)"
    } else {
      &p.device
    },
    format_timestamp(p.timestamp)
  )
}

fn status(target: &Target) -> Result<bool> {
  let session = Session::open(target)?;
  let not_in_line = NotInLine::load(&session.device.root)?;
  let server = Server::open(target)?;
  match &server {
    Some(s) => println!("Server {} as {}", s.config.server, s.config.user),
    None => println!("Server not configured"),
  }
  // After the first failed request the server is not asked again.
  let mut server_error: Option<String> = None;
  for book in &session.books {
    heading(book);
    let format = if book.is_kepub() { "kepub" } else { "EPUB" };
    println!("  file      {} ({format})", book.onboard_path());
    let hash = session.hash(book);
    println!(
      "  hash      {}",
      hash.as_deref().unwrap_or_else(|e| e.as_str())
    );
    println!(
      "  position  {}",
      book.position.as_deref().unwrap_or("(never opened)")
    );
    let local = session.local(book);
    if let Some(stored) = &book.position {
      let koreader = match session.xpointer(book, stored) {
        Ok(x) if x.exact => x.xpointer,
        Ok(x) => format!("{} (chapter only)", x.xpointer),
        Err(e) => format!("(not translated: {e})"),
      };
      println!("  koreader  {koreader}");
    }
    if let Some(at) = not_in_line.get(&book.content_id) {
      println!("  plugin    not in line: Nickel opened at {at} instead of the server's position");
    }
    let state = match book.read_status {
      2 => "finished",
      1 => "reading",
      _ => "unread",
    };
    match &book.last_read {
      Some(date) => println!(
        "  progress  {} %, {state}, last read {date}",
        book.percent_read
      ),
      None => println!("  progress  {} %, {state}", book.percent_read),
    }
    let (Some(server), Ok(hash)) = (&server, &hash) else {
      continue;
    };
    if let Some(e) = &server_error {
      println!("  server    (not checked: {e})");
      continue;
    }
    match server.client.progress(hash) {
      Ok(remote) => {
        match &remote {
          Some(p) => println!("  server    {}", describe(p)),
          None => println!("  server    (nothing stored)"),
        }
        if let Ok((local, _)) = &local {
          println!(
            "  sync      {}",
            sync_note(local, remote.as_ref(), server.config.confirm)
          );
        }
      }
      Err(e) => {
        println!("  server    (failed: {e})");
        server_error = Some(e.to_string());
      }
    }
  }
  Ok(true)
}

/// What `push` and `pull` would do with a book, for `status`.
fn sync_note(local: &Local, remote: Option<&Progress>, confirm: Confirm) -> String {
  let Some(remote) = remote else {
    return "device is newer: push sends it".into();
  };
  let push = decide(Direction::Push, local, remote, confirm);
  let pull = decide(Direction::Pull, local, remote, confirm);
  // Otherwise one of them asks: the one moving to the newer position.
  let (newer, other, command, ask) = match (push, pull) {
    (Decision::InSync, _) => return "in sync".into(),
    (Decision::Apply, _) => return "device is newer: push sends it".into(),
    (_, Decision::Apply) => return "server is newer: pull would move the device".into(),
    (Decision::Ask { newer: true, .. }, _) => ("device", "server", "push", push),
    _ => ("server", "device", "pull", pull),
  };
  if matches!(
    ask,
    Decision::Ask {
      backwards: true,
      ..
    }
  ) {
    format!("{newer} is newer, but behind the {other}: {command} asks first")
  } else {
    format!("{newer} is newer: {command} asks first (confirm = {confirm})")
  }
}

/// Shows why a book needs an answer and gets one: from the terminal, or,
/// in a dry run or without a terminal, none. `true` if `direction` should
/// overwrite the other side.
fn confirm(
  direction: Direction,
  local: &Local,
  remote: &Progress,
  (backwards, confirm): (bool, Confirm),
  dry_run: bool,
  unanswered: &mut usize,
) -> Result<bool> {
  match local.last_read {
    Some(t) => println!(
      "  device    {} ({}), last read {}",
      local.xpointer,
      percent(local.percentage),
      format_timestamp(t)
    ),
    None => println!(
      "  device    {} ({})",
      local.xpointer,
      percent(local.percentage)
    ),
  }
  println!("  server    {}", describe(remote));
  let (new, old) = match direction {
    Direction::Push => ("device", "server"),
    Direction::Pull => ("server", "device"),
  };
  if backwards {
    println!("  ask       the {new} position lies before the {old}'s (confirm = {confirm})");
  } else {
    println!("  ask       the positions differ (confirm = {confirm})");
  }
  if dry_run {
    println!("            would ask first");
    return Ok(false);
  }
  if !std::io::stdin().is_terminal() {
    println!("  skipped   needs an answer, but there is no terminal; --force overwrites anyway");
    *unanswered += 1;
    return Ok(false);
  }
  let side = sync::ask(&mut std::io::stdin().lock(), &mut std::io::stdout())?;
  let apply = match (side, direction) {
    (None, _) => {
      println!("  skipped   no answer");
      return Ok(false);
    }
    (Some(Side::Device), Direction::Push) | (Some(Side::Server), Direction::Pull) => true,
    (Some(_), _) => false,
  };
  if !apply {
    println!("  kept      the {old} position");
  }
  Ok(apply)
}

fn unanswered_note(unanswered: usize) {
  if unanswered > 0 {
    println!("\n{unanswered} book(s) need an answer: run again in a terminal, or with --force.");
  }
}

fn push(target: &Target, force: bool, dry_run: bool, emit: bool) -> Result<bool> {
  let session = Session::open(target)?;
  let mut not_in_line = NotInLine::load(&session.device.root)?;
  let mut forced = Vec::new();
  let server = Server::require(target)?;
  let (device, device_id) = server.config.identity(&session.model);
  if let Err(e) = server.client.authorize() {
    emit_error(emit, &e);
    eprintln!("nickelinksync: {}: {e}", server.config.server);
    return Ok(false);
  }
  println!(
    "Server {} as {}, device \"{device}\"",
    server.config.server, server.config.user
  );
  let confirm_setting = server.config.confirm;
  let mut unanswered = 0;
  let mut ok = true;
  for book in &session.books {
    heading(book);
    let (hash, (local, exact)) = match (session.hash(book), session.local(book)) {
      (Ok(hash), Ok(local)) => (hash, local),
      (Err(e), _) | (_, Err(e)) => {
        println!("  skipped   {e}");
        if emit {
          emit_line("NOT_SENT", &["skipped", &e]);
        }
        continue;
      }
    };
    println!("  hash      {hash}");
    if let Some(at) = not_in_line.get(&book.content_id) {
      if !force {
        println!("  skipped   not in line: Nickel opened at {at} instead of the server's position");
        println!("            pull takes the server's position; --force sends the device's anyway");
        if emit {
          emit_line("NOT_SENT", &["not-in-line", at]);
        }
        continue;
      }
      forced.push(book.content_id.as_str());
    }
    let remote = match server.client.progress(&hash) {
      Ok(remote) => remote,
      Err(e) => {
        println!("  failed    {e}");
        emit_error(emit, &e);
        ok = false;
        continue;
      }
    };
    if let Some(remote) = &remote {
      match decide(Direction::Push, &local, remote, confirm_setting) {
        Decision::InSync => {
          println!("  in sync   {}", describe(remote));
          if emit {
            emit_line("IN_SYNC", &[]);
          }
          continue;
        }
        Decision::Apply => {}
        _ if force => {}
        Decision::Keep => {
          println!("  skipped   server is newer: {}", describe(remote));
          println!("            --force sends the device position anyway");
          if emit {
            emit_line("NOT_SENT", &["server-newer", &describe(remote)]);
          }
          continue;
        }
        Decision::Ask { backwards, .. } => {
          let answer = (backwards, confirm_setting);
          if !confirm(
            Direction::Push,
            &local,
            remote,
            answer,
            dry_run,
            &mut unanswered,
          )? {
            continue;
          }
        }
      }
    }
    let update = Update {
      document: hash,
      progress: local.xpointer,
      percentage: local.percentage,
      device: device.clone(),
      device_id: device_id.clone(),
    };
    let what = format!(
      "{}{} ({})",
      update.progress,
      if exact { "" } else { " (chapter only)" },
      percent(update.percentage)
    );
    if dry_run {
      println!("  would send {what}");
      continue;
    }
    match server.client.update(&update) {
      Ok(t) => {
        match t {
          Some(t) => println!("  sent      {what} at {}", format_timestamp(t)),
          None => println!("  sent      {what}"),
        }
        if emit {
          let percent = (update.percentage * 100.0).round().clamp(0.0, 100.0);
          emit_line("SENT", &[&percent.to_string()]);
        }
      }
      Err(e) => {
        println!("  failed    {e}");
        emit_error(emit, &e);
        ok = false;
        if matches!(e, kosync::Error::Unauthorized) {
          break;
        }
      }
    }
  }
  unanswered_note(unanswered);
  if emit && unanswered > 0 {
    emit_line("UNANSWERED", &[&unanswered.to_string()]);
  }
  if dry_run {
    println!("\nDry run: nothing was sent.");
  } else if ok {
    // Sent with --force: the server holds the device's position now.
    not_in_line.remove(forced)?;
  }
  Ok(ok)
}

fn pull(target: &Target, force: bool, write: bool, emit: bool) -> Result<bool> {
  let session = Session::open(target)?;
  let mut not_in_line = NotInLine::load(&session.device.root)?;
  let server = Server::require(target)?;
  println!("Server {} as {}", server.config.server, server.config.user);
  let confirm_setting = server.config.confirm;
  let mut unanswered = 0;
  let mut writes = Vec::new();
  let mut ok = true;
  for book in &session.books {
    heading(book);
    let hash = match session.hash(book) {
      Ok(hash) => hash,
      Err(e) => {
        println!("  skipped   {e}");
        if emit {
          emit_line("NOT_TAKEN", &["skipped", &e]);
        }
        continue;
      }
    };
    let remote = match server.client.progress(&hash) {
      Ok(Some(remote)) => remote,
      Ok(None) => {
        println!("  server    (nothing stored)");
        continue;
      }
      Err(e) => {
        println!("  failed    {e}");
        emit_error(emit, &e);
        ok = false;
        if matches!(e, kosync::Error::Unauthorized) {
          break;
        }
        continue;
      }
    };
    println!(
      "  stored    {}",
      book.position.as_deref().unwrap_or("(never opened)")
    );
    // Without a device position there is nothing on the device to keep.
    let local = session.local(book).ok().map(|(local, _)| local);
    // After a failed take Nickel's DateLastRead says nothing about reading.
    let left_by_nickel = not_in_line.get(&book.content_id);
    let decision = match (&local, left_by_nickel) {
      (Some(_), Some(at)) if book.position.as_deref() == Some(at) => {
        println!(
          "  plugin    Nickel opened here instead of the server's position, nothing read since"
        );
        Decision::Apply
      }
      (Some(local), Some(_)) => {
        println!(
          "  plugin    read on since Nickel missed the server's position: the time does not count"
        );
        let undated = Local {
          last_read: None,
          ..local.clone()
        };
        decide(Direction::Pull, &undated, &remote, confirm_setting)
      }
      (Some(local), None) => decide(Direction::Pull, local, &remote, confirm_setting),
      (None, _) => Decision::Apply,
    };
    let apply = match (decision, &local) {
      (Decision::InSync, _) => {
        println!("  in sync   {}", describe(&remote));
        false
      }
      (Decision::Apply, _) => true,
      (_, Some(_)) if force => true,
      (Decision::Keep, _) => {
        println!("  keep      device is newer than {}", describe(&remote));
        println!("            --force takes the server position anyway");
        false
      }
      (Decision::Ask { backwards, .. }, Some(local)) => {
        let answer = (backwards, confirm_setting);
        confirm(
          Direction::Pull,
          local,
          &remote,
          answer,
          !write,
          &mut unanswered,
        )?
      }
      (Decision::Ask { .. }, None) => unreachable!("asks only with a device position"),
    };
    if !apply {
      continue;
    }
    let verb = if write { "move to  " } else { "would move to" };
    println!("  {verb} {}", describe(&remote));
    let translated = match session.translate(book, &remote.progress) {
      Ok(t) => t,
      Err(e) => {
        println!("  skipped   not translated: {e}");
        if emit {
          emit_line("NOT_TAKEN", &["not-translated", &e]);
        }
        continue;
      }
    };
    let only = if translated.exact {
      ""
    } else {
      " (chapter only)"
    };
    println!("  on device {}{only}", translated.position);
    // Keep the server the newer side, so the next push does not send the
    // old device position back: DateLastRead stays as it is unless it is
    // newer than the server's position.
    let last_read = local
      .and_then(|l| l.last_read)
      .filter(|&t| t >= remote.timestamp)
      .map(|_| iso_timestamp(remote.timestamp - 1));
    let percent = (remote.percentage * 100.0).round().clamp(0.0, 100.0) as i64;
    if emit {
      let position = translated.position.to_string();
      emit_line("POSITION", &[&percent.to_string(), &position]);
      emit_line("FROM", &[&remote.device]);
    }
    writes.push(Write {
      content_id: book.content_id.clone(),
      position: translated.position.to_string(),
      percent,
      last_read,
    });
  }
  unanswered_note(unanswered);
  if emit && unanswered > 0 {
    // For the plugin: a position needs an answer it cannot give.
    emit_line("UNANSWERED", &[&unanswered.to_string()]);
  }
  if !write && target.live {
    println!("\nNothing was written to the device database (Nickel is running).");
  } else if !write {
    println!("\nDry run: nothing was written to the device; --write writes it.");
  } else if writes.is_empty() {
    println!("\nNothing to write to the device.");
  } else {
    let dir = Config::backup_dir().ok_or("no directory for the backup (HOME is not set)")?;
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)?
      .as_secs() as i64;
    let stamp: String = iso_timestamp(now)
      .chars()
      .filter(|c| !"-:".contains(*c))
      .collect();
    if target.live {
      // Only the one book the plugin is about to open.
      if target.content_id.is_none() {
        return Err("--write with --live needs --content-id".into());
      }
      let made = session.device.write_live(&writes, &dir, &stamp[..8])?;
      not_in_line.remove(writes.iter().map(|w| w.content_id.as_str()))?;
      println!(
        "\nWrote {} position(s) to the device database while Nickel runs.",
        writes.len()
      );
      if let Some(backup) = made {
        println!("Backup of the database (one a day): {}", backup.display());
      }
      return Ok(ok);
    }
    let backup = dir.join(format!("KoboReader-{stamp}.sqlite"));
    session.device.write_positions(&writes, &backup)?;
    not_in_line.remove(writes.iter().map(|w| w.content_id.as_str()))?;
    println!("\nWrote {} position(s) to the device.", writes.len());
    println!("Backup of the database: {}", backup.display());
    println!("Eject the device before you open the book.");
  }
  Ok(ok)
}

fn map(command: MapCommand) -> Result<bool> {
  let path = Store::default_path().ok_or("no data directory (HOME is not set)")?;
  let mut store = Store::load(&path)?;
  match command {
    MapCommand::List => {
      map_list(&store);
      Ok(true)
    }
    MapCommand::Add {
      device,
      book,
      content_id,
      epub,
    } => map_add(
      &mut store,
      &device,
      book.as_deref(),
      content_id.as_deref(),
      &epub,
    ),
    MapCommand::Remove { key } => {
      let removed = store.remove(&key)?;
      store.save()?;
      println!(
        "Removed the mapping of {} to {}.",
        device_label(&removed.device),
        removed.epub.path
      );
      Ok(true)
    }
  }
}

/// Title and author of a device book, or its path.
fn device_label(book: &DeviceBook) -> String {
  let title = match (book.title(), book) {
    (Some(title), _) => title.to_string(),
    (None, DeviceBook::File { path, .. }) => path.clone(),
    (None, DeviceBook::Kindle { content_id, .. }) => content_id.clone(),
  };
  match book.author() {
    Some(author) => format!("{title} — {author}"),
    None => title,
  }
}

fn map_list(store: &Store) {
  println!("Mappings in {}", store.path.display());
  if store.mappings.is_empty() {
    println!("\nNo mappings yet; `nickelinksync map add` makes one.");
    return;
  }
  let mut broken = 0;
  for m in &store.mappings {
    println!("\n{}", device_label(&m.device));
    match &m.device {
      DeviceBook::File { hash, path, .. } => {
        println!("  device    {path} (Kobo/Tolino, checksum {hash})")
      }
      DeviceBook::Kindle { content_id, .. } => {
        println!("  device    Kindle, content_id {content_id}")
      }
    }
    println!("  epub      {} (checksum {})", m.epub.path, m.epub.hash);
    if let Some(title) = &m.epub.title {
      match &m.epub.author {
        Some(author) => println!("            {title} — {author}"),
        None => println!("            {title}"),
      }
    }
    let state = m.check();
    if state != EpubState::Ok {
      broken += 1;
    }
    println!("  state     {state}");
    println!("  made      {}", m.created);
  }
  if broken > 0 {
    println!(
      "\n{broken} broken mapping(s): these books are not synced. Map them again with \
       `map add` (it replaces the old mapping) or remove them with `map remove <checksum or content_id>`."
    );
  }
}

fn map_add(
  store: &mut Store,
  device: &std::path::Path,
  filter: Option<&str>,
  content_id: Option<&str>,
  epub_path: &std::path::Path,
) -> Result<bool> {
  let epub_path = epub_path
    .canonicalize()
    .map_err(|e| format!("{}: {e}", epub_path.display()))?;
  let epub = Epub::open(&epub_path).map_err(|e| format!("{}: {e}", epub_path.display()))?;
  let epub_hash = partial_md5(&epub_path)?;
  let original = Original {
    path: epub_path
      .to_str()
      .ok_or_else(|| format!("{}: path is not UTF-8", epub_path.display()))?
      .to_string(),
    hash: epub_hash.clone(),
    title: epub.title.clone(),
    author: (!epub.authors.is_empty()).then(|| epub.authors.join(", ")),
  };

  let (book, device_hash, chapters) = if kindle_device::is_kindle(device) {
    let (books, _) = kindle_device::books(device)?;
    let book = one_kindle_book(device, books, filter, content_id)?;
    let sections = nickelinksync::kfx::Kfx::open(&book.kfx)?.sections.len();
    let chapters = if sections == epub.spine.len() {
      format!("{sections} sections, as many as spine items in the EPUB")
    } else {
      format!(
        "{sections} sections, but {} spine items in the EPUB: positions may land in the wrong chapter",
        epub.spine.len()
      )
    };
    let book = DeviceBook::Kindle {
      content_id: book.content_id,
      title: book.title,
      author: book.author,
    };
    (book, None, chapters)
  } else {
    let opened = Device::open(device, false)?;
    let book = one_nickel_book(select(opened.db.sideloaded_books()?, filter, content_id))?;
    let file = opened.book_file(&book);
    let hash = partial_md5(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let chapters = match Epub::open(&file) {
      Ok(on_device) => compare_spines(&on_device.spine, &epub.spine),
      Err(e) => format!("not compared: {e}"),
    };
    let book = DeviceBook::File {
      hash: hash.clone(),
      path: book.onboard_path().to_string(),
      title: book.title.clone(),
      author: book.author.clone(),
    };
    (book, Some(hash), chapters)
  };
  println!("Device    {}", device_label(&book));
  println!("EPUB      {} (checksum {epub_hash})", original.path);
  if device_hash.as_deref() == Some(epub_hash.as_str()) {
    println!(
      "\nSame file as the EPUB (checksum {epub_hash}): it already syncs under that checksum, \
       no mapping needed."
    );
    if let Some(old) = store.get(&book).cloned() {
      store.remove(old.device.key())?;
      store.save()?;
      println!("Removed the old mapping to {}.", old.epub.path);
    }
    return Ok(true);
  }
  println!("chapters  {chapters}");
  let mapping = Mapping {
    device: book,
    epub: original,
    created: iso_timestamp(
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64,
    ),
  };
  let replaced = store.insert(mapping);
  store.save()?;
  if let Some(old) = replaced {
    println!("\nMapped; this replaces the mapping to {}.", old.epub.path);
  } else {
    println!("\nMapped.");
  }
  println!("Saved in {}", store.path.display());
  Ok(true)
}

/// How many chapters (spine items) of the device file the EPUB has, by
/// path: positions translate by chapter.
fn compare_spines(device: &[String], epub: &[String]) -> String {
  let same = device.iter().filter(|c| epub.contains(c)).count();
  if same == device.len() && device.len() == epub.len() {
    format!("all {same} spine items match")
  } else if same == 0 {
    format!(
      "none of the {} spine items is in the EPUB ({} there): is it the same book? Positions \
       fall back to the percentage",
      device.len(),
      epub.len()
    )
  } else {
    format!(
      "{same} of {} spine items found in the EPUB ({} there): positions elsewhere fall back \
       to the percentage",
      device.len(),
      epub.len()
    )
  }
}

/// Exactly one book, or an error that lists the candidates.
fn one_nickel_book(books: Vec<Book>) -> Result<Book> {
  match books.len() {
    1 => Ok(books.into_iter().next().unwrap()),
    0 => Err("no sideloaded book matches; see `nickelinksync status --device …`".into()),
    n => {
      let mut e =
        format!("{n} books match, choose one with --book <part of the file name> or --content-id:");
      for b in &books {
        e.push_str(&format!(
          "\n  {} ({})",
          b.title.as_deref().unwrap_or("(no title)"),
          b.content_id
        ));
      }
      Err(e.into())
    }
  }
}

fn one_kindle_book(
  root: &std::path::Path,
  mut books: Vec<KindleBook>,
  filter: Option<&str>,
  content_id: Option<&str>,
) -> Result<KindleBook> {
  if let Some(filter) = filter {
    let filter = filter.to_lowercase();
    books.retain(|b| {
      b.relative_path(root)
        .to_string_lossy()
        .to_lowercase()
        .contains(&filter)
        || [&b.title, &b.author].iter().any(|t| {
          t.as_deref()
            .is_some_and(|t| t.to_lowercase().contains(&filter))
        })
    });
  }
  if let Some(id) = content_id {
    books.retain(|b| b.content_id.eq_ignore_ascii_case(id));
  }
  match books.len() {
    1 => Ok(books.into_iter().next().unwrap()),
    0 => Err("no KFX book on the Kindle matches".into()),
    n => {
      let mut e = format!(
        "{n} books match, choose one with --book <part of the title or file name> or --content-id:"
      );
      for b in &books {
        e.push_str(&format!(
          "\n  {} ({})",
          b.title.as_deref().unwrap_or("(no title)"),
          b.content_id
        ));
      }
      Err(e.into())
    }
  }
}
