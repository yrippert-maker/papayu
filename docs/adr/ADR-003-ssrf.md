# ADR-003: Centralized Network Access and SSRF Protection

## Context

The application performs external fetch operations based on user or LLM input. Uncontrolled network access introduces SSRF and data exfiltration risks.

## Decision

All network access must go through a single module (`net`) with explicit safety controls.

## Controls

- Allowlisted schemes (http, https)
- Deny private and loopback IP ranges (RFC1918, link-local)
- Request size limit (1 MB)
- Timeout (15 s)
- Reject URL with `user:pass@`

## Consequences

**Positive:**

- Eliminates a large class of security vulnerabilities
- Centralized policy enforcement

**Negative:**

- Less flexibility for ad-hoc network calls
- Requires discipline when adding new features
