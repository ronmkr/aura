Feature: Integrated Hook System
  Scenario: Execute script on task completion
    Given a hook script "notify.sh" that writes task ID to "hook_output.txt"
    And the configuration "on_download_complete" is set to "sh notify.sh"
    When a download task completes
    Then the file "hook_output.txt" should contain the task ID
