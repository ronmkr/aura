Feature: Multi-source Protocol Aggregation
  As a user with limited bandwidth from a single source
  I want to download a file from multiple protocols simultaneously
  So that I can maximize my total throughput.

  @ADR-0023 @Milestone-4
  Scenario: Download a file via Metalink with mixed mirrors
    Given a Metalink file "test.metalink" containing:
      | Protocol | URI                                           |
      | HTTP     | https://mirror1.com/file.zip                  |
      | FTP      | ftp://mirror2.com/file.zip                   |
    And the global download limit is unlimited
    When I add the task via "test.metalink"
    Then the engine should spawn 1 HTTP worker and 1 FTP worker
    And the downloaded data should be aggregated into "file.zip"
    And the final file "file.zip" should pass SHA-256 verification

  @ADR-0005
  Scenario: Work Stealing from a lagging mirror
    Given a download task with 2 HTTP mirrors
    And Mirror A is throttled to 10 KB/s
    And Mirror B is unlimited
    When the download starts
    Then the engine should detect Mirror A is lagging
    And Mirror B should "steal" the remaining ranges assigned to Mirror A
    And the download should complete without waiting for Mirror A
