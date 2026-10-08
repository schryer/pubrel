//! Versions, and the bump a set of changes requires.
//!
//! Versions follow Cargo's flavour of semantic versioning, the one
//! `cargo` itself resolves `^` requirements by. From 1.0.0:
//!
//! | category              | meaning                                  | bump  |
//! |-----------------------|------------------------------------------|-------|
//! | `changed`, `removed`  | an already-published interface changed   | major |
//! | `added`               | a new feature                            | minor |
//! | `fixed`, `security`   | the same functionality                   | patch |
//! | `internal`            | code changed, nothing users run did      | none  |
//!
//! Below 1.0.0 every part shifts one place right, as Cargo reads `0.y.z`:
//! a breaking change bumps `y`, and anything compatible -- a feature or a
//! fix -- bumps `z`. `0.3.1` to `0.4.0` breaks; `0.3.1` to `0.3.2` does
//! not, whatever it adds.
//!
//! An `internal` change -- a refactor, a crate moved -- satisfies `check`'s
//! demand that a code change says what it changed, without claiming a fix
//! or a feature that is not there. It never makes a release on its own; it
//! is listed in the next release that something else makes.
//!
//! A package never released before is released at its current manifest
//! version, raised to at least 0.1.0: that is the baseline later releases
//! are measured from.

pub use semver::Version;

/// The categories a change may be recorded under.
pub const CATEGORIES: [&str; 6] = [
    "changed", "removed", "added", "fixed", "security", "internal",
];

/// The lowest first release.
const BASELINE: Version = Version::new(0, 1, 0);

/// Parse a release version: `MAJOR.MINOR.PATCH`, nothing more.
///
/// Releases here have no pre-release or build suffix, so one is refused
/// rather than carried along.
///
/// # Errors
///
/// Returns a message if the text is not three dot-separated integers.
pub fn parse(text: &str) -> Result<Version, String> {
    let bad = || format!("{text:?} is not MAJOR.MINOR.PATCH");
    let version = Version::parse(text.trim()).map_err(|_| bad())?;
    if version.pre.is_empty() && version.build.is_empty() {
        Ok(version)
    } else {
        Err(bad())
    }
}

/// Whether `category` is one changes may be recorded under.
#[must_use]
pub fn known(category: &str) -> bool {
    CATEGORIES.contains(&category)
}

/// The version a release must have.
///
/// `previous` is the last released version, or `None` if the package has
/// never been released, in which case `manifest` -- its current version --
/// is the starting point.
///
/// # Errors
///
/// Returns a message if a category is unknown, or if nothing but internal
/// changes (or nothing at all) is recorded.
pub fn next(
    previous: Option<Version>,
    manifest: Version,
    categories: &[&str],
) -> Result<Version, String> {
    if let Some(unknown) = categories.iter().find(|c| !known(c)) {
        return Err(format!(
            "unknown change category `{unknown}` (known: {})",
            CATEGORIES.join(", ")
        ));
    }
    if categories.is_empty() {
        return Err("nothing has changed, so there is nothing to release".to_owned());
    }
    if categories.iter().all(|c| *c == "internal") {
        return Err(
            "only internal changes are recorded: nothing users run has changed, \
             so there is nothing to release"
                .to_owned(),
        );
    }
    let Some(previous) = previous else {
        return Ok(manifest.max(BASELINE));
    };
    let breaking = categories
        .iter()
        .any(|c| matches!(*c, "changed" | "removed"));
    let feature = categories.contains(&"added");
    let (major, minor, patch) = (previous.major, previous.minor, previous.patch);
    Ok(if major == 0 {
        // Cargo's 0.y.z: y is the breaking part; everything else is z.
        if breaking {
            Version::new(0, minor + 1, 0)
        } else {
            Version::new(0, minor, patch + 1)
        }
    } else if breaking {
        Version::new(major + 1, 0, 0)
    } else if feature {
        Version::new(major, minor + 1, 0)
    } else {
        Version::new(major, minor, patch + 1)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        parse(text).unwrap()
    }

    fn bump(previous: &str, categories: &[&str]) -> Version {
        next(Some(v(previous)), v(previous), categories).unwrap()
    }

    // covers: version::next
    #[test]
    fn from_one_each_category_bumps_its_part() {
        assert_eq!(bump("1.4.2", &["added"]), v("1.5.0"));
        assert_eq!(bump("1.4.2", &["changed"]), v("2.0.0"));
        assert_eq!(bump("1.4.2", &["removed", "fixed"]), v("2.0.0"));
        assert_eq!(bump("1.4.2", &["fixed"]), v("1.4.3"));
        assert_eq!(bump("1.4.2", &["security"]), v("1.4.3"));
        assert_eq!(bump("1.4.2", &["fixed", "added"]), v("1.5.0"));
    }

    // covers: version::next
    #[test]
    fn below_one_a_break_bumps_minor_and_anything_else_patch() {
        assert_eq!(bump("0.3.1", &["changed"]), v("0.4.0"));
        assert_eq!(bump("0.3.1", &["removed", "added"]), v("0.4.0"));
        assert_eq!(bump("0.3.1", &["added"]), v("0.3.2"));
        assert_eq!(bump("0.3.1", &["fixed"]), v("0.3.2"));
        assert_eq!(bump("0.3.1", &["added", "fixed"]), v("0.3.2"));
    }

    // covers: version::next
    #[test]
    fn a_first_release_is_the_manifest_version_at_least_0_1_0() {
        assert_eq!(next(None, v("0.0.1"), &["added"]).unwrap(), v("0.1.0"));
        assert_eq!(next(None, v("0.1.0"), &["fixed"]).unwrap(), v("0.1.0"));
        assert_eq!(next(None, v("0.2.2"), &["changed"]).unwrap(), v("0.2.2"));
    }

    // covers: version::next
    #[test]
    fn internal_changes_never_bump_and_never_release_alone() {
        assert!(next(Some(v("0.1.0")), v("0.1.0"), &["internal"]).is_err());
        assert!(next(None, v("0.1.0"), &["internal", "internal"]).is_err());
        assert_eq!(bump("0.1.0", &["internal", "fixed"]), v("0.1.1"));
        assert_eq!(bump("0.1.0", &["changed", "internal"]), v("0.2.0"));
    }

    // covers: version::next
    #[test]
    fn unknown_or_absent_changes_are_refused() {
        assert!(next(Some(v("0.1.0")), v("0.1.0"), &["oops"]).is_err());
        assert!(next(Some(v("0.1.0")), v("0.1.0"), &[]).is_err());
    }

    // covers: version::parse
    #[test]
    fn only_plain_release_versions_parse() {
        assert_eq!(v("10.2.33").to_string(), "10.2.33");
        assert!(parse("1.2").is_err());
        assert!(parse("1.2.x").is_err());
        assert!(parse("1.0.0-rc.1").is_err());
        assert!(parse("1.0.0+build").is_err());
        assert!(v("0.10.0") > v("0.9.9"));
    }
}
