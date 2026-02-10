# ADR-001: Use Tauri for Desktop Application

## Context

The product requires a desktop UI with access to local filesystem while keeping the core logic secure, testable, and portable.

Alternatives considered:

- Electron
- Native GUI frameworks
- Web-only application

## Decision

Use Tauri with a Rust backend and a thin UI layer.

## Rationale

- Smaller attack surface than Electron
- Native performance
- Strong isolation between UI and core logic
- Good fit for Rust-based domain logic

## Consequences

**Positive:**

- Reduced resource usage
- Clear separation of concerns

**Negative:**

- More explicit boundary management
- Rust knowledge required for core development
