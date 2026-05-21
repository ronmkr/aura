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

  Scenario: Successful MD5 verification
    Given an HTTP mirror for "integrity.md5" with content "C"
    And the expected MD5 checksum is "0d61f8370cad1d412f80b84d143e1257"
    When I add the task with the checksum
    Then the task should eventually be "Complete"

  Scenario: Successful SHA-512 verification
    Given an HTTP mirror for "integrity.sha512" with content "D"
    And the expected SHA-512 checksum is "2ac968752f624be3e3df46764b51b7831feb70d40307df5d587d4793bffeaf8b4042a1fd6d465df2aacc3304328d431ef10e083baf690b8cc535480a4fef092f"
    When I add the task with the checksum
    Then the task should eventually be "Complete"
