use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_nickelinksync"))
    .args(args)
    .output()
    .expect("failed to run nickelinksync")
}

#[test]
fn version_prints_crate_version() {
  let output = run(&["--version"]);
  assert!(output.status.success());
  assert_eq!(
    String::from_utf8_lossy(&output.stdout).trim(),
    format!("nickelinksync {}", env!("CARGO_PKG_VERSION"))
  );
}

#[test]
fn push_and_pull_need_a_device() {
  for command in ["push", "pull"] {
    let output = run(&[command]);
    assert!(!output.status.success(), "`{command}` should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--device"), "`{command}` printed: {stderr}");
  }
}

#[test]
fn status_needs_a_device() {
  let output = run(&["status"]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("--device"));
}

#[test]
fn device_check_reports_and_fails_on_a_problem() {
  let dir = std::env::temp_dir().join(format!("nickelinksync-cli-check-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(dir.join(".kobo")).unwrap();
  std::fs::write(
    dir.join(".kobo/version"),
    "N1,4.9.77,6.0.279225,4.9.77,4.9.77,00000000-0000-0000-0000-000000000391\n",
  )
  .unwrap();
  let output = run(&["device", "check", "--device", dir.to_str().unwrap()]);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(!output.status.success(), "{stdout}");
  assert!(
    stdout.contains("Kobo Clara BW, firmware 6.0.279225 (tested)"),
    "{stdout}"
  );
  assert!(stdout.contains("✗ Config"), "{stdout}");
  assert!(!dir.join(".adds").exists(), "check must not write");
  let _ = std::fs::remove_dir_all(&dir);
}
