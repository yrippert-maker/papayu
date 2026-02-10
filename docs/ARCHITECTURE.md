# Architecture Overview — papa-yu

## 1. Purpose

papa-yu is a desktop application built with Tauri.  
Its goal is to orchestrate LLM-driven workflows involving local files, structured editing (PATCH/EDIT), and controlled external research.

The system prioritizes:

- deterministic behavior
- reproducibility (golden traces)
- controlled IO and network access
- long-term maintainability

---

## 2. High-level architecture

- Desktop application (Tauri)
- Core logic implemented in Rust
- UI acts as a thin client
- All critical logic resides in the Rust backend

**Key principle:**  
UI never performs filesystem or network operations directly.

---

## 3. Core modules

### 3.1 net

**Location:** `src-tauri/src/net.rs`

**Responsibilities:**

- Single entry point for all outbound network access
- SSRF protection
- Request limits (timeout, size)
- Explicit allow/deny rules

**Constraints:**

- No direct `reqwest::Client::get()` usage outside this module
- All fetch operations go through `fetch_url_safe`

---

### 3.2 llm_planner

**Responsibilities:**

- Planning and orchestration of LLM-driven workflows
- Translating user intent into structured operations
- Managing execution order and context

**Known risks:**

- Sensitive to malformed prompts
- Requires deterministic input for reproducible behavior

---

### 3.3 online_research

**Responsibilities:**

- External information retrieval
- Adapter layer over `net::fetch_url_safe`
- Re-export of safe network primitives

**Design note:** Acts as an integration boundary, not business logic.

---

### 3.4 commands/*

**Responsibilities:**

- Tauri command boundary
- Validation of input coming from UI
- Delegation to internal services

**Constraints:**

- No business logic
- No direct filesystem or network access

---

## 4. Data flow (simplified)

```
UI → Tauri command → domain/service logic → adapters (fs / net) → result returned to UI
```

---

## 5. Protocol versions and determinism

- Multiple protocol versions (v1, v2, v3)
- Golden traces used to lock observable behavior
- Protocol changes are versioned explicitly

This enables:

- regression detection
- reproducible behavior across releases

---

## 6. Architectural boundaries (hard rules)

- Domain logic must not perform IO directly
- All network access must go through `net`
- Filesystem access is centralized
- Side effects are isolated and testable

Violations are treated as architectural defects.

---

## 7. Extension points

- New research sources via `online_research`
- New protocol versions
- Additional planners or execution strategies

---

## 8. Known limitations

- Not designed for real-time or high-concurrency workloads
- Desktop-oriented architecture
- Relies on deterministic execution context for PATCH/EDIT

See `LIMITS.md` for details.
