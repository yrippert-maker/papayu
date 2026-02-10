# ADR-002: Structured PATCH/EDIT (v3) with Golden Traces

## Context

The system performs automated file modifications driven by LLM output. Naive diff-based approaches led to nondeterministic and hard-to-debug behavior.

## Decision

Introduce structured PATCH/EDIT protocol (v3) and lock behavior using golden traces.

## Rationale

- Deterministic behavior is more valuable than flexibility
- Golden traces provide regression safety
- Protocol versioning allows evolution without breaking behavior

## Consequences

**Positive:**

- Predictable edits
- Easier debugging
- Strong regression detection

**Negative:**

- More rigid protocol
- Higher upfront complexity
