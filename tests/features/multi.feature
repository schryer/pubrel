Feature: One repository releases several packages
  release.json may list `packages`: here a workspace, `demo`, and a crate
  in it released with a version of its own, `demo-algo`. Each has its own
  unreleased list, package publet, changelog and tag prefix; a workspace
  bump never moves the other crate's pin, and the crate's own release does.

  Background:
    Given a workspace whose "demo" and "demo-algo" are released at version "0.1.0"

  Scenario: Each package is tagged with its own prefix
    When I run pubrel tag
    Then it succeeds
    And the tag "v0.1.0" exists
    And the tag "demo-algo-v0.1.0" exists
    And a GitHub release "demo-algo-v0.1.0" was requested

  Scenario: The crate with its own version is released alone, and pinned
    When "demo-algo" records "added" "a faster sort" and is released
    Then the release branch is "release/demo-algo-v0.1.1"
    And "crates/algo/Cargo.toml" states "0.1.1"
    And the workspace states "0.1.0"
    And the workspace pins "crates/algo" at "0.1.1"
    And the workspace pins "crates/core" at "0.1.0"

  Scenario: A workspace bump leaves the other crate's pin alone, even at the same version
    When "demo" records "fixed" "a crash" and is released
    Then the workspace states "0.1.1"
    And the workspace pins "crates/core" at "0.1.1"
    And the workspace pins "crates/algo" at "0.1.0"
    And "crates/algo/Cargo.toml" states "0.1.0"

  Scenario: A release of one package passes check
    When "demo-algo" records "added" "a faster sort" and its release is cut
    And I run pubrel check against main
    Then it succeeds
    And it prints "demo-algo: release 0.1.1"

  Scenario: A change to a crate both packages ship is recorded for both
    When a branch changes "crates/algo/src/lib.rs" and records "fixed" "an overflow" for "demo-algo"
    And I run pubrel check against main
    Then it fails
    And stderr mentions "demo: this change touches crates/algo/src/lib.rs"
    And stderr mentions "pubrel add --package demo"

  Scenario: Recorded for both, the change passes
    When a branch changes "crates/algo/src/lib.rs" and records "fixed" "an overflow" for "demo-algo"
    And it also records "fixed" "an overflow in demo-algo" for "demo"
    And I run pubrel check against main
    Then it succeeds
    And it prints "demo-algo: ok: 1 change(s) recorded"
    And it prints "demo: ok: 1 change(s) recorded"

  Scenario: A command that acts on one package must be told which
    When I run pubrel add "fixed" "something" without naming a package
    Then it fails
    And stderr mentions "several packages (demo, demo-algo)"

  Scenario: next reports every package
    When "demo-algo" records "fixed" "an overflow" on main
    And I run pubrel next
    Then it succeeds
    And it prints "demo-algo 0.1.1"
    And it prints "demo: nothing has changed"

  Scenario: Each package has its own changelog
    When I run pubrel changelog
    Then it succeeds
    And "CHANGELOG.md" names the package publet "pkg.demo"
    And "crates/algo/CHANGELOG.md" names the package publet "pkg.demo-algo"
