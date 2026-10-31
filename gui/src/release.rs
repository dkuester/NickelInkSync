//! The notice of a newer release: at most once a day the window asks
//! GitHub for the latest release (`/releases/latest`, which leaves out
//! drafts and pre-releases) and names it if it is newer than this one. It
//! can be switched off in "About"; nothing is installed by itself.

use std::time::Duration;

use serde_json::Value;

const LATEST: &str = "https://api.github.com/repos/dkuester/NickelInkSync/releases/latest";
/// How long a check counts, in seconds.
pub const EVERY: i64 = 24 * 60 * 60;

/// A newer release than this one.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
  /// "0.2.0".
  pub version: String,
  /// Its page on GitHub.
  pub url: String,
}

/// Whether a check is due: never checked, or a day ago or longer.
pub fn due(last: Option<i64>, now: i64) -> bool {
  last.is_none_or(|t| now - t >= EVERY || now < t)
}

/// "v0.2.0" or "0.2.0" as numbers; `None` for anything else.
fn numbers(version: &str) -> Option<Vec<u64>> {
  version
    .trim()
    .trim_start_matches('v')
    .split('.')
    .map(|n| n.parse().ok())
    .collect()
}

/// Whether `tag` names a newer version than `current`.
pub fn newer(tag: &str, current: &str) -> bool {
  match (numbers(tag), numbers(current)) {
    (Some(tag), Some(current)) => tag > current,
    _ => false,
  }
}

/// The release in GitHub's answer, if it is newer than `current`.
pub fn from_answer(answer: &Value, current: &str) -> Option<Release> {
  let tag = answer.get("tag_name")?.as_str()?;
  let url = answer.get("html_url")?.as_str()?;
  newer(tag, current).then(|| Release {
    version: tag.trim_start_matches('v').to_string(),
    url: url.to_string(),
  })
}

/// Asks GitHub; `None` without a newer release (also before the first).
pub fn check() -> Result<Option<Release>, String> {
  let agent: ureq::Agent = ureq::Agent::config_builder()
    .http_status_as_error(false)
    .timeout_global(Some(Duration::from_secs(10)))
    .user_agent(concat!("NickelInkSync/", env!("CARGO_PKG_VERSION")))
    .build()
    .into();
  let mut response = agent
    .get(LATEST)
    .header("accept", "application/vnd.github+json")
    .call()
    .map_err(|e| e.to_string())?;
  match response.status().as_u16() {
    200 => {}
    // No release yet.
    404 => return Ok(None),
    status => return Err(format!("GitHub answered {status}")),
  }
  let answer: Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
  Ok(from_answer(&answer, env!("CARGO_PKG_VERSION")))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_a_newer_release_counts() {
    assert!(newer("v0.2.0", "0.1.0"));
    assert!(newer("0.1.1", "0.1.0"));
    assert!(newer("v1.0.0", "0.9.9"));
    assert!(newer("v0.10.0", "0.9.0"));
    assert!(!newer("v0.1.0", "0.1.0"));
    assert!(!newer("v0.0.9", "0.1.0"));
    assert!(!newer("nightly", "0.1.0"));
    let answer = serde_json::json!({
      "tag_name": "v0.2.0",
      "html_url": "https://github.com/dkuester/NickelInkSync/releases/tag/v0.2.0"
    });
    assert_eq!(
      from_answer(&answer, "0.1.0"),
      Some(Release {
        version: "0.2.0".into(),
        url: "https://github.com/dkuester/NickelInkSync/releases/tag/v0.2.0".into()
      })
    );
    assert_eq!(from_answer(&answer, "0.2.0"), None);
  }

  #[test]
  fn once_a_day() {
    assert!(due(None, 1_000_000));
    assert!(!due(Some(1_000_000), 1_000_000 + EVERY - 1));
    assert!(due(Some(1_000_000), 1_000_000 + EVERY));
    // A clock set back does not stop it for good.
    assert!(due(Some(1_000_000), 999_000));
  }
}
