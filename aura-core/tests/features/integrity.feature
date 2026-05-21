Feature: Non-Swarm Integrity Verification
  As a user downloading files via HTTP or FTP
  I want the engine to verify the file checksum after download
  So that I can be sure the data is not corrupted.

  @ADR-0041
  Scenario: Successful SHA-256 verification
    Given an HTTP mirror for "integrity.bin" with content "A"
    And the expected SHA-256 checksum is "559aead08264d5795d3909718cdd05abd49572e84fe55590eef31a88a08fdffd"
    When I add the task with the checksum
    Then the download should transition to "Verifying" phase after 100%
    And the task should eventually be "Complete"

  Scenario: Failed SHA-256 verification
    Given an HTTP mirror for "corrupt.bin" with content "B"
    And the expected SHA-256 checksum is "559aead08264d5795d3909718cdd05abd49572e84fe55590eef31a88a08fdffd"
    When I add the task with the checksum
    Then the task should eventually fail with a "Checksum mismatch" error
    And the "corrupt.bin" file should be preserved
