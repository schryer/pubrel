//! A crate's public API, checked against its last release.
//!
//! A package whose `release.json` entry sets `"semver-checks": true` has
//! its public API compared with the one at its last release's tag, by
//! `cargo-semver-checks`. The changes recorded since that release say how
//! far the API may move:
//!
//! | recorded                  | the API may         | level |
//! |---------------------------|---------------------|-------|
//! | `changed` or `removed`    | break               | major |
//! | `added`                   | grow, not break     | minor |
//! | anything else             | stay as it is       | patch |
//!
//! The levels are what a change *is*, not the version it produces: below
//! 1.0.0 a break bumps the minor version (see [`crate::version`]), but it
//! is still a break, and `cargo-semver-checks` reads the level that way.
//! So an unrecorded break fails, whatever the version.
//!
//! The tool is run as `cargo-semver-checks semver-checks`, exactly as
//! `cargo semver-checks` runs it, so the first one on `PATH` is used.

use std::process::Command;

use crate::record::Config;
use crate::version::Version;

/// How far the public API may change, given the categories recorded.
#[must_use]
pub fn allowed(categories: &[&str]) -> &'static str {
    if categories
        .iter()
        .any(|c| matches!(*c, "changed" | "removed"))
    {
        "major"
    } else if categories.contains(&"added") {
        "minor"
    } else {
        "patch"
    }
}

/// Check `cfg`'s API against its release `baseline`, allowing what
/// `categories` record; return the line to report.
///
/// # Errors
///
/// Returns a message if `git` or `cargo-semver-checks` cannot be started,
/// if the baseline's tag is not in the repository, if
/// [`Config::crate_name`] fails, or if the tool exits
/// non-zero: the API changed more than the changes recorded allow, or the
/// tool failed for another reason, whose output the message includes.
pub fn check(cfg: &Config, baseline: &Version, categories: &[&str]) -> Result<String, String> {
    let tag = cfg.git_tag(baseline);
    let tagged = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/tags/{tag}"),
        ])
        .current_dir(&cfg.root)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !tagged.status.success() {
        return Err(format!(
            "{}'s API is checked against its last release, but the tag {tag} does not exist here; \
             fetch tags (`git fetch --tags`) and try again",
            cfg.name
        ));
    }
    let crate_name = cfg.crate_name()?;
    let level = allowed(categories);
    let out = Command::new("cargo-semver-checks")
        .args([
            "semver-checks",
            "check-release",
            "--package",
            &crate_name,
            "--baseline-rev",
            &tag,
            "--release-type",
            level,
        ])
        .current_dir(cfg.root.join(cfg.manifest_dir()))
        .output()
        .map_err(|e| {
            format!(
                "release.json asks for {}'s API to be checked, but cargo-semver-checks cannot \
                 run ({e}); install it: cargo install --locked cargo-semver-checks",
                cfg.name
            )
        })?;
    if out.status.success() {
        return Ok(format!(
            "API: changes no more than a {level} release allows, against {tag}"
        ));
    }
    let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&out.stderr));
    Err(format!(
        "{}'s public API changed more than its recorded changes allow ({}, so a {level} \
         release) since {tag}:\n{}\n{}",
        cfg.name,
        recorded(categories),
        tail(&output, 40),
        advice(level)
    ))
}

fn recorded(categories: &[&str]) -> String {
    if categories.is_empty() {
        "none recorded".to_owned()
    } else {
        let mut seen: Vec<&str> = Vec::new();
        for c in categories {
            if !seen.contains(c) {
                seen.push(c);
            }
        }
        format!("recorded: {}", seen.join(", "))
    }
}

fn advice(level: &str) -> &'static str {
    if level == "minor" {
        "If the break is intended, record it: `pubrel add changed \"...\"` or `pubrel add removed \"...\"`."
    } else {
        "If the change is intended, record it: `pubrel add added \"...\"` for an addition, \
         `pubrel add changed \"...\"` or `pubrel add removed \"...\"` for a break."
    }
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: api::allowed
    #[test]
    fn the_strongest_recorded_change_sets_the_level() {
        assert_eq!(allowed(&["fixed", "removed"]), "major");
        assert_eq!(allowed(&["changed"]), "major");
        assert_eq!(allowed(&["fixed", "added"]), "minor");
        assert_eq!(allowed(&["fixed", "security", "internal"]), "patch");
        assert_eq!(allowed(&[]), "patch");
    }

    // covers: api::recorded
    #[test]
    fn categories_are_reported_once_each() {
        assert_eq!(
            recorded(&["fixed", "added", "fixed"]),
            "recorded: fixed, added"
        );
        assert_eq!(recorded(&[]), "none recorded");
    }
}
