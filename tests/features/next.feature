Feature: The version follows from what changed
  changed or removed: major; added: minor; fixed or security: patch. A
  package never released is first released at its manifest version, raised
  to at least 0.1.0.

  Scenario Outline: A first release is the manifest version, at least 0.1.0
    Given a released-never cargo package at version "<manifest>"
    And the unreleased changes "fixed"
    When I run pubrel next
    Then it prints "<first>"

    Examples:
      | manifest | first |
      | 0.0.1    | 0.1.0 |
      | 0.1.0    | 0.1.0 |
      | 0.2.2    | 0.2.2 |

  Scenario Outline: After a release, each category bumps its part
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "<categories>"
    When I run pubrel next
    Then it prints "<next>"

    Examples:
      | categories    | next  |
      | added         | 0.2.0 |
      | changed       | 1.0.0 |
      | fixed,removed | 1.0.0 |
      | fixed         | 0.1.1 |
      | security      | 0.1.1 |

  Scenario: Nothing to release is refused
    Given a cargo package released at version "0.1.0"
    When I run pubrel next
    Then it fails
    And stderr mentions "nothing has changed"
