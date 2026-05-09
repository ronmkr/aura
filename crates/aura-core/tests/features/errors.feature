Feature: Error Management and Self-healing
  As a user with an unstable network
  I want the engine to automatically recover from transient failures
  So that I don't have to manually restart downloads.

  @ADR-0006
  Scenario: Exponential backoff for HTTP 503 errors
    Given an HTTP mirror that is returning "503 Service Unavailable"
    When the "HttpWorker" receives the error
    Then it should wait 2 seconds before the first retry
    And it should wait 4 seconds before the second retry
    And it should mark the source as "Degraded" after 5 attempts

  @ADR-0006
  Scenario: Failover between Metalink sources
    Given a Metalink task with HTTP Mirror A and FTP Mirror B
    When Mirror A returns a "404 Not Found"
    Then the "Orchestrator" should automatically switch all pending ranges to Mirror B
    And Mirror A should be marked as "Failed" in the task metadata

  @ADR-0003
  Scenario: Graceful handling of Disk Full condition
    Given the destination drive has only 10 MB of free space
    When I add a task for a 100 MB file
    Then the "StorageEngine" should fail the pre-allocation
    And the "Orchestrator" should immediately pause the task with "Error: Disk Full"
    And the .aura control file should be preserved to allow resumption after cleanup
