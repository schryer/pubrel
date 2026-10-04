//! The release record: a package's `release.json`, its unreleased changes,
//! and the package publet its releases are versions of.
//!
//! Building and publishing the package publet -- which signs it -- go
//! through the installed `pub`, a released 0.x (`^0.1`). Reading it back is
//! done here, with `publet-core` (see `published`).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::manifest::Kind;
use crate::version::{self, Version};

/// A repository's `release.json`.
pub struct Config {
    pub root: PathBuf,
    /// The package's name, as releases are titled: `publet-cli`.
    pub name: String,
    /// The package publet's slug: `pkg.publet-cli`.
    pub package: String,
    /// Its Section 9.1 citation tag.
    pub tag: String,
    /// The corpus directory, relative to the root.
    pub corpus: PathBuf,
    pub kind: Kind,
    pub manifest: PathBuf,
    /// Path prefixes whose change is a change to what users run.
    pub code: Vec<String>,
    /// The command the package provides, and how to ask its version.
    pub command: Option<(String, String)>,
    /// How to install a release; `{tag}` is substituted.
    pub install: Option<String>,
    /// The key object a release must be signed by: the corpus's author.
    pub key: Option<String>,
}

impl Config {
    /// Read `release.json` from `root`.
    ///
    /// # Errors
    ///
    /// Returns a message if it is missing or malformed.
    pub fn read(root: &Path) -> Result<Self, String> {
        let path = root.join("release.json");
        let text = std::fs::read_to_string(&path).map_err(|_| {
            format!(
                "no release.json in {}; `pubrel init` writes one",
                root.display()
            )
        })?;
        let json: Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_owned);
        let need = |k: &str| text(k).ok_or_else(|| format!("release.json needs `{k}`"));
        let manifest = json
            .get("manifest")
            .ok_or("release.json needs `manifest`: {kind, path}")?;
        let kind = Kind::parse(
            manifest
                .get("kind")
                .and_then(Value::as_str)
                .ok_or("manifest.kind is required")?,
        )?;
        let command = json.get("command").and_then(|c| {
            Some((
                c.get("name")?.as_str()?.to_owned(),
                c.get("check")?.as_str()?.to_owned(),
            ))
        });
        Ok(Self {
            root: root.to_path_buf(),
            name: need("name")?,
            package: need("package")?,
            tag: need("tag")?,
            corpus: PathBuf::from(text("corpus").unwrap_or_else(|| "corpus".to_owned())),
            kind,
            manifest: PathBuf::from(
                manifest
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("manifest.path is required")?,
            ),
            code: json
                .get("code")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            command,
            install: text("install"),
            key: text("key"),
        })
    }

    pub fn corpus_dir(&self) -> PathBuf {
        self.root.join(&self.corpus)
    }

    pub fn package_dir(&self) -> PathBuf {
        self.corpus_dir().join("publets").join(&self.package)
    }

    /// The unreleased list, relative to the root, as git names it.
    pub fn unreleased_rel(&self) -> String {
        self.corpus
            .join("publets")
            .join(&self.package)
            .join("unreleased.json")
            .to_string_lossy()
            .into_owned()
    }

    pub fn lock_rel(&self) -> String {
        self.corpus
            .join("corpus.lock")
            .to_string_lossy()
            .into_owned()
    }

    pub fn manifest_rel(&self) -> String {
        self.manifest.to_string_lossy().into_owned()
    }

    /// The manifest's current version.
    ///
    /// # Errors
    ///
    /// Returns a message if it cannot be read.
    pub fn manifest_version(&self) -> Result<Version, String> {
        let text = std::fs::read_to_string(self.root.join(&self.manifest))
            .map_err(|e| format!("{}: {e}", self.manifest.display()))?;
        crate::manifest::read(&text, self.kind)
    }
}

/// One recorded change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub category: String,
    pub change: String,
}

/// The changes an `unreleased.json` lists.
///
/// # Errors
///
/// Returns a message if it is malformed or names an unknown category.
pub fn changes(text: Option<&str>) -> Result<Vec<Change>, String> {
    let Some(text) = text else {
        return Ok(Vec::new());
    };
    let json: Value = serde_json::from_str(text).map_err(|e| format!("unreleased.json: {e}"))?;
    let mut out = Vec::new();
    for row in json
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |k: &str| {
            row.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let (category, change) = (field("category"), field("change"));
        if !version::known(&category) || change.trim().is_empty() {
            return Err(format!(
                "each change needs a known category ({}) and a description: {row}",
                version::CATEGORIES.join(", ")
            ));
        }
        out.push(Change { category, change });
    }
    Ok(out)
}

/// Write an `unreleased.json`.
///
/// # Errors
///
/// Returns a message if the file cannot be written.
pub fn write_changes(path: &Path, rows: &[Change]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = json!({
        "comment": "Changes not yet released. One row per change: category changed or \
                    removed (a published interface changed: major), added (a feature: \
                    minor), or fixed or security (patch). `pubrel add` appends one; \
                    `pubrel prepare` moves them into the package publet.",
        "changes": rows.iter().map(|r| json!({"category": r.category, "change": r.change}))
            .collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The package publet's source for a release.
#[must_use]
pub fn package_source(
    cfg: &Config,
    version: Version,
    date: &str,
    commit: &str,
    rows: &[Change],
) -> Value {
    let tag = format!("v{version}");
    let (name, check, expect) = match &cfg.command {
        Some((name, check)) => (name.clone(), check.clone(), format!("{name} {version}")),
        None => (cfg.name.clone(), String::new(), String::new()),
    };
    let scope = format!("the published release {tag} of {}", cfg.name);
    let columns =
        |names: &[&str]| -> Vec<Value> { names.iter().map(|n| json!({"name": n})).collect() };
    json!({
        "slug": cfg.package, "tag": cfg.tag, "created": format!("{date}T00:00:00Z"),
        "title": cfg.name,
        "claims": {
            "identity": {
                "class": "archival", "scope": scope,
                "content": format!("{} {version} is released as {tag} from commit {commit}.", cfg.name),
                "data": {"columns": columns(&["name", "version", "check", "expect", "tag", "commit"]),
                         "rows": [[name, version.to_string(), check, expect, tag, commit]]},
                "sources": [{"ref": cfg.name, "revision": commit}],
            },
            "release": {
                "class": "archival", "scope": scope,
                "content": format!("{} {version} was released on {date} from commit {commit} with these changes.", cfg.name),
                "depends": ["#identity"],
                "data": {"columns": columns(&["category", "change"]),
                         "rows": rows.iter().map(|r| json!([r.category, r.change])).collect::<Vec<_>>()},
                "sources": [{"ref": cfg.name, "revision": commit}],
            },
        },
        "sections": [
            {"heading": "Identity", "items": [{"ref": "#identity"}]},
            {"heading": "Release", "items": [{"ref": "#release"}]},
        ],
    })
}

// --- pub --------------------------------------------------------------------

/// The `pub` to drive: `$PUB`, or `pub` on the PATH.
pub fn pub_program() -> String {
    std::env::var("PUB").unwrap_or_else(|_| "pub".to_owned())
}

/// Confirm the `pub` this will drive is a released 0.x, at least 0.1.
///
/// # Errors
///
/// Returns a message if it cannot run or is not such a release.
pub fn require_pub() -> Result<(), String> {
    let program = pub_program();
    let out = Command::new(&program)
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run `{program} --version` ({e}); install a released pub"))?;
    let first = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let version = first
        .strip_prefix("pub ")
        .and_then(|v| Version::parse(v).ok())
        .filter(|v| v.major == 0 && v.minor >= 1);
    if version.is_none() {
        return Err(format!(
            "pubrel drives a released pub ^0.1, and `{program} --version` says {first:?}; \
             install one: cargo install --locked --git https://github.com/schryer/publet \
             --tag v0.1.1 publet-cli"
        ));
    }
    Ok(())
}

/// Run `pub` in `dir`.
///
/// # Errors
///
/// Returns a message with `pub`'s output if it fails.
pub fn run_pub(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(pub_program())
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run pub: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!(
            "pub {} failed:\n{text}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

pub use crate::published::read_object;

/// The identifier a lock records for `slug`.
#[must_use]
pub fn locked(lock_text: Option<&str>, slug: &str) -> Option<String> {
    let json: Value = serde_json::from_str(lock_text?).ok()?;
    json.get("publets")?
        .as_array()?
        .iter()
        .find(|r| r.get("slug").and_then(Value::as_str) == Some(slug))?
        .get("cid")?
        .as_str()
        .map(str::to_owned)
}

/// The version a lock's published package identity states, if any.
///
/// # Errors
///
/// Returns a message if the identity is recorded but cannot be read.
pub fn published_version(cfg: &Config, lock_text: Option<&str>) -> Result<Option<Version>, String> {
    let Some(cid) = locked(lock_text, &format!("{}#identity", cfg.package)) else {
        return Ok(None);
    };
    let object = read_object(cfg, &cid)?;
    let stated = object
        .pointer("/body/data/rows/0/1")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{cid} states no version"))?;
    Version::parse(stated).map(Some)
}

/// A published release, read back from its `release` claim.
pub struct Release {
    pub version: Version,
    pub date: String,
    pub commit: String,
    pub cid: String,
    pub rows: Vec<(String, String)>,
}

/// Every published release of the package, newest first: the `release`
/// claim's lineage, read from the corpus's objects.
#[must_use]
pub fn releases(cfg: &Config) -> Vec<Release> {
    let prefix = format!("{} ", cfg.name);
    let mut out = Vec::new();
    let dir = cfg.corpus_dir().join("objects");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".cbor") else {
            continue;
        };
        let cid = stem.replace('_', ":");
        let Ok(object) = read_object(cfg, &cid) else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("claim.prose") {
            continue;
        }
        let content = object
            .pointer("/body/content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(rest) = content.strip_prefix(&prefix) else {
            continue;
        };
        // "<version> was released on <date> from commit <sha> with these changes."
        let words: Vec<&str> = rest.split_whitespace().collect();
        let (Some(v), Some(&"was"), Some(date), Some(commit)) =
            (words.first(), words.get(1), words.get(4), words.get(7))
        else {
            continue;
        };
        let Ok(version) = Version::parse(v) else {
            continue;
        };
        let rows = object
            .pointer("/body/data/rows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some((
                    r.get(0)?.as_str()?.to_owned(),
                    r.get(1)?.as_str()?.to_owned(),
                ))
            })
            .collect();
        out.push(Release {
            version,
            date: (*date).to_owned(),
            commit: (*commit).to_owned(),
            cid,
            rows,
        });
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.version));
    out
}

/// One release as a changelog section.
#[must_use]
pub fn section(r: &Release) -> String {
    use std::fmt::Write as _;
    let mut out = format!(
        "## {} - {}\n\nCommit `{}`; release record `{}`.\n\n",
        r.version,
        r.date,
        r.commit.chars().take(12).collect::<String>(),
        r.cid
    );
    for category in version::CATEGORIES {
        let entries: Vec<&String> = r
            .rows
            .iter()
            .filter(|(c, _)| c == category)
            .map(|(_, e)| e)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let mut title = category.to_owned();
        if let Some(first) = title.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let _ = writeln!(out, "### {title}");
        for e in entries {
            let _ = writeln!(out, "- {e}");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn unreleased_rows_are_validated() {
        let ok = changes(Some(
            r#"{"changes": [{"category": "added", "change": "x"}]}"#,
        ))
        .unwrap();
        assert_eq!(ok.len(), 1);
        assert!(
            changes(Some(
                r#"{"changes": [{"category": "nope", "change": "x"}]}"#
            ))
            .is_err()
        );
        assert!(
            changes(Some(
                r#"{"changes": [{"category": "fixed", "change": " "}]}"#
            ))
            .is_err()
        );
        assert!(changes(None).unwrap().is_empty());
    }

    #[test]
    fn a_lock_row_is_found_by_slug() {
        let lock = r#"{"publets": [{"slug": "pkg.x#identity", "cid": "pub:sha2-256:abc"}]}"#;
        assert_eq!(
            locked(Some(lock), "pkg.x#identity").as_deref(),
            Some("pub:sha2-256:abc")
        );
        assert_eq!(locked(Some(lock), "pkg.y#identity"), None);
        assert_eq!(locked(None, "pkg.x#identity"), None);
    }
}
