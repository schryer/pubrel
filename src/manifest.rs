//! Where a package states its version, and how to change it.
//!
//! Edits go through `toml_edit`, so a manifest keeps its comments and
//! layout: a release changes the version and nothing else about the file.

use std::path::Path;
use std::process::Command;

use toml_edit::{DocumentMut, Item, value};

use crate::version::{self, Version};

/// The kinds of manifest a package may keep its version in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `[workspace.package].version`, which every internal path dependency
    /// in `[workspace.dependencies]` also pins.
    CargoWorkspace,
    /// `[package].version`.
    CargoPackage,
    /// `[project].version` in a `pyproject.toml`.
    Pyproject,
}

impl Kind {
    /// Read a kind from its name in `release.json`.
    ///
    /// # Errors
    ///
    /// Returns a message naming the known kinds.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "cargo-workspace" => Ok(Self::CargoWorkspace),
            "cargo-package" => Ok(Self::CargoPackage),
            "pyproject" => Ok(Self::Pyproject),
            other => Err(format!(
                "unknown manifest kind `{other}` (cargo-workspace, cargo-package, pyproject)"
            )),
        }
    }

    fn table(self) -> [&'static str; 2] {
        match self {
            Self::CargoWorkspace => ["workspace", "package"],
            Self::CargoPackage => ["package", ""],
            Self::Pyproject => ["project", ""],
        }
    }
}

fn version_item(doc: &DocumentMut, kind: Kind) -> Option<&Item> {
    let [outer, inner] = kind.table();
    let table = doc.get(outer)?;
    let table = if inner.is_empty() {
        table
    } else {
        table.get(inner)?
    };
    table.get("version")
}

/// The version a manifest states.
///
/// # Errors
///
/// Returns a message if the text is not TOML or states no version there.
pub fn read(text: &str, kind: Kind) -> Result<Version, String> {
    let doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let stated = version_item(&doc, kind)
        .and_then(Item::as_str)
        .ok_or_else(|| format!("the manifest states no version where a {kind:?} does"))?;
    version::parse(stated)
}

/// The manifest with its version set to `version`. For a workspace, every
/// internal path dependency pinned at the old version moves with it, or
/// the workspace no longer resolves -- except those under `own`: crates
/// released with versions of their own, whose pins only their own
/// releases change (see [`pin`]), even when the two versions coincide.
///
/// # Errors
///
/// Returns a message if the text is not TOML or states no version.
pub fn write(text: &str, kind: Kind, version: &Version, own: &[String]) -> Result<String, String> {
    let old = read(text, kind)?.to_string();
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let [outer, inner] = kind.table();
    let table = if inner.is_empty() {
        &mut doc[outer]
    } else {
        &mut doc[outer][inner]
    };
    table["version"] = value(version.to_string());

    if kind == Kind::CargoWorkspace
        && let Some(deps) = doc
            .get_mut("workspace")
            .and_then(|w| w.get_mut("dependencies"))
            .and_then(Item::as_table_like_mut)
    {
        for (_, dep) in deps.iter_mut() {
            let Some(dep) = dep.as_table_like_mut() else {
                continue;
            };
            let Some(path) = dep.get("path").and_then(Item::as_str) else {
                continue;
            };
            if own.iter().any(|o| same_dir(o, path)) {
                continue;
            }
            if dep.get("version").and_then(Item::as_str) == Some(old.as_str()) {
                dep.insert("version", value(version.to_string()));
            }
        }
    }
    Ok(doc.to_string())
}

fn same_dir(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// A workspace manifest with the dependency at `dir` pinned at `version`,
/// if `[workspace.dependencies]` names one there; `None` if it does not.
///
/// # Errors
///
/// Returns a message if the text is not TOML.
pub fn pin(text: &str, dir: &str, version: &Version) -> Result<Option<String>, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let Some(deps) = doc
        .get_mut("workspace")
        .and_then(|w| w.get_mut("dependencies"))
        .and_then(Item::as_table_like_mut)
    else {
        return Ok(None);
    };
    let mut found = false;
    for (_, dep) in deps.iter_mut() {
        let Some(dep) = dep.as_table_like_mut() else {
            continue;
        };
        if dep
            .get("path")
            .and_then(Item::as_str)
            .is_some_and(|p| same_dir(p, dir))
        {
            dep.insert("version", value(version.to_string()));
            found = true;
        }
    }
    Ok(found.then(|| doc.to_string()))
}

/// Bring the lockfile in line with the new version, where there is one.
///
/// # Errors
///
/// Returns a message if the package manager fails.
pub fn refresh_lock(root: &Path, kind: Kind, name: &str) -> Result<(), String> {
    let args: Vec<&str> = match kind {
        Kind::CargoWorkspace => vec!["update", "--workspace", "--offline"],
        Kind::CargoPackage => vec!["update", "-p", name, "--offline"],
        Kind::Pyproject => return Ok(()),
    };
    if !root.join("Cargo.lock").exists() {
        return Ok(());
    }
    let status = Command::new("cargo")
        .args(&args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const WORKSPACE: &str = r#"[workspace]
members = ["crates/*"]

# The version every crate shares.
[workspace.package]
version = "0.0.1"
edition = "2024"

[workspace.dependencies]
graphset = { git = "https://example.org/g", rev = "abc", version = "0.1.0" }
core = { path = "crates/core", version = "0.0.1" }
"#;

    #[test]
    fn a_workspace_moves_with_its_internal_dependencies() {
        let v = &version::parse("0.1.0").unwrap();
        let out = write(WORKSPACE, Kind::CargoWorkspace, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::CargoWorkspace).unwrap(), *v);
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.1.0" }"#),
            "{out}"
        );
        assert!(out.contains(r#"rev = "abc", version = "0.1.0""#), "{out}");
        assert!(
            out.contains("# The version every crate shares."),
            "comments kept"
        );
    }

    #[test]
    fn a_package_and_a_pyproject_change_only_their_version() {
        let v = &version::parse("0.2.0").unwrap();
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1.0\" # keep\n";
        let out = write(cargo, Kind::CargoPackage, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::CargoPackage).unwrap(), *v);
        assert!(out.contains("name = \"x\""));
        let py = "[project]\nname = \"y\"\nversion = \"0.1.0\"\n";
        let out = write(py, Kind::Pyproject, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::Pyproject).unwrap(), *v);
    }

    #[test]
    fn a_crate_with_its_own_version_keeps_its_pin_through_a_workspace_bump() {
        // core shares the workspace's old version by coincidence; it is
        // released on its own, so the workspace bump leaves it alone.
        let v = &version::parse("0.1.0").unwrap();
        let out = write(
            WORKSPACE,
            Kind::CargoWorkspace,
            v,
            &["crates/core".to_owned()],
        )
        .unwrap();
        assert_eq!(read(&out, Kind::CargoWorkspace).unwrap(), *v);
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.0.1" }"#),
            "{out}"
        );
    }

    #[test]
    fn its_own_release_pins_it() {
        let v = &version::parse("0.3.0").unwrap();
        let out = pin(WORKSPACE, "crates/core/", v).unwrap().unwrap();
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.3.0" }"#),
            "{out}"
        );
        assert_eq!(
            read(&out, Kind::CargoWorkspace).unwrap().to_string(),
            "0.0.1"
        );
        assert!(pin(WORKSPACE, "crates/other", v).unwrap().is_none());
    }

    #[test]
    fn a_manifest_without_a_version_there_is_refused() {
        assert!(read("[package]\nname = \"x\"\n", Kind::CargoPackage).is_err());
        assert!(Kind::parse("npm").is_err());
    }
}
