//! Security checks a release must pass, and the signed record of them.
//!
//! `release.json` declares the checks -- commands such as
//! `cargo deny check` or `cargo vet --locked` -- and facts worth keeping
//! with them, such as the revision of the advisory database the check
//! read. `prepare` runs every check before it releases anything, refuses
//! to release if one fails, and records what ran as a `security` claim in
//! the package publet: the command, the tool's version, the outcome and
//! the tool's own summary line, beside each fact. The claim is signed with
//! the release, so what was checked for a version is as verifiable as the
//! version itself.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// A command a release must pass.
#[derive(Debug, Clone)]
pub struct Check {
    /// What the check is called in the record: `advisories`.
    pub name: String,
    /// The shell command that performs it; it passes by exiting zero.
    pub run: String,
    /// A shell command whose first line names the tool and its version.
    pub version: Option<String>,
}

/// A command whose first line of output is recorded beside the checks.
#[derive(Debug, Clone)]
pub struct Fact {
    pub name: String,
    pub run: String,
}

/// What `release.json`'s `security` section declares.
#[derive(Debug, Clone, Default)]
pub struct Declared {
    pub checks: Vec<Check>,
    pub facts: Vec<Fact>,
}

impl Declared {
    /// Read a `security` section: `{"checks": [...], "facts": [...]}`.
    ///
    /// # Errors
    ///
    /// Returns a message naming what is missing.
    pub fn from_json(json: Option<&Value>) -> Result<Self, String> {
        let Some(json) = json else {
            return Ok(Self::default());
        };
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let list = |k: &str| {
            json.get(k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let checks = list("checks")
            .iter()
            .map(|c| {
                Ok(Check {
                    name: text(c, "name").ok_or("each security check needs a `name`")?,
                    run: text(c, "run").ok_or("each security check needs a `run` command")?,
                    version: text(c, "version"),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let facts = list("facts")
            .iter()
            .map(|f| {
                Ok(Fact {
                    name: text(f, "name").ok_or("each security fact needs a `name`")?,
                    run: text(f, "run").ok_or("each security fact needs a `run` command")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { checks, facts })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty() && self.facts.is_empty()
    }
}

/// One row of the record: a check that passed, or a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// `check` or `fact`.
    pub kind: String,
    pub name: String,
    /// The command run.
    pub command: String,
    /// The tool and its version, for a check; empty for a fact.
    pub tool: String,
    /// For a check, the tool's summary: the last line it printed. For a
    /// fact, its value: the first line printed.
    pub result: String,
}

fn shell(command: &str, dir: &Path) -> Result<(bool, String), String> {
    let out = Command::new("sh")
        .args(["-c", command])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run `{command}`: {e}"))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.success(), text))
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn last_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default();
    line.chars().take(200).collect()
}

/// Run every declared check and fact in `dir`.
///
/// # Errors
///
/// Returns a message, with the end of its output, if any check fails or
/// any command cannot be run: a release is cut only when its checks pass.
pub fn run(declared: &Declared, dir: &Path) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for check in &declared.checks {
        println!("security check {}: {}", check.name, check.run);
        let (ok, output) = shell(&check.run, dir)?;
        if !ok {
            let tail: Vec<&str> = output.lines().rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(format!(
                "security check `{}` failed, so nothing is released:\n{}",
                check.name,
                tail.join("\n")
            ));
        }
        let tool = match &check.version {
            Some(command) => first_line(&shell(command, dir)?.1),
            None => String::new(),
        };
        rows.push(Row {
            kind: "check".to_owned(),
            name: check.name.clone(),
            command: check.run.clone(),
            tool,
            result: last_line(&output),
        });
    }
    for fact in &declared.facts {
        let (ok, output) = shell(&fact.run, dir)?;
        if !ok {
            return Err(format!(
                "security fact `{}` could not be read: `{}` failed",
                fact.name, fact.run
            ));
        }
        rows.push(Row {
            kind: "fact".to_owned(),
            name: fact.name.clone(),
            command: fact.run.clone(),
            tool: String::new(),
            result: first_line(&output),
        });
    }
    Ok(rows)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn summaries_are_the_first_and_last_lines_printed() {
        assert_eq!(first_line("\n  tool 1.2\nmore\n"), "tool 1.2");
        assert_eq!(last_line("checking\n\nadvisories ok\n\n"), "advisories ok");
        assert_eq!(last_line(""), "");
    }

    #[test]
    fn a_declaration_needs_names_and_commands() {
        let ok = serde_json::json!({"checks": [{"name": "a", "run": "true"}]});
        assert_eq!(Declared::from_json(Some(&ok)).unwrap().checks.len(), 1);
        let bad = serde_json::json!({"checks": [{"run": "true"}]});
        assert!(Declared::from_json(Some(&bad)).is_err());
        assert!(Declared::from_json(None).unwrap().is_empty());
    }
}
