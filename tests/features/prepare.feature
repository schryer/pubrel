Feature: A release is cut from main as a pull request
  `pubrel prepare` moves the unreleased changes into the package publet,
  publishes it, bumps the manifest, writes the changelog, and opens a
  release PR. Nothing reaches main except through that PR.

  Scenario Outline: Each manifest kind is bumped
    Given a <kind> package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the manifest states "0.2.0"
    And the published release record states "0.2.0" with "added"
    And the unreleased list is empty
    And CHANGELOG.md has a section "## 0.2.0"
    And a pull request "Release v0.2.0" was requested from "release/v0.2.0"

    Examples:
      | kind            |
      | cargo           |
      | cargo-workspace |
      | pyproject       |

  Scenario: A workspace's internal dependencies move with it
    Given a cargo-workspace package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then the manifest pins its internal dependency at "0.1.1"

  Scenario: A dirty tree is refused
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    And an uncommitted edit
    When I run pubrel prepare
    Then it fails
    And stderr mentions "not clean"
