//! Versions, and the bump a set of changes requires.
//!
//! The rules, mechanically:
//!
//! | category              | meaning                                  | bump  |
//! |-----------------------|------------------------------------------|-------|
//! | `changed`, `removed`  | an already-published interface changed   | major |
//! | `added`               | a new feature                            | minor |
//! | `fixed`, `security`   | the same functionality                   | patch |
//!
//! A package never released before is released at its current manifest
//! version, raised to at least 0.1.0: that is the baseline later releases
//! are measured from.

use std::fmt;

/// The categories a change may be recorded under.
pub const CATEGORIES: [&str; 5] = ["changed", "removed", "added", "fixed", "security"];

/// The lowest first release.
const BASELINE: Version = Version {
    major: 0,
    minor: 1,
    patch: 0,
};

/// A `MAJOR.MINOR.PATCH` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// Parse `MAJOR.MINOR.PATCH`.
    ///
    /// # Errors
    ///
    /// Returns a message if the text is not three dot-separated integers.
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.trim().split('.').collect();
        let bad = || format!("{text:?} is not MAJOR.MINOR.PATCH");
        let [major, minor, patch] = parts.as_slice() else {
            return Err(bad());
        };
        let num = |s: &str| s.parse::<u64>().map_err(|_| bad());
        Ok(Self {
            major: num(major)?,
            minor: num(minor)?,
            patch: num(patch)?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
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
/// Returns a message if a category is unknown or there are none.
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
    let Some(previous) = previous else {
        return Ok(manifest.max(BASELINE));
    };
    let major = categories
        .iter()
        .any(|c| matches!(*c, "changed" | "removed"));
    let minor = categories.contains(&"added");
    Ok(if major {
        Version {
            major: previous.major + 1,
            minor: 0,
            patch: 0,
        }
    } else if minor {
        Version {
            major: previous.major,
            minor: previous.minor + 1,
            patch: 0,
        }
    } else {
        Version {
            patch: previous.patch + 1,
            ..previous
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn each_category_bumps_its_part() {
        let prev = Some(v("0.1.0"));
        let m = v("0.1.0");
        assert_eq!(next(prev, m, &["added"]).unwrap(), v("0.2.0"));
        assert_eq!(next(prev, m, &["changed"]).unwrap(), v("1.0.0"));
        assert_eq!(next(prev, m, &["removed", "fixed"]).unwrap(), v("1.0.0"));
        assert_eq!(next(prev, m, &["fixed"]).unwrap(), v("0.1.1"));
        assert_eq!(
            next(Some(v("1.4.2")), m, &["security"]).unwrap(),
            v("1.4.3")
        );
        assert_eq!(next(prev, m, &["fixed", "added"]).unwrap(), v("0.2.0"));
    }

    #[test]
    fn a_first_release_is_the_manifest_version_at_least_0_1_0() {
        assert_eq!(next(None, v("0.0.1"), &["added"]).unwrap(), v("0.1.0"));
        assert_eq!(next(None, v("0.1.0"), &["fixed"]).unwrap(), v("0.1.0"));
        assert_eq!(next(None, v("0.2.2"), &["changed"]).unwrap(), v("0.2.2"));
    }

    #[test]
    fn unknown_or_absent_changes_are_refused() {
        assert!(next(Some(v("0.1.0")), v("0.1.0"), &["oops"]).is_err());
        assert!(next(Some(v("0.1.0")), v("0.1.0"), &[]).is_err());
    }

    #[test]
    fn versions_parse_and_print() {
        assert_eq!(v("10.2.33").to_string(), "10.2.33");
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.x").is_err());
    }
}
