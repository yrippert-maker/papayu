# Golden traces — эталонные артефакты

Фиксируют детерминированные результаты papa-yu без зависимости от LLM.
Позволяют ловить регрессии в валидации, парсинге, диете, кеше.

## Структура

```
docs/golden_traces/
  README.md
  v1/                  # Protocol v1 fixtures
    001_fix_bug_plan.json
    002_fix_bug_apply.json
    ...
  v2/                  # Protocol v2 fixtures (PATCH_FILE, base_sha256)
    001_fix_bug_plan.json
    002_fix_bug_apply_patch.json
    003_base_mismatch_block.json
    004_patch_apply_failed_block.json
    005_no_changes_apply.json
```

## Формат fixture (без секретов)

Минимальный стабильный JSON:
- `protocol` — schema_version, schema_hash
- `request` — mode, input_chars, token_budget, strict_json, provider, model
- `context` — context_digest (опц.), context_stats, cache_stats
- `result` — validated_json (объект), validation_outcome, error_code

Без raw_content, без секретов.

## Генерация из трасс

```bash
cd src-tauri
cargo run --bin trace_to_golden -- <trace_id> [output_path]
cargo run --bin trace_to_golden -- <path/to/trace.json> [output_path]
```

Читает trace из `.papa-yu/traces/<trace_id>.json` или из файла. Пишет в `docs/golden_traces/v1/`.

## Регрессионный тест

```bash
cargo test golden_traces_v1_validate golden_traces_v2_validate
# или
make test-protocol
npm run test-protocol
```

---

## Политика обновления golden traces

**Когда обновлять:** только при намеренном изменении протокола или валидатора (path/content/conflicts, schema, диета).

**Как обновлять:** `trace_to_golden` — `make golden` (из последней трассы) или `make golden TRACE_ID=<id>`.

**Как добавлять новый сценарий:** выполни propose с PAPAYU_TRACE=1, затем `make golden` и сохрани вывод в `v1/NNN_<name>.json` с номером NNN.

**При смене schema_hash:** либо bump schema_version (новый документ v2), либо обнови все fixtures (`trace_to_golden` на свежие трассы) и зафиксируй в CHANGELOG.
