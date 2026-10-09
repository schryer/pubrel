Feature: A release cut but never merged can be withdrawn
  `prepare` publishes into pub's local store, and closing the release's
  pull request removes its objects from git but not from the store. pub
  exports what it holds, so the next release would carry the abandoned
  one's records too. `pubrel withdraw` lists that release's objects in
  `.draft-discards`, which pub never exports, and keeps the file out of
  git. `prepare` and `check` refuse a release carrying another attempt's
  records.

  Background:
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    And a release is cut and its branch abandoned as "abandoned"

  Scenario: Withdrawn, the next release carries only its own records
    Given main moves on with "fixed" "a second fix"
    When I run pubrel withdraw "abandoned"
    Then it succeeds
    And it prints "withdrew abandoned"
    And .draft-discards is ignored by git
    When the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"
    And the release adds no object the abandoned branch added
    And pubrel check against main passes on the release branch

  Scenario: Not withdrawn, prepare refuses to publish the abandoned records
    Given main moves on with "fixed" "a second fix"
    When I run pubrel prepare
    Then it fails
    And stderr mentions "pub exported records of another attempt at a release"
    And stderr mentions "pubrel withdraw PR"

  Scenario: A release carrying another attempt's records fails check
    Given main moves on with "fixed" "a second fix"
    And pubrel withdraw "abandoned" has run
    When the release is cut again
    And the abandoned branch's objects are added to the release branch
    And I run pubrel check against main
    Then it fails
    And stderr mentions "this release also adds records of another attempt at a release"

  Scenario: A release cut again unchanged, after withdrawing, says what to undo
    Given pubrel withdraw "abandoned" has run
    When I run pubrel prepare
    Then it fails
    And stderr mentions ".draft-discards; remove that line and prepare again"

  Scenario: A merged release cannot be withdrawn
    When I run pubrel withdraw "main"
    Then it fails
    And stderr mentions "a merged release cannot be withdrawn"

  Scenario: A branch that is not a release is not withdrawn
    Given a branch "feature" that changes only "src/lib.rs"
    When I run pubrel withdraw "feature"
    Then it fails
    And stderr mentions "adds no objects"
