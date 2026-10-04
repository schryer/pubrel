Feature: CI tags what was published, and only that
  On main, a manifest version the package publet records as released gets
  a tag naming the package publet, and a GitHub release.

  Scenario: A published release is tagged and released
    Given a cargo package released at version "0.1.0"
    When I run pubrel tag
    Then it succeeds
    And the tag "v0.1.0" exists and names the package publet
    And a GitHub release "v0.1.0" was requested

  Scenario: An existing tag is left alone
    Given a cargo package released at version "0.1.0"
    And the tag "v0.1.0" already exists
    When I run pubrel tag
    Then it succeeds
    And it prints "already exists"

  Scenario: An unpublished version is not tagged
    Given a released-never cargo package at version "0.1.0"
    When I run pubrel tag
    Then it succeeds
    And it prints "not a published release"
    And no tag exists
