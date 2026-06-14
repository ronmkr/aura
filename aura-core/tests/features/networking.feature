Feature: Advanced Networking and Privacy
  As a privacy-conscious user
  I want my traffic to be routed through proxies and verified interfaces
  So that my IP and location are protected.

  @Decision-0012
  Scenario: SOCKS5 Proxy for BitTorrent Swarm
    Given a global SOCKS5 proxy is configured at "127.0.0.1:9050"
    When I add a BitTorrent task
    Then the "BtWorker" should establish all peer connections via the proxy
    And the Tracker "announce" request should include the proxy credentials

  @Decision-0025
  Scenario: Automatic NAT Port Mapping
    Given the engine is behind a UPnP-capable router
    When the "NatActor" starts
    Then it should request a port mapping for the "listen_port" (6881)
    And it should periodically refresh the mapping before it expires
    And if UPnP fails, it should fallback to NAT-PMP

  @Decision-0026
  Scenario: Happy Eyeballs (Dual-stack Connectivity)
    Given a mirror that supports both IPv4 and IPv6
    When a "ProtocolWorker" initiates a connection
    Then it should attempt to connect to both addresses in parallel
    And it should use the first one that successfully completes the handshake
    And it should cancel the lagging attempt immediately
