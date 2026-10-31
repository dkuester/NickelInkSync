//! The tests' data directory: no configuration, no mappings and no history
//! of the machine running the tests.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// The data directory, empty, for as long as a test holds it.
///
/// The environment variables pointing to it belong to the process, so all
/// tests share the one directory: the mappings, the history and the backups
/// one test writes there would be seen by another running at the same time.
/// The lock lets one test at a time use it.
pub struct DataDir {
  _lock: MutexGuard<'static, ()>,
}

pub fn isolate() -> DataDir {
  static DIR: OnceLock<PathBuf> = OnceLock::new();
  static LOCK: Mutex<()> = Mutex::new(());
  let lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
  let dir = DIR.get_or_init(|| {
    let dir = std::env::temp_dir().join(format!("nickelinksync-gui-{}-data", std::process::id()));
    std::env::set_var("NICKELINKSYNC_DATA_DIR", &dir);
    std::env::set_var("NICKELINKSYNC_CONFIG", dir.join("no-config.ini"));
    dir
  });
  // What a test that failed half-way left behind.
  let _ = fs::remove_dir_all(dir);
  DataDir { _lock: lock }
}
