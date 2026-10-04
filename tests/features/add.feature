Feature: Changes are recorded as they are made
  Every change to what users run is recorded, with the category that
  decides the next version, in the package's unreleased list.

  Background:
    Given a released-never cargo package at version "0.1.0"

  Scenario: A change is appended under a known category
    When I run pubrel add "added" "a new subcommand"
    Then it succeeds
    And the unreleased list holds "added" "a new subcommand"

  Scenario: An unknown category is refused
    When I run pubrel add "improved" "something"
    Then it fails
    And stderr mentions "unknown category"
