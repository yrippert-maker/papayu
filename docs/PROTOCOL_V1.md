# Protocol v1 — контракт papa-yu

Краткий документ (1–2 страницы): что гарантируется, лимиты, логирование, PLAN→APPLY, strict/best-effort.

---

## Версионирование

- **schema_version:** 1
- **schema_hash:** sha256 от `llm_response_schema.json` (в trace)
- При изменении контракта — увеличивать schema_version; v2 — новый документ.

**Default protocol:** v2; Apply может fallback на v1 при специфичных кодах ошибок (см. PROTOCOL_V2_PLAN.md).

---

## Гарантии

1. **JSON:** ответ LLM парсится; при неудаче — 1 repair-ретрай с подсказкой.
2. **Валидация:** path (no `../`, absolute, `~`), конфликты действий, content (no NUL, pseudo-binary).
3. **UPDATE base:** в APPLY каждый UPDATE_FILE — только для файлов, прочитанных в Plan.
4. **Protected paths:** `.env`, `*.pem`, `*.key`, `id_rsa*`, `**/secrets/**` — запрещены.
5. **Apply:** snapshot → apply → auto_check; при падении check — rollback.

---

## Лимиты

| Область | Переменная | По умолчанию |
|---------|------------|--------------|
| path_len | — | 240 |
| actions | — | 200 |
| total_content_bytes | — | 5MB |
| context_files | PAPAYU_CONTEXT_MAX_FILES | 8 |
| file_chars | PAPAYU_CONTEXT_MAX_FILE_CHARS | 20000 |
| context_total | PAPAYU_CONTEXT_MAX_TOTAL_CHARS | 120000 |

---

## Логирование

| Событие | Где |
|---------|-----|
| LLM_REQUEST_SENT | stderr (model, schema_version, provider, token_budget, input_chars) |
| LLM_RESPONSE_OK, LLM_RESPONSE_REPAIR | stderr |
| VALIDATION_FAILED | stderr (code, reason) |
| CONTEXT_CACHE_HIT, CONTEXT_CACHE_MISS | stderr (key) |
| CONTEXT_DIET_APPLIED | stderr (files, dropped, truncated, total_chars) |
| APPLY_SUCCESS, APPLY_ROLLBACK, PREVIEW_READY | stderr |

**Trace (PAPAYU_TRACE=1):** `.papa-yu/traces/<trace_id>.json` — config_snapshot, context_stats, cache_stats, validated_json, schema_version, schema_hash.

---

## PLAN → APPLY

1. **Plan:** `plan: <текст>` или «исправь/почини» → LLM возвращает `actions: []`, `summary`, `context_requests`.
2. **Apply:** `apply: <текст>` или «ok»/«применяй» при сохранённом плане → LLM применяет план с тем же context.
3. **NO_CHANGES:** при пустом `actions` в APPLY — `summary` обязан начинаться с `NO_CHANGES:`.

---

## Strict / best-effort

- **strict (PAPAYU_LLM_STRICT_JSON=1):** `response_format: json_schema` в API; при ошибке schema — repair-ретрай.
- **best-effort:** без response_format; извлечение из ```json ... ```; при ошибке — repair-ретрай.
- **Capability detection:** при 4xx/5xx с упоминанием response_format — retry без него.

---

## Кеш контекста

read_file, search, logs, env кешируются в plan-цикле. Ключ Logs: `{source, last_n}` — разные last_n не пересекаются.

---

## Контекст-диета

При превышении лимитов — урезание: search → logs → файлы низкого приоритета; FILE (read_file) — последними; для priority=0 гарантия минимум 4k chars.

---

## Provider Compatibility

| Provider | Endpoint | `response_format: json_schema` | `strict` | OneOf (array/object) | Режим |
|----------|----------|--------------------------------:|---------:|---------------------:|-------|
| OpenAI | `/v1/chat/completions` | ✅ | ✅ | ✅ | strict + local validate |
| OpenAI-compatible (часть) | разные | ⚠️ | ⚠️ | ⚠️ | best-effort + local validate + repair |
| Ollama | `/api/chat` | ❌ (часто) | ❌ | ⚠️ | best-effort + local validate + repair |

**Поведенческие гарантии:**
1. Если `response_format` не поддержан провайдером → fallback и лог `LLM_RESPONSE_FORMAT_FALLBACK`.
2. Локальная schema validation выполняется всегда (если schema compile ok).
3. Repair-ретрай выполняется один раз при невалидном JSON.
4. Если после repair невалидно → Err.
5. Capability detection: при 4xx/5xx с упоминанием response_format/json_schema → retry без него.

Ускоряет диагностику: если Ollama/локальная модель «тупит» — выключи `PAPAYU_LLM_STRICT_JSON` или оставь пустым.
