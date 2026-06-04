Feature: Hierarchical Configuration and CLI Overrides
  As an administrator configuring Aura
  I want to define settings in a hierarchy of files and override them via the CLI
  So that I have precise control over the engine behavior.

  Scenario: Hierarchical file resolution and CLI overrides
    Given a mock environment with user config file at "~/.config/aura/Aura.toml" containing:
      """
      [network]
      listen_port = 8080
      rpc_port = 9090
      """
    And a local config file at "./Aura.toml" containing:
      """
      [network]
      listen_port = 8888
      """
    When I resolve the configuration with custom config path "None"
    Then the resolved configuration should use local config port 8888 and user config rpc_port 9090

  Scenario: Overriding config via CLI options
    Given a configuration file loaded with:
      """
      [storage]
      download_dir = "/default/path"
      [bandwidth]
      global_download_limit = 1000
      """
    When I apply CLI overrides:
      | Option       | Value          |
      | download_dir | /custom/path   |
      | limit        | 5000           |
    Then the final configuration should use download_dir "/custom/path" and global_download_limit 5000
