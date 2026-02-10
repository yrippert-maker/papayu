# Runbook — papa-yu

## Build

### Requirements

- Node.js 18+
- Rust 1.70+
- npm

### One-command build

```bash
cd papa-yu
npm install
npm run tauri build
```

Из корня: `cd src-tauri && cargo build --release` (только бэкенд).

---

## Run

### Development

```bash
npm run tauri dev
```

Поднимает Vite и Tauri. Интерфейс доступен в окне приложения.

**Важно:** не открывать скомпилированный .app без dev-сервера — фронт не загрузится.

### Production

Собранный бинарник: `src-tauri/target/release/` (или через `npm run tauri build`).

---

## Where logs are

- **Traces:** `.papa-yu/traces/*.json` (при `PAPAYU_TRACE=1`)
- **Stderr:** события LLM, apply, fallback — в консоль/терминал
- **Weekly report:** агрегация из traces

---

## Common issues

### Golden traces mismatch

**Симптом:** `cargo test golden_traces` падает с ошибкой schema_hash.

**Причина:** изменён `llm_response_schema_v*.json`.

**Действие:** пересчитать SHA256 схемы, обновить `schema_hash` во всех фикстурах в `docs/golden_traces/v*/*.json`.

---

### LLM planner instability

**Симптом:** невалидный JSON, ERR_SCHEMA_VALIDATION, частые repair.

**Причина:** модель не держит strict JSON, или промпт перегружен.

**Действие:** включить `PAPAYU_LLM_STRICT_JSON=1` (если провайдер поддерживает); уменьшить контекст; проверить `PAPAYU_CONTEXT_MAX_*`.

---

### PATCH/EDIT conflicts

**Симптом:** ERR_EDIT_ANCHOR_NOT_FOUND, ERR_EDIT_BEFORE_NOT_FOUND, ERR_EDIT_AMBIGUOUS.

**Причина:** anchor/before не соответствуют текущему содержимому файла.

**Действие:** см. `docs/EDIT_FILE_DEBUG.md`. Убедиться, что FILE-блоки в контексте включают sha256 (v2/v3).

---

### "Could not fetch a valid…" (UI)

**Симптом:** пустое окно при запуске.

**Причина:** фронт не загрузился (Vite не поднят).

**Действие:** запускать только `npm run tauri dev`, не открывать .app напрямую.

---

## Diagnostics

- **Проверить протокол:** `PAPAYU_PROTOCOL_VERSION=3` для EDIT_FILE.
- **Воспроизведение:** включить `PAPAYU_TRACE=1`, выполнить сценарий, смотреть `.papa-yu/traces/`.
- **Тесты:** `cd src-tauri && cargo test` — полный прогон.
- **CI:** `cargo fmt --check`, `cargo clippy`, `cargo audit`, `cargo test`.
