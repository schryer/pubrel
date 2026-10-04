//! `pubrel`: version a package by what changed, and record each release as
//! a version of its package publet.
//!
//! A release of a package is a version of a publet -- `pkg.<name>` in the
//! package's own corpus -- whose `identity` claim states the version, tag,
//! and commit, and whose `release` claim lists what changed. The publet's
//! lineage is the release history; `CHANGELOG.md` is generated from it.
//! Changes not yet released accumulate in an `unreleased.json` beside the
//! package publet's source: workshop data, never published.
//!
//! The version follows from the categories of what changed, never from
//! judgement at release time (see [`version`]).

mod manifest;
mod published;
mod record;
mod version;

use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};

use record::{Change, Config};
use version::Version;

const USAGE: &str = "\
usage: pubrel <command>

  init                    write release.json and an empty unreleased.json
  add CATEGORY CHANGE...  record a change: changed, removed, added, fixed, security
  next                    the version the unreleased changes imply
  check BASE              CI: a code change records what it changes; a release
                          is the bump its changes require, and published
  prepare [--no-pr]       cut a release: publish the package publet, commit,
                          and open a release PR
  changelog               regenerate CHANGELOG.md from the package publet
  tag                     CI on main: tag and release what was published
  --version               this pubrel's version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version" | "version") => {
            println!("pubrel {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("init") => init(),
        Some("add") => add(&args[1..]),
        Some("next") => next(),
        Some("check") => args.get(1).map_or_else(
            || Err("usage: pubrel check BASE".to_owned()),
            |base| check(base),
        ),
        Some("prepare") => prepare(!args.iter().any(|a| a == "--no-pr")),
        Some("changelog") => changelog(),
        Some("tag") => tag(),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn root() -> Result<std::path::PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let top = run("git", &["rev-parse", "--show-toplevel"], &here)?;
    Ok(std::path::PathBuf::from(top.trim()))
}

fn config() -> Result<Config, String> {
    Config::read(&root()?)
}

/// Run a program in `dir`, returning its stdout.
fn run(program: &str, args: &[&str], dir: &Path) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// A file's content at a git ref, if it exists there.
fn at(cfg: &Config, rev: &str, path: &str) -> Option<String> {
    run("git", &["show", &format!("{rev}:{path}")], &cfg.root).ok()
}

fn unreleased(cfg: &Config) -> Result<Vec<Change>, String> {
    let path = cfg.root.join(cfg.unreleased_rel());
    record::changes(std::fs::read_to_string(path).ok().as_deref())
}

fn categories(rows: &[Change]) -> Vec<&str> {
    rows.iter().map(|r| r.category.as_str()).collect()
}

fn lock_text(cfg: &Config) -> Option<String> {
    std::fs::read_to_string(cfg.root.join(cfg.lock_rel())).ok()
}

// --- commands -----------------------------------------------------------------

fn init() -> Result<(), String> {
    let root = root()?;
    let path = root.join("release.json");
    if path.exists() {
        return Err("release.json already exists".to_owned());
    }
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package")
        .to_owned();
    let (kind, manifest) = if root.join("pyproject.toml").exists() {
        ("pyproject", "pyproject.toml")
    } else {
        ("cargo-package", "Cargo.toml")
    };
    let mut json = serde_json::json!({
        "name": name,
        "package": format!("pkg.{name}"),
        "tag": "PKG-CHANGE-01-2026",
        "corpus": "corpus",
        "manifest": {"kind": kind, "path": manifest},
        "code": ["src/", manifest],
    });
    // The key releases must be signed by: the corpus's author, if it has one.
    let author = std::fs::read_to_string(root.join("corpus/.publet/config"))
        .ok()
        .and_then(|c| {
            c.lines()
                .find_map(|l| l.strip_prefix("author=").map(str::to_owned))
        });
    if let Some(author) = author {
        json["key"] = serde_json::Value::String(author);
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    let cfg = Config::read(&root)?;
    record::write_changes(&cfg.root.join(cfg.unreleased_rel()), &[])?;
    println!("release.json and {} written.", cfg.unreleased_rel());
    println!(
        "Set `tag` to a Section 9.1 tag (PKG-XXXXXX-MM-YYYY) and `code` to the paths users run."
    );
    if !cfg.corpus_dir().join("corpus.json").exists() {
        println!("No corpus yet: in {}, run", cfg.corpus_dir().display());
        println!("  pub corpus init --parent=base=<path to publet-corpus>");
        println!("  pub sign --generate-key --principal=human --label=<you>");
        println!("  pub policy --root=<the key it printed>");
    }
    Ok(())
}

fn add(args: &[String]) -> Result<(), String> {
    let (Some(category), text) = (args.first(), args.get(1..).unwrap_or_default().join(" ")) else {
        return Err("usage: pubrel add CATEGORY CHANGE...".to_owned());
    };
    if !version::known(category) {
        return Err(format!(
            "unknown category `{category}` (known: {})",
            version::CATEGORIES.join(", ")
        ));
    }
    if text.trim().is_empty() {
        return Err("say what changed: pubrel add CATEGORY CHANGE...".to_owned());
    }
    let cfg = config()?;
    let mut rows = unreleased(&cfg)?;
    rows.push(Change {
        category: category.clone(),
        change: text.trim().to_owned(),
    });
    record::write_changes(&cfg.root.join(cfg.unreleased_rel()), &rows)?;
    println!("{} change(s) unreleased", rows.len());
    Ok(())
}

fn next() -> Result<(), String> {
    let cfg = config()?;
    let previous = record::published_version(&cfg, lock_text(&cfg).as_deref())?;
    let rows = unreleased(&cfg)?;
    println!(
        "{}",
        version::next(previous, cfg.manifest_version()?, &categories(&rows))?
    );
    Ok(())
}

fn check(base: &str) -> Result<(), String> {
    let cfg = config()?;
    let changed = run(
        "git",
        &["diff", "--name-only", &format!("{base}...HEAD")],
        &cfg.root,
    )?;
    let code: Vec<&str> = changed
        .lines()
        .filter(|p| cfg.code.iter().any(|c| p.starts_with(c.as_str())))
        .collect();
    let rel = cfg.unreleased_rel();
    let before = record::changes(at(&cfg, base, &rel).as_deref())?;
    let after = unreleased(&cfg)?;
    let added: Vec<&Change> = after.iter().filter(|r| !before.contains(r)).collect();

    let base_lock = at(&cfg, base, &cfg.lock_rel());
    let base_identity = record::locked(base_lock.as_deref(), &format!("{}#identity", cfg.package));
    let now_lock = lock_text(&cfg);
    let now_identity = record::locked(now_lock.as_deref(), &format!("{}#identity", cfg.package));
    if now_identity != base_identity {
        // A release: published here, so it must be signed by the package's
        // key, exactly the bump its changes require, and the manifest must
        // say so.
        for part in ["identity", "release"] {
            let cid = record::locked(now_lock.as_deref(), &format!("{}#{part}", cfg.package))
                .ok_or_else(|| format!("the lock records no {}#{part}", cfg.package))?;
            published::require_signed(&cfg, &cid)?;
        }
        let previous = record::published_version(&cfg, base_lock.as_deref())?;
        let published = record::published_version(&cfg, now_lock.as_deref())?
            .ok_or("the package identity is recorded but states no version")?;
        let base_manifest = at(&cfg, base, &cfg.manifest_rel())
            .ok_or("the manifest does not exist at the base")
            .and_then(|t| {
                manifest::read(&t, cfg.kind).map_err(|_| "the base manifest has no version")
            })?;
        let release = record::releases(&cfg)
            .into_iter()
            .find(|r| r.version == published)
            .ok_or_else(|| format!("no published release record states {published}"))?;
        let cats: Vec<&str> = release.rows.iter().map(|(c, _)| c.as_str()).collect();
        let expected = version::next(previous, base_manifest, &cats)?;
        if published != expected {
            return Err(format!(
                "the release states {published}, but its changes after {} make it {expected}",
                previous.map_or_else(|| "nothing".to_owned(), |p| p.to_string())
            ));
        }
        let manifest_now = cfg.manifest_version()?;
        if manifest_now != published {
            return Err(format!(
                "the release is {published}, but {} states {manifest_now}",
                cfg.manifest_rel()
            ));
        }
        if !after.is_empty() {
            return Err(format!(
                "a release moves every unreleased change into the package publet; {rel} still lists some"
            ));
        }
        println!("release {published}: the bump its changes require, and published");
        return Ok(());
    }
    if !code.is_empty() && added.is_empty() {
        return Err(format!(
            "this change touches {} but records no change in {rel}; \
             run `pubrel add CATEGORY \"what changed\"`",
            code.join(", ")
        ));
    }
    if code.is_empty() {
        println!("ok: no code changed");
    } else {
        println!("ok: {} change(s) recorded", added.len());
    }
    Ok(())
}

fn today() -> Result<String, String> {
    let out = run("date", &["-u", "+%Y-%m-%d"], Path::new("."))?;
    Ok(out.trim().to_owned())
}

fn prepare(open_pr: bool) -> Result<(), String> {
    let cfg = config()?;
    record::require_pub()?;
    if !run("git", &["status", "--porcelain"], &cfg.root)?
        .trim()
        .is_empty()
    {
        return Err("the tree is not clean; a release is cut from a clean main".to_owned());
    }
    run("git", &["fetch", "origin", "main"], &cfg.root)?;
    let head = run("git", &["rev-parse", "HEAD"], &cfg.root)?
        .trim()
        .to_owned();
    let main = run("git", &["rev-parse", "origin/main"], &cfg.root)?
        .trim()
        .to_owned();
    if head != main {
        return Err("HEAD is not origin/main; a release is cut from the current main".to_owned());
    }
    let rows = unreleased(&cfg)?;
    let previous = record::published_version(&cfg, lock_text(&cfg).as_deref())?;
    let release = version::next(previous, cfg.manifest_version()?, &categories(&rows))?;
    let date = today()?;
    let branch = format!("release/v{release}");
    println!(
        "releasing {} {} -> {release} from {}",
        cfg.name,
        previous.map_or_else(|| "(first release)".to_owned(), |p| p.to_string()),
        head.chars().take(12).collect::<String>()
    );
    run("git", &["checkout", "-b", &branch], &cfg.root)?;
    println!(
        "on {branch}; if a step below fails: git checkout -- . && git clean -fd {} && \
         git checkout main && git branch -D {branch}",
        cfg.corpus.display()
    );

    let source = record::package_source(&cfg, release, &date, &head, &rows);
    std::fs::create_dir_all(cfg.package_dir()).map_err(|e| e.to_string())?;
    std::fs::write(
        cfg.package_dir().join("publet.json"),
        serde_json::to_string_pretty(&source).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    record::write_changes(&cfg.root.join(cfg.unreleased_rel()), &[])?;
    let manifest_path = cfg.root.join(&cfg.manifest);
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, manifest::write(&text, cfg.kind, release)?)
        .map_err(|e| e.to_string())?;
    manifest::refresh_lock(&cfg.root, cfg.kind, &cfg.name)?;
    record::run_pub(&cfg.corpus_dir(), &["build"])?;
    print!(
        "{}",
        record::run_pub(&cfg.corpus_dir(), &["publish", &cfg.package])?
    );
    changelog()?;

    run("git", &["add", "-A"], &cfg.root)?;
    let message = format!(
        "Release v{release}\n\nPublishes {} with {} {release}.",
        cfg.package, cfg.name
    );
    run("git", &["commit", "-m", &message], &cfg.root)?;
    if !open_pr {
        println!("committed on {branch}; push it and open a PR to release");
        return Ok(());
    }
    run("git", &["push", "-u", "origin", &branch], &cfg.root)?;
    let notes = notes(&cfg, release)?;
    let pr = run(
        "gh",
        &[
            "pr",
            "create",
            "--base",
            "main",
            "--title",
            &format!("Release v{release}"),
            "--body",
            &notes,
        ],
        &cfg.root,
    )?;
    print!("{pr}");
    Ok(())
}

fn notes(cfg: &Config, release: Version) -> Result<String, String> {
    let found = record::releases(cfg)
        .into_iter()
        .find(|r| r.version == release)
        .ok_or_else(|| format!("no published release record states {release}"))?;
    let mut text = record::section(&found);
    if let Some(install) = &cfg.install {
        let _ = write!(
            text,
            "Install:\n\n```\n{}\n```\n",
            install.replace("{tag}", &format!("v{release}"))
        );
    }
    Ok(text)
}

fn changelog() -> Result<(), String> {
    let cfg = config()?;
    let body: String = record::releases(&cfg).iter().map(record::section).collect();
    let text = format!(
        "# Changelog\n\nGenerated by `pubrel changelog` from the package publet `{}` [{}] in \
         `{}/`. Do not edit: each release is a published version of that publet, and this \
         file is a view of it.\n\n{body}",
        cfg.package,
        cfg.tag,
        cfg.corpus.display()
    );
    std::fs::write(cfg.root.join("CHANGELOG.md"), text).map_err(|e| e.to_string())?;
    println!("CHANGELOG.md written");
    Ok(())
}

fn tag() -> Result<(), String> {
    let cfg = config()?;
    let version = cfg.manifest_version()?;
    let tag = format!("v{version}");
    if !run("git", &["tag", "-l", &tag], &cfg.root)?
        .trim()
        .is_empty()
    {
        println!("{tag} already exists; nothing to do");
        return Ok(());
    }
    let lock = lock_text(&cfg);
    if record::published_version(&cfg, lock.as_deref())? != Some(version) {
        println!(
            "{version} is not a published release of {}; nothing to tag",
            cfg.package
        );
        return Ok(());
    }
    let identity =
        record::locked(lock.as_deref(), &format!("{}#identity", cfg.package)).unwrap_or_default();
    let release =
        record::locked(lock.as_deref(), &format!("{}#release", cfg.package)).unwrap_or_default();
    published::require_signed(&cfg, &identity)?;
    published::require_signed(&cfg, &release)?;
    let annotation = format!(
        "{} {version}\n\npackage {} [{}]\nidentity {identity}\nrelease  {release}",
        cfg.name, cfg.package, cfg.tag
    );
    run("git", &["tag", "-a", &tag, "-m", &annotation], &cfg.root)?;
    run("git", &["push", "origin", &tag], &cfg.root)?;
    let notes = notes(&cfg, version)?;
    run(
        "gh",
        &[
            "release",
            "create",
            &tag,
            "--title",
            &format!("{} {version}", cfg.name),
            "--notes",
            &notes,
        ],
        &cfg.root,
    )?;
    println!("tagged and released {tag}");
    Ok(())
}
