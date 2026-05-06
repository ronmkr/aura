# ADR 0042: Internationalization (i18n) Architecture

## Status
Proposed

## Context
Aura is intended for a global audience. Hardcoded English strings in the CLI, TUI, and logs make it difficult for non-English speakers to use the tool effectively.

## Decision
1. **Localization Crate**: We will adopt the `fluent` crate (by Mozilla) for localization. Fluent provides a powerful, asynchronous-friendly way to handle translations with support for pluralization and gender.
2. **External Resource Files**: All user-facing strings will be moved to `.ftl` files in a `locales/` directory.
3. **Context-Aware Translation**: The **Persona Switcher** will detect the user's system locale and initialize the appropriate `FluentBundle`.
4. **Zero-Copy Keys**: We will use a macro-based system or compile-time keys to ensure that translation lookups have minimal runtime overhead.

## Alternatives Considered
- **Gettext**: Industry standard but uses an older format and is less idiomatic in modern Rust.
- **Hardcoded Maps**: Difficult to maintain and lacks advanced features like pluralization.

## Consequences
- **Pros**: Reach a global user base, clean separation of code and content, and professional UI/UX.
- **Cons**: Significant refactoring required to externalize all existing strings; slightly larger binary size due to resource embedding.
