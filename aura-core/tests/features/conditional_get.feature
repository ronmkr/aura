Feature: Conditional GET for Incremental Refresh

  Scenario: Conditional GET with ETag returns 304 Not Modified
    Given a mock HTTP server with ETag "etag-123" that returns 304 on match
    When I start the engine and add the task
    And I wait for the task to complete
    When I refresh the task
    Then the mock server should have received "If-None-Match" header
    And the task should emit a NotModified event
