# Technical Investment Memo — papa-yu

## 1. Executive Summary

papa-yu is a desktop application built with Tauri and Rust, designed to orchestrate LLM-driven workflows involving structured file editing (PATCH/EDIT) and controlled external research.

The project demonstrates a high level of technical maturity:

- deterministic behavior enforced via protocol versioning and golden traces
- strong CI/CD quality gates
- explicit security controls around network access (SSRF-safe design)
- clear separation between UI, domain logic, and IO

The codebase is maintainable, testable, and transferable with moderate onboarding effort. No critical technical blockers for further development or transfer of ownership were identified.

---

## 2. Product Overview (Technical Perspective)

### Purpose

The system automates and orchestrates complex workflows driven by LLM output, with a focus on reproducibility, safety, and long-term maintainability.

### Target usage

- Desktop environments
- Controlled workloads (non–real-time, non–high-concurrency)
- Users requiring deterministic behavior over flexibility

### Explicit non-goals

- Server-side, high-concurrency workloads
- Real-time processing
- Execution of untrusted plugins

(See `docs/LIMITS.md` for details.)

---

## 3. Architecture Overview

### High-level design

- Desktop application using Tauri
- Core logic implemented in Rust
- UI is a thin client without direct filesystem or network access

### Key architectural principles

- All IO is centralized and controlled
- Domain logic is isolated from side effects
- Observable behavior is locked via golden traces

### Core modules

- `net` — single entry point for outbound network access with SSRF protection
- `llm_planner` — orchestration and planning logic
- `online_research` — external data integration via safe adapters
- `commands/*` — Tauri boundary layer

Architecture documentation is available in `docs/ARCHITECTURE.md`.

---

## 4. Code Quality and Testing

### Testing strategy

- >100 automated tests
- Golden traces for protocol versions v1, v2, v3
- Regression detection is enforced in CI

### CI/CD

- Formatting and linting enforced (`cargo fmt`, `clippy`)
- Automated test execution
- Dependency vulnerability scanning (`cargo audit`)
- Reproducible builds from a clean checkout

The CI pipeline serves as a hard quality gate.

---

## 5. Security Posture (Design & Code Level)

Security is addressed at the architectural level:

- Centralized network access via `net::fetch_url_safe`
- SSRF mitigations:
  - scheme allowlist (http, https)
  - denial of private/loopback IP ranges
  - request size limit (1 MB)
  - timeout (15 seconds)
- No secrets stored in the repository
- Dependency vulnerability scanning in CI

**Scope limitation:**

- No penetration testing performed
- Security review limited to design and code analysis

(See `docs/adr/ADR-003-ssrf.md` for rationale.)

---

## 6. Dependencies and Supply Chain

- Dependencies are locked via `Cargo.lock` and `package-lock.json`
- Automated vulnerability scanning is enabled
- Planned addition: license policy enforcement via `cargo deny`

No known blocking license risks identified at this stage.

---

## 7. Operational Maturity

- Project can be built and run via documented steps
- Common failure modes are documented in `docs/INCIDENTS.md`
- Deterministic behavior simplifies debugging and reproduction
- Runbook documentation (`docs/RUNBOOK.md`) provides basic operational guidance

---

## 8. Known Risks and Technical Debt

Known risks are explicitly documented:

- Sensitivity of LLM planning to malformed input
- Rigid PATCH/EDIT protocol trade-offs
- Desktop-centric architecture limits scalability

Technical debt is tracked and intentional where present. No unbounded or hidden debt has been identified.

---

## 9. Roadmap (Technical)

### Short-term

- License policy enforcement (`cargo deny`)
- Further documentation hardening

### Mid-term

- Reduction of bus-factor through onboarding exercises
- Optional expansion of test coverage in edge cases

### Long-term

- Additional protocol versions
- New research adapters via existing extension points

---

## 10. Transferability Assessment

From a technical perspective:

- The system is explainable within days, not weeks
- No single undocumented "magic" components exist
- Ownership transfer risk is considered low to moderate

Overall technical readiness supports both continued independent development and potential acquisition.
