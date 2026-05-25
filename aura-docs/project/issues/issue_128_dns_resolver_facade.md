---
title      : "Bug: DNS resolver config is a facade — DoH/DoT not wired"
labels     : [type:bug, priority:moderate, area:network]
description: |
  `net_util/logic.rs` contains a `create_resolver()` function that accepts a `ResolverConfig` enum with variants `System`, `Cloudflare`, `Google`, and `Custom(url)`. However, ALL variants create the same system resolver via `TokioResolver::builder_tokio()`. The `hickory-resolver` crate (with `https-aws-lc-rs` feature) supports DoH, but it is never configured.

  The `config/logic.rs` has a `dns_resolver` field in `NetworkConfig` that maps to this enum, but changing the config value has zero effect.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Wire `ResolverConfig::Cloudflare` to `hickory_resolver::config::ResolverConfig::cloudflare_https()`.
  - Wire `ResolverConfig::Google` to `hickory_resolver::config::ResolverConfig::google_https()`.
  - Wire `ResolverConfig::Custom(url)` to a custom DoH endpoint.
  - Add `[dns]` section to `Aura.example.toml` documenting the options.
  - Add integration test verifying resolver selection.
---
