use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use nickelinksync::db::Book;
use nickelinksync::history;
use nickelinksync::kindle_device::{self, KindleBook};
use nickelinksync::kobalt;
use nickelinksync::kosync::{self, Client, Progress, Update};
use nickelinksync::library::{self, Library, LibraryBook};
use nickelinksync::map::{self, candidates, DeviceChapters, OnDevice, Outcome};
use nickelinksync::mapping::{DeviceBook, EpubState, Store};
use nickelinksync::session::{
  self, Action, Identity, KindleSession, Options, Plugin, PullOutcome, PushOutcome, Server,
  Session, Written,
};
use nickelinksync::suggest::{self, Candidate};
use nickelinksync::sync::{
  self, format_timestamp, outlook, Confirm, Direction, Local, Outlook, Side,
};

type Result<T> = session::Result<T>;

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
  /// List the backups of a device database that `pull --write` made, and
  /// with --backup put one back (after a backup of the database as it is)
  Restore {
    /// Mount point of the device (Kobo or Tolino)
    #[arg(long)]
    device: PathBuf,
    /// The backup to put back, as the list shows it
    #[arg(long)]
    backup: Option<PathBuf>,
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
    /// The original EPUB, the file KOReader opens; without it, the best
    /// suggestion from the library, after asking
    #[arg(long)]
    epub: Option<PathBuf>,
    #[command(flatten)]
    library: LibraryArgs,
  },
  /// Suggest an original EPUB from the library for each book on the device
  /// that is neither mapped nor the same file as one in the library
  Suggest {
    /// Mount point of the device (Kobo, Tolino or Kindle)
    #[arg(long)]
    device: PathBuf,
    /// Only books whose title or file path contains this text
    #[arg(long)]
    book: Option<String>,
    /// Only the book with exactly this ContentID (Kobo, Tolino) or
    /// content_id (Kindle)
    #[arg(long)]
    content_id: Option<String>,
    #[command(flatten)]
    library: LibraryArgs,
  },
  /// Remove a mapping, given by its key (or the start of it) from `map list`
  Remove { key: String },
}

#[derive(Args)]
struct LibraryArgs {
  /// Folder with the original EPUBs: a Calibre library (read-only) or any
  /// folder [default: `library` in the configuration file]
  #[arg(long)]
  library: Option<PathBuf>,
  /// Configuration file [default: ~/.config/nickelinksync/config.ini]
  #[arg(long)]
  config: Option<PathBuf>,
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
  /// Folder with the original EPUBs (Calibre library or any folder): books
  /// that are neither mapped nor the same file as one there are not synced
  /// [default: `library` in the configuration file]
  #[arg(long)]
  library: Option<PathBuf>,
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
    Command::Restore { device, backup } => (restore(&device, backup.as_deref()), false),
  };
  match result {
    Ok(true) => ExitCode::SUCCESS,
    Ok(false) => ExitCode::FAILURE,
    Err(e) => {
      if emit {
        let kind = match e.downcast_ref::<kobalt::Active>() {
          Some(_) => "kobalt",
          None => "other",
        };
        emit_line("ERROR", &[kind, &e.to_string()]);
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

impl Target {
  fn options(&self) -> Options {
    Options {
      device: self.device.clone(),
      config: self.config.clone(),
      book: self.book.clone(),
      content_id: self.content_id.clone(),
      live: self.live,
      timeout: Duration::from_secs(self.timeout),
      library: self.library.clone(),
    }
  }
}

/// Opens the device and prints its header.
fn open_session(options: &Options) -> Result<Session> {
  let session = Session::open(options)?;
  let model = &session.model;
  match &session.device.firmware {
    Some(fw) => println!(
      "{model}, firmware {} (database version {})",
      fw.version,
      session.device.db.version()?
    ),
    None => println!("{model}, database version {}", session.device.db.version()?),
  }
  if let Some(library) = &session.library {
    print_library(library);
  }
  if let Some(note) = session.kobalt.note() {
    let level = if session.kobalt.blocks() {
      "warning: "
    } else {
      ""
    };
    eprintln!("nickelinksync: {level}{note}");
  }
  if !session.store.mappings.is_empty() {
    println!(
      "Mappings  {} in {}",
      session.store.mappings.len(),
      session.store.path.display()
    );
  }
  if session.books.is_empty() {
    println!("\nNo sideloaded books.");
  }
  Ok(session)
}

/// [`Server::open`], with its warning printed.
fn open_server(options: &Options) -> Result<Option<Server>> {
  let server = Server::open(options)?;
  if let Some(warning) = server.as_ref().and_then(|s| s.warning.as_ref()) {
    eprintln!("nickelinksync: warning: {warning}");
  }
  Ok(server)
}

/// [`Server::require`], with its warning printed.
fn require_server(options: &Options) -> Result<Server> {
  let server = Server::require(options)?;
  if let Some(warning) = &server.warning {
    eprintln!("nickelinksync: warning: {warning}");
  }
  Ok(server)
}

/// What `status`, `push` and `pull` print about a book's identity.
fn describe_identity(identity: &Identity) -> String {
  match identity {
    Identity::Own { why, .. } => format!("own checksum ({why})"),
    Identity::Mapped { mapping, .. } => {
      format!(
        "mapped to {} (checksum {})",
        mapping.epub.path, mapping.epub.hash
      )
    }
    Identity::Skip { message, .. } => message.clone(),
  }
}

fn heading(book: &Book) {
  println!();
  let title = book.title.as_deref().unwrap_or("(no title)");
  match &book.author {
    Some(author) => println!("{title} — {author}"),
    None => println!("{title}"),
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
  let options = target.options();
  if kindle_device::is_kindle(&options.device) {
    return kindle_status(&options);
  }
  let session = open_session(&options)?;
  let server = open_server(&options)?;
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
    let identity = session.identity(book);
    if let Some(hash) = identity.device_hash() {
      println!("  hash      {hash}");
    }
    println!("  sync as   {}", describe_identity(&identity));
    println!(
      "  position  {}",
      book.position.as_deref().unwrap_or("(never opened)")
    );
    let local = session.local(book, &identity);
    if book.position.is_some() {
      let koreader = match &local {
        Ok((l, None)) => l.xpointer.clone(),
        Ok((l, Some(note))) => format!("{} ({note})", l.xpointer),
        Err(e) => format!("({e})"),
      };
      println!("  koreader  {koreader}");
    }
    if let Some(at) = session.not_in_line.get(&book.content_id) {
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
    let (Some(server), Some(hash)) = (&server, identity.document()) else {
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
          let local = session.against(book, &identity, local.clone(), remote.as_ref());
          println!(
            "  sync      {}",
            sync_note(&local, remote.as_ref(), server.config.confirm)
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
  match outlook(local, remote, confirm) {
    Outlook::InSync => "in sync".into(),
    Outlook::Push => "device is newer: push sends it".into(),
    Outlook::Pull => "server is newer: pull would move the device".into(),
    Outlook::Asks {
      direction,
      backwards,
    } => {
      let (newer, other, command) = match direction {
        Direction::Push => ("device", "server", "push"),
        Direction::Pull => ("server", "device", "pull"),
      };
      if backwards {
        format!("{newer} is newer, but behind the {other}: {command} asks first")
      } else {
        format!("{newer} is newer: {command} asks first (confirm = {confirm})")
      }
    }
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

/// Sends `update` and says so; the server's time, if it gave one.
fn send(
  client: &Client,
  update: &Update,
  note: Option<String>,
  emit: bool,
) -> std::result::Result<Option<i64>, kosync::Error> {
  let what = sent_what(update, note);
  match client.update(update) {
    Ok(t) => {
      match t {
        Some(t) => println!("  sent      {what} at {}", format_timestamp(t)),
        None => println!("  sent      {what}"),
      }
      if emit {
        let percent = (update.percentage * 100.0).round().clamp(0.0, 100.0);
        emit_line("SENT", &[&percent.to_string()]);
      }
      Ok(t)
    }
    Err(e) => {
      println!("  failed    {e}");
      emit_error(emit, &e);
      Err(e)
    }
  }
}

fn push(target: &Target, force: bool, dry_run: bool, emit: bool) -> Result<bool> {
  let options = target.options();
  if kindle_device::is_kindle(&options.device) {
    return kindle_push(&options, force, dry_run);
  }
  let mut session = open_session(&options)?;
  if session.kobalt.blocks() {
    // Sending touches only the server; Kobalt's own KOSync may send too.
    eprintln!("nickelinksync: warning: sending anyway; if Kobalt syncs with KOSync itself, it may send these books as well");
  }
  let mut forced = Vec::new();
  let server = require_server(&options)?;
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
    let step = session.push_step(book, &server.client, confirm_setting, force);
    if let PushOutcome::Skip { kind, message } = &step.outcome {
      println!("  skipped   {message}");
      if emit {
        emit_line("NOT_SENT", &[kind, message]);
      }
      continue;
    }
    if let Identity::Mapped { .. } = step.identity {
      println!("  sync as   {}", describe_identity(&step.identity));
    }
    let hash = step.identity.document().unwrap_or_default();
    println!("  hash      {hash}");
    let (local, note, remote, action) = match step.outcome {
      PushOutcome::NotInLine { at } => {
        println!("  skipped   not in line: Nickel opened at {at} instead of the server's position");
        println!("            pull takes the server's position; --force sends the device's anyway");
        if emit {
          emit_line("NOT_SENT", &["not-in-line", &at]);
        }
        continue;
      }
      PushOutcome::Failed(e) => {
        println!("  failed    {e}");
        emit_error(emit, &e);
        ok = false;
        continue;
      }
      PushOutcome::Ready {
        local,
        note,
        remote,
        action,
        forced: was_forced,
      } => {
        if was_forced {
          forced.push(book.content_id.as_str());
        }
        (local, note, remote, action)
      }
      PushOutcome::Skip { .. } => unreachable!("handled above"),
    };
    let remote = remote.as_ref();
    match action {
      Action::InSync => {
        println!("  in sync   {}", describe(remote.expect("both hold it")));
        if emit {
          emit_line("IN_SYNC", &[]);
        }
        continue;
      }
      Action::Apply => {}
      Action::Keep => {
        let remote = remote.expect("only a server position is kept");
        println!("  skipped   server is newer: {}", describe(remote));
        println!("            --force sends the device position anyway");
        if emit {
          emit_line("NOT_SENT", &["server-newer", &describe(remote)]);
        }
        continue;
      }
      Action::Ask { backwards } => {
        let remote = remote.expect("asks only about a server position");
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
    let update = session::update(hash, &local, &device, &device_id);
    if dry_run {
      println!("  would send {}", sent_what(&update, note));
      continue;
    }
    match send(&server.client, &update, note, emit) {
      // The plugin's pushes on the device are not kept.
      Ok(t) if !options.live => record(&[sent_entry(&update, t, book.title.clone())]),
      Ok(_) => {}
      Err(e) => {
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
    session.not_in_line.remove(forced)?;
  }
  Ok(ok)
}

/// A position sent, for the history.
fn sent_entry(update: &Update, time: Option<i64>, title: Option<String>) -> history::Entry {
  history::Entry {
    time: time.unwrap_or_else(history::now),
    direction: history::Direction::Push,
    device: update.device.clone(),
    title,
    document: update.document.clone(),
    position: update.progress.clone(),
    percentage: update.percentage,
    by: "cli".into(),
  }
}

/// Adds to the sync history; a failure is only reported.
fn record(entries: &[history::Entry]) {
  if let Err(e) = history::record(entries) {
    eprintln!("nickelinksync: history: {e}");
  }
}

/// The position `update` sends, with what was not exact about it.
fn sent_what(update: &Update, note: Option<String>) -> String {
  format!(
    "{}{} ({})",
    update.progress,
    note.map(|n| format!(" ({n})")).unwrap_or_default(),
    percent(update.percentage)
  )
}

fn pull(target: &Target, force: bool, write: bool, emit: bool) -> Result<bool> {
  let options = target.options();
  if kindle_device::is_kindle(&options.device) {
    return Err(
      "a Kindle is only read: its position goes to the server with `push`, nothing comes back"
        .into(),
    );
  }
  let mut session = open_session(&options)?;
  if write {
    session.kobalt.check()?;
  }
  let server = require_server(&options)?;
  println!("Server {} as {}", server.config.server, server.config.user);
  let confirm_setting = server.config.confirm;
  let mut unanswered = 0;
  let mut writes = Vec::new();
  // For the history, once written.
  let mut taken = Vec::new();
  let mut ok = true;
  for book in &session.books {
    heading(book);
    let step = session.pull_step(book, &server.client, confirm_setting, force);
    if let PullOutcome::Skip { kind, message } = &step.outcome {
      println!("  skipped   {message}");
      if emit {
        emit_line("NOT_TAKEN", &[kind, message]);
      }
      continue;
    }
    if let Identity::Mapped { .. } = step.identity {
      println!("  sync as   {}", describe_identity(&step.identity));
    }
    let (remote, local, plugin, action) = match step.outcome {
      PullOutcome::NothingStored => {
        println!("  server    (nothing stored)");
        continue;
      }
      PullOutcome::Failed(e) => {
        println!("  failed    {e}");
        emit_error(emit, &e);
        ok = false;
        if matches!(e, kosync::Error::Unauthorized) {
          break;
        }
        continue;
      }
      PullOutcome::Ready {
        remote,
        local,
        plugin,
        action,
      } => (remote, local, plugin, action),
      PullOutcome::Skip { .. } => unreachable!("handled above"),
    };
    println!(
      "  stored    {}",
      book.position.as_deref().unwrap_or("(never opened)")
    );
    match plugin {
      Some(Plugin::NothingReadSince) => println!(
        "  plugin    Nickel opened here instead of the server's position, nothing read since"
      ),
      Some(Plugin::ReadOnSince) => println!(
        "  plugin    read on since Nickel missed the server's position: the time does not count"
      ),
      None => {}
    }
    let apply = match action {
      Action::InSync => {
        println!("  in sync   {}", describe(&remote));
        false
      }
      Action::Apply => true,
      Action::Keep => {
        println!("  keep      device is newer than {}", describe(&remote));
        println!("            --force takes the server position anyway");
        false
      }
      Action::Ask { backwards } => {
        let local = local.as_ref().expect("asks only with a device position");
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
    };
    if !apply {
      continue;
    }
    let verb = if write { "move to  " } else { "would move to" };
    println!("  {verb} {}", describe(&remote));
    let take = match session.take(book, &step.identity, &remote, local.as_ref()) {
      Ok(take) => take,
      Err(e) => {
        println!("  skipped   not translated: {e}");
        if emit {
          emit_line("NOT_TAKEN", &["not-translated", &e]);
        }
        continue;
      }
    };
    let note = take.note.map(|n| format!(" ({n})")).unwrap_or_default();
    println!("  on device {}{note}", take.write.position);
    if emit {
      emit_line(
        "POSITION",
        &[&take.write.percent.to_string(), &take.write.position],
      );
      emit_line("FROM", &[&remote.device]);
    }
    taken.push(history::Entry {
      time: history::now(),
      direction: history::Direction::Pull,
      device: session.model.clone(),
      title: book.title.clone(),
      document: step.identity.document().unwrap_or_default().to_string(),
      position: take.write.position.clone(),
      percentage: remote.percentage,
      by: "cli".into(),
    });
    writes.push(take.write);
  }
  unanswered_note(unanswered);
  if emit && unanswered > 0 {
    // For the plugin: a position needs an answer it cannot give.
    emit_line("UNANSWERED", &[&unanswered.to_string()]);
  }
  if !write && options.live {
    println!("\nNothing was written to the device database (Nickel is running).");
  } else if !write {
    println!("\nDry run: nothing was written to the device; --write writes it.");
  } else if writes.is_empty() {
    println!("\nNothing to write to the device.");
  } else {
    let written = session.write(&writes)?;
    if !options.live {
      record(&taken);
    }
    match written {
      Written::Live {
        backup,
        pages_dropped,
      } => {
        println!(
          "\nWrote {} position(s) to the device database while Nickel runs.",
          writes.len()
        );
        if pages_dropped > 0 {
          println!("Dropped {pages_dropped} of Nickel's cached pages that hid the place.");
        }
        if let Some(backup) = backup {
          println!("Backup of the database (one a day): {}", backup.display());
        }
      }
      Written::Mounted {
        backup,
        pages_dropped,
      } => {
        println!("\nWrote {} position(s) to the device.", writes.len());
        println!("Backup of the database: {}", backup.display());
        if pages_dropped > 0 {
          println!(
            "Dropped {pages_dropped} of Nickel's cached pages that hid the place. Eject the \
             device and restart it before you open the book: Nickel keeps those pages in \
             memory until then."
          );
        } else {
          println!("Eject the device before you open the book.");
        }
      }
    }
  }
  Ok(ok)
}

fn restore(device: &Path, backup: Option<&Path>) -> Result<bool> {
  let Some(source) = backup else {
    let backups = nickelinksync::restore::list(device)?;
    if backups.is_empty() {
      println!("No backups of this device's database.");
      return Ok(true);
    }
    println!(
      "Backups of the database on {} (newest first):",
      device.display()
    );
    for b in &backups {
      let origin = match b.origin {
        nickelinksync::restore::Origin::ThisDevice => "",
        nickelinksync::restore::Origin::Unknown => {
          "  (device unknown: made before backups named it)"
        }
      };
      println!(
        "  {}  {}{origin}",
        format_timestamp(b.time),
        b.path.display()
      );
    }
    println!("\n--backup PATH puts one back.");
    return Ok(true);
  };
  let before = nickelinksync::restore::restore(device, source)?;
  println!("Restored {}.", source.display());
  println!("Backup of the database before: {}", before.display());
  println!("Eject the device and restart it before you open a book.");
  Ok(true)
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
      library,
    } => map_add(
      &mut store,
      &device,
      book.as_deref(),
      content_id.as_deref(),
      epub.as_deref(),
      &library,
    ),
    MapCommand::Suggest {
      device,
      book,
      content_id,
      library,
    } => map_suggest(
      &store,
      &device,
      book.as_deref(),
      content_id.as_deref(),
      &library,
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

fn library_label(book: &LibraryBook) -> String {
  let title = book.title.as_deref().unwrap_or("(no title)");
  match book.authors.as_slice() {
    [] => title.to_string(),
    authors => format!("{title} — {}", authors.join(", ")),
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

/// For `map add`: how far positions will translate by chapter, against
/// the EPUB's spine.
fn describe_chapters(chapters: &DeviceChapters, epub_spine: &[String]) -> String {
  let spine = epub_spine.len();
  match (chapters, chapters.overlap(epub_spine)) {
    (DeviceChapters::Unknown(e), _) => format!("not compared: {e}"),
    (DeviceChapters::Sections(n), _) if *n == spine => {
      format!("{n} sections, as many as spine items in the EPUB")
    }
    (DeviceChapters::Sections(n), _) => format!(
      "{n} sections, {spine} spine items in the EPUB (Send-to-Kindle adds a cover and splits \
       chapters): positions are found by their text"
    ),
    (DeviceChapters::Spine(own), Some((found, _))) if found == own.len() && own.len() == spine => {
      format!("all {found} spine items match")
    }
    (DeviceChapters::Spine(own), Some((0, _))) => format!(
      "none of the {} spine items is in the EPUB ({spine} there): is it the same book? \
       Positions fall back to the percentage",
      own.len()
    ),
    (DeviceChapters::Spine(own), Some((found, _))) => format!(
      "{found} of {} spine items found in the EPUB ({spine} there): positions elsewhere fall \
       back to the percentage",
      own.len()
    ),
    (DeviceChapters::Spine(_), None) => unreachable!("a spine always compares"),
  }
}

/// [`map::device_books`], saying which KFX files could not be read.
fn device_books(
  root: &std::path::Path,
  filter: Option<&str>,
  content_id: Option<&str>,
) -> Result<Vec<OnDevice>> {
  let (books, unreadable) = map::device_books(root, filter, content_id)?;
  for (path, e) in unreadable {
    eprintln!("nickelinksync: skipped {}: {e}", path.display());
  }
  Ok(books)
}

/// Exactly one book, or an error that lists the candidates.
fn one_book(books: Vec<OnDevice>) -> Result<OnDevice> {
  match books.len() {
    1 => Ok(books.into_iter().next().unwrap()),
    0 => Err("no book on the device matches".into()),
    n => {
      let mut e = format!(
        "{n} books match, choose one with --book <part of the title or file name> or --content-id:"
      );
      for b in &books {
        e.push_str(&format!("\n  {} ({})", device_label(&b.book), b.id));
      }
      Err(e.into())
    }
  }
}

/// The library from `--library`, else from `library` in the configuration
/// file, and says so; `None` if neither is set.
fn open_library(args: &LibraryArgs) -> Result<Option<Library>> {
  let library = map::open_library(args.library.as_deref(), args.config.as_deref())?;
  if let Some(library) = &library {
    print_library(library);
  }
  Ok(library)
}

/// Says which library is used, and what in it was skipped.
fn print_library(library: &Library) {
  let kind = match library.kind {
    library::Kind::Calibre => "Calibre library",
    library::Kind::Folder => "folder",
  };
  println!(
    "Library   {} ({kind}, {} EPUBs)",
    library.root.display(),
    library.books.len()
  );
  for (path, e) in &library.skipped {
    eprintln!("nickelinksync: library: skipped {}: {e}", path.display());
  }
}

/// A shell word in single quotes.
fn quote(s: &str) -> String {
  format!("'{}'", s.replace('\'', r"'\''"))
}

fn print_candidates(candidates: &[Candidate], shown: usize) {
  for (i, c) in candidates.iter().take(shown).enumerate() {
    println!(
      "  {:<9} {:>5}  {}",
      if i == 0 { "suggest" } else { "" },
      format!("{}.", i + 1),
      library_label(c.book)
    );
    println!("                   {}", c.book.path.display());
    println!(
      "                   score {}: {c}",
      suggest::percent(c.score())
    );
  }
}

fn map_suggest(
  store: &Store,
  device: &std::path::Path,
  filter: Option<&str>,
  content_id: Option<&str>,
  args: &LibraryArgs,
) -> Result<bool> {
  let library = open_library(args)?.ok_or(
    "no library: give --library <folder>, or set `library = <folder>` in the configuration file",
  )?;
  let books = device_books(device, filter, content_id)?;
  if books.is_empty() {
    println!("\nNo book on the device matches.");
  }
  let (mut mapped, mut same_file, mut suggested, mut none) = (0, 0, 0, 0);
  for on in &books {
    println!("\n{}", device_label(&on.book));
    match &on.book {
      DeviceBook::File { path, hash, .. } => println!("  device    {path} (checksum {hash})"),
      DeviceBook::Kindle { content_id, .. } => {
        println!("  device    Kindle, content_id {content_id}")
      }
    }
    if let Some(m) = store.get(&on.book) {
      let state = m.check();
      println!("  mapped    {} ({state})", m.epub.path);
      if state == EpubState::Ok {
        mapped += 1;
        continue;
      }
    }
    let (candidates, same) = candidates(on, &library);
    if let Some(same) = same {
      println!(
        "  same file {}: syncs under its checksum, no mapping needed",
        same.path.display()
      );
      same_file += 1;
      continue;
    }
    if candidates.is_empty() {
      println!("  not mapped: no candidate in the library");
      none += 1;
      continue;
    }
    suggested += 1;
    print_candidates(&candidates, 3);
    let best = &candidates[0];
    println!(
      "  {}  nickelinksync map add --device {} --content-id {} --epub {}",
      if best.score() >= suggest::LIKELY {
        "map with"
      } else {
        "uncertain: check first, then"
      },
      quote(&device.to_string_lossy()),
      quote(&on.id),
      quote(&best.book.path.to_string_lossy())
    );
  }
  println!(
    "\n{} book(s): {mapped} mapped, {same_file} the same file as in the library, \
     {suggested} not mapped with a suggestion, {none} not mapped without one.",
    books.len()
  );
  if suggested + none > 0 {
    println!(
      "Books that are not mapped are not synced while a library is set up (`library` or \
       --library), and a Kindle book never."
    );
  }
  Ok(true)
}

fn map_add(
  store: &mut Store,
  device: &std::path::Path,
  filter: Option<&str>,
  content_id: Option<&str>,
  epub_path: Option<&std::path::Path>,
  args: &LibraryArgs,
) -> Result<bool> {
  let on = one_book(device_books(device, filter, content_id)?)?;
  let epub_path = match epub_path {
    Some(path) => path.to_path_buf(),
    None => {
      let library = open_library(args)?.ok_or(
        "give --epub <file>, or a library (--library or `library` in the configuration file) \
         to take the best suggestion from",
      )?;
      println!("Device    {}", device_label(&on.book));
      let (candidates, same) = candidates(&on, &library);
      if let Some(same) = same {
        println!(
          "\nSame file as {} in the library: it already syncs under its checksum, no mapping \
           needed.",
          same.path.display()
        );
        return Ok(true);
      }
      if candidates.is_empty() {
        return Err("no candidate in the library; give --epub <file>".into());
      }
      print_candidates(&candidates, 3);
      if !std::io::stdin().is_terminal() {
        return Err("no terminal to confirm a suggestion; give --epub <file>".into());
      }
      let shown = candidates.len().min(3);
      print!("\nMap to which one? [1-{shown}, Enter: none] ");
      std::io::Write::flush(&mut std::io::stdout())?;
      let mut answer = String::new();
      std::io::stdin().read_line(&mut answer)?;
      match answer.trim().parse::<usize>() {
        Ok(n) if (1..=shown).contains(&n) => candidates[n - 1].book.path.clone(),
        _ => {
          println!("Nothing mapped.");
          return Ok(true);
        }
      }
    }
  };
  add_mapping(store, on, &epub_path)
}

fn add_mapping(store: &mut Store, on: OnDevice, epub_path: &std::path::Path) -> Result<bool> {
  let added = map::add(store, &on, epub_path)?;
  let original = &added.original;
  println!("Device    {}", device_label(&on.book));
  println!("EPUB      {} (checksum {})", original.path, original.hash);
  match added.outcome {
    Outcome::SameFile { removed } => {
      println!(
        "\nSame file as the EPUB (checksum {}): it already syncs under that checksum, \
         no mapping needed.",
        original.hash
      );
      if let Some(old) = removed {
        println!("Removed the old mapping to {}.", old.epub.path);
      }
    }
    Outcome::Mapped { replaced } => {
      println!(
        "chapters  {}",
        describe_chapters(&on.chapters, &added.spine)
      );
      if let Some(old) = replaced {
        println!("\nMapped; this replaces the mapping to {}.", old.epub.path);
      } else {
        println!("\nMapped.");
      }
      println!("Saved in {}", store.path.display());
    }
  }
  Ok(true)
}

/// Opens the Kindle (or the copy of its books) and prints its header.
fn open_kindle(options: &Options) -> Result<KindleSession> {
  let session = KindleSession::open(options)?;
  println!(
    "Kindle, {} KFX book(s) in {}",
    session.found,
    kindle_device::documents(&session.root).display()
  );
  for (path, e) in &session.unreadable {
    println!("  skipped   {}: {e}", path.display());
  }
  if session.books.is_empty() {
    println!("\nNo KFX books.");
  }
  Ok(session)
}

fn kindle_heading(root: &std::path::Path, book: &KindleBook) {
  println!();
  let title = book.title.as_deref().unwrap_or("(no title)");
  match &book.author {
    Some(author) => println!("{title} — {author}"),
    None => println!("{title}"),
  }
  println!("  file      {}", book.relative_path(root).display());
  println!("  content   {}", book.content_id);
}

fn kindle_status(options: &Options) -> Result<bool> {
  let session = open_kindle(options)?;
  let server = open_server(options)?;
  match &server {
    Some(s) => println!("Server {} as {}", s.config.server, s.config.user),
    None => println!("Server not configured"),
  }
  let mut server_error: Option<String> = None;
  for book in &session.books {
    kindle_heading(&session.root, book);
    let mapping = match session.mapping(book) {
      Ok(mapping) => mapping,
      Err(e) => {
        println!("  sync as   {e}");
        continue;
      }
    };
    println!(
      "  sync as   mapped to {} (checksum {})",
      mapping.epub.path, mapping.epub.hash
    );
    let local = match session.local(book, mapping) {
      Ok((local, note, shown)) => {
        println!("  position  {shown}");
        match note {
          Some(note) => println!("  koreader  {} ({note})", local.xpointer),
          None => println!("  koreader  {}", local.xpointer),
        }
        match local.last_read {
          Some(t) => println!(
            "  progress  {}, last read {}",
            percent(local.percentage),
            format_timestamp(t)
          ),
          None => println!("  progress  {}", percent(local.percentage)),
        }
        Some(local)
      }
      Err(e) => {
        println!("  position  ({e})");
        None
      }
    };
    let Some(server) = &server else {
      continue;
    };
    if let Some(e) = &server_error {
      println!("  server    (not checked: {e})");
      continue;
    }
    match server.client.progress(&mapping.epub.hash) {
      Ok(remote) => {
        match &remote {
          Some(p) => println!("  server    {}", describe(p)),
          None => println!("  server    (nothing stored)"),
        }
        if let Some(local) = &local {
          println!(
            "  sync      {}",
            sync_note(local, remote.as_ref(), server.config.confirm)
              .replace("pull would move the device", "the Kindle is only read")
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

fn kindle_push(options: &Options, force: bool, dry_run: bool) -> Result<bool> {
  let session = open_kindle(options)?;
  let server = require_server(options)?;
  let (device, device_id) = server.config.identity("Kindle");
  if let Err(e) = server.client.authorize() {
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
    kindle_heading(&session.root, book);
    let step = session.push_step(book, &server.client, confirm_setting, force);
    let mapping = match (&step.mapping, &step.outcome) {
      (_, PushOutcome::Skip { message, .. }) => {
        println!("  skipped   {message}");
        continue;
      }
      (Some(mapping), _) => mapping,
      (None, _) => unreachable!("only a skipped book has no mapping"),
    };
    let hash = &mapping.epub.hash;
    println!(
      "  sync as   mapped to {} (checksum {hash})",
      mapping.epub.path
    );
    let (local, note, remote, action) = match step.outcome {
      PushOutcome::Failed(e) => {
        println!("  failed    {e}");
        ok = false;
        continue;
      }
      PushOutcome::Ready {
        local,
        note,
        remote,
        action,
        ..
      } => (local, note, remote, action),
      PushOutcome::Skip { .. } | PushOutcome::NotInLine { .. } => {
        unreachable!("a Kindle book is skipped above and has no plugin")
      }
    };
    let remote = remote.as_ref();
    match action {
      Action::InSync => {
        println!("  in sync   {}", describe(remote.expect("both hold it")));
        continue;
      }
      Action::Apply => {}
      Action::Keep => {
        let remote = remote.expect("only a server position is kept");
        println!("  skipped   server is newer: {}", describe(remote));
        println!("            --force sends the Kindle position anyway");
        continue;
      }
      Action::Ask { backwards } => {
        let remote = remote.expect("asks only about a server position");
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
    let update = session::update(hash, &local, &device, &device_id);
    if dry_run {
      println!("  would send {}", sent_what(&update, note));
      continue;
    }
    match send(&server.client, &update, note, false) {
      Ok(t) => record(&[sent_entry(&update, t, book.title.clone())]),
      Err(e) => {
        ok = false;
        if matches!(e, kosync::Error::Unauthorized) {
          break;
        }
      }
    }
  }
  unanswered_note(unanswered);
  if dry_run {
    println!("\nDry run: nothing was sent.");
  }
  Ok(ok)
}
