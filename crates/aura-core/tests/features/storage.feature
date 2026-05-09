Feature: Storage Performance and Integrity
  As a user
  I want my files to be saved safely and efficiently
  So that my disk health is protected and data is never corrupt.

  @ADR-0003 @Scenario-5.1
  Scenario: Atomic file completion
    Given a new download task for "movie.mp4"
    When the download is in progress
    Then a file named "movie.mp4.part" should exist in the download directory
    And "movie.mp4" should NOT exist
    When the download reaches 100% and integrity is verified
    Then "movie.mp4.part" should be renamed to "movie.mp4"
    And the .aura control file should be deleted

  @ADR-0002 @Scenario-5.2
  Scenario: Sequential Write Aggregation
    Given a BitTorrent swarm delivering pieces out of order
    When Piece 5 (10MB-20MB) arrives before Piece 4 (0MB-10MB)
    Then the Storage Engine should buffer Piece 5 in memory
    When Piece 4 arrives and is written
    Then the Storage Engine should immediately flush Piece 5 to disk
    And the disk seek count should be minimized
