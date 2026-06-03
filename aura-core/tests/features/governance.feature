Feature: Resource Governance and Throttling
  As a system administrator
  I want to limit the bandwidth usage of Aura
  So that it doesn't interfere with other network activities.

  @ADR-0009 @Scenario-3.1
  Scenario: Global download speed limit
    Given the configuration "global_download_limit" is set to "51200" (50 KB/s)
    When I start a high-speed HTTP download
    Then the EWMA throughput should not exceed 55 KB/s over any 5-second window
    And the workers should wait for tokens from the global bucket before network reads

  @ADR-0009 @Scenario-3.2
  Scenario: Hierarchical task-level throttling
    Given the global download limit is "204800" (200 KB/s)
    And Task A has a per-task limit of "51200" (50 KB/s)
    When I start Task A
    Then Task A should be capped at 50 KB/s
    And the global bucket should still have remaining capacity

  @ADR-0023
  Scenario: Adaptive connection scaling for slow servers
    Given an HTTP server that caps per-connection speed to 100 KB/s
    And the "max_connections_per_task" is set to "8"
    And the "global_download_limit" is "1048576" (1 MB/s)
    When the download starts with 1 connection
    Then the Orchestrator should detect throughput is below the global potential
    And the Orchestrator should scale the subtask to 8 concurrent connections

  Scenario: Task dependency chain unblocking
    Given Task B depends on Task A
    When both tasks are added to the Orchestrator
    Then Task B should start in the "Waiting" phase
    When Task A completes
    Then Task B should automatically transition to the "Downloading" phase
