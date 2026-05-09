Feature: Persistence and Reliability
  As a user downloading large files
  I want the engine to be resilient to interruptions
  So that I don't lose progress during crashes or network drops.

  @ADR-0017 @Scenario-4.1
  Scenario: Pause and Resume download
    Given an active download at 50% completion
    When I send the "Pause" command
    Then the .aura control file should be updated with current bitfield
    And all active workers should stop
    When I send the "Resume" command
    Then the engine should reload the .aura file
    And download should continue from 50% without re-downloading existing chunks

  @ADR-0035 @Scenario-4.3
  Scenario: VPN Kill-switch protection
    Given the network interface is set to "tun0"
    And the VPN kill-switch is "Enabled"
    When the "tun0" interface becomes unavailable
    Then the engine should immediately pause all active tasks
    And no data should be sent over the default interface
    And a warning should be logged to the telemetry bus
