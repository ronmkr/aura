Feature: Daemon and RPC Orchestration
  As a developer building tools on top of Aura
  I want a consistent and synchronized RPC interface
  So that my UI always reflects the true engine state.

  @Decision-0016 @Milestone-5
  Scenario: Multi-client state synchronization
    Given the "Aura-daemon" is running
    And Client A (CLI) and Client B (TUI) are both connected via JSON-RPC
    When Client A sends a "Pause" command for Task 1
    Then the Daemon should broadcast the "TaskPaused" event to the Event Bus
    And both Client A and Client B should receive the update within 500ms
    And both clients should show the task as "Paused"

  @Decision-0014
  Scenario: Secure RPC Authentication
    Given the daemon is configured with an "rpc_secret"
    When a client attempts to connect without a token
    Then the daemon should reject the request with "401 Unauthorized"
    When a client provides a valid "X-Aura-Token"
    Then the daemon should allow "aura.addUri" commands
