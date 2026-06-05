Feature: BitTorrent Seeding Limits Configuration and Overrides
  As a BitTorrent user
  I want to define global seeding limits in the config file and override them on a per-task basis
  So that I have precise control over seeding ratio and time limits.

  Scenario: Loading seeding limits from configuration
    Given a configuration file loaded with:
      """
      [bittorrent]
      enabled = true
      [bittorrent.seeding]
      min_ratio = 1.8
      max_seeding_time = 7200
      stop_on_either = false
      """
    Then the resolved configuration seeding ratio should be 1.8 and max_seeding_time should be 7200 and stop_on_either should be false

  Scenario: Task seeding overrides via changeOption
    Given a running engine
    When I add a mock BitTorrent task with ID 100
    Then the task 100 should have no seeding overrides
    When I change options for task 100 with:
      | Option     | Value |
      | seed-ratio | 2.5   |
      | seed-time  | 120   |
    Then the task 100 should have seed_ratio 2.5 and seed_time 120
