Feature: BitTorrent Swarm and v2 Support
  As a P2P user
  I want to participate in modern swarms
  So that I can download files efficiently and verify their integrity.

  @Decision-0036 @Scenario-2.1
  Scenario: Magnet link metadata maturation
    Given a magnet link with info-hash "..."
    When I add the task
    Then the engine should enter "MetadataExchange" phase
    And it should connect to DHT and PEX to find peers
    And once the info-dict is received, it should transition to "Downloading" phase
    And the total file size should be correctly resolved

  @Decision-0031 @Scenario-2.3
  Scenario: BitTorrent v2 hybrid integrity
    Given a v2 hybrid torrent file
    When a piece is downloaded
    Then the engine should verify the piece against the SHA-256 Merkle tree
    And the piece layer should be persisted in the Sled database
    And corrupted pieces should be immediately discarded and re-requested
