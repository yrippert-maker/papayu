# План Protocol v2

Минимальный набор изменений для v2 — без «воды».

---

## Diff v1 → v2 (схема)

| v1 | v2 |
|----|-----|
| `oneOf` (root array \| object) | всегда **объект** |
| `proposed_changes.actions` | только `actions` в корне |
| `UPDATE_FILE` с `content` | `PATCH_FILE` с `patch` + `base_sha256` (по умолчанию) |
| 5 kinds | 6 kinds (+ PATCH_FILE) |
| `content` для CREATE/UPDATE | `content` для CREATE/UPDATE; `patch`+`base_sha256` для PATCH |

Добавлено: `patch`, `base_sha256` (hex 64), взаимоисключающие правила (content vs patch/base).

---

## Главная цель v2

Снизить риск/стоимость «UPDATE_FILE целиком» и улучшить точность правок:
- частичные патчи,
- «операции редактирования» вместо полной перезаписи.

---

## Минимальный набор изменений

### A) Новый action kind: `PATCH_FILE`

Вместо полного `content`, передаётся unified diff:

```json
{ "kind": "PATCH_FILE", "path": "src/app.py", "patch": "@@ -1,3 +1,4 @@\n..." }
```

- Валидация патча локально.
- Применение патча транзакционно.
- Preview diff становится тривиальным.

### B) Новый action kind: `REPLACE_RANGE`

Если unified diff сложен:

```json
{
  "kind": "REPLACE_RANGE",
  "path": "src/app.py",
  "start_line": 120,
  "end_line": 180,
  "content": "новый блок"
}
```

Плюсы: проще валидировать. Минусы: зависит от line numbers (хрупко при изменениях).

### C) «Base hash» для UPDATE/PATCH

Исключить race (файл изменился между plan/apply):

```json
{ "kind": "PATCH_FILE", "path": "...", "base_sha256": "...", "patch": "..." }
```

Если hash не совпал → Err и переход в PLAN.

---

## Совместимость v1/v2

- `schema_version=1` → нынешний формат (UPDATE_FILE, CREATE_FILE, …).
- `schema_version=2` → допускает `PATCH_FILE` / `REPLACE_RANGE` и расширенные поля.

В коде:
- Компилировать обе схемы: `llm_response_schema.json` (v1), `llm_response_schema_v2.json`.
- Выбор активной по env: `PAPAYU_PROTOCOL_DEFAULT` или `PAPAYU_PROTOCOL_VERSION` (default 2).
- Валидация/парсер: сначала проверить schema v2 (если включена), иначе v1.

---

## Порядок внедрения v2 без риска

1. Добавить v2 schema + валидаторы + apply engine.
2. Добавить «LLM prompt v2» (рекомендовать PATCH_FILE вместо UPDATE_FILE).
3. Golden traces v2.
4. **v2 default** с автоматическим fallback на v1 (реализовано).

---

## v2 default + fallback (реализовано)

- **PAPAYU_PROTOCOL_DEFAULT** (или PAPAYU_PROTOCOL_VERSION): default 2.
- **PAPAYU_PROTOCOL_FALLBACK_TO_V1**: default 1 (включён). При ошибках v2 (ERR_PATCH_APPLY_FAILED, ERR_NON_UTF8_FILE, ERR_V2_UPDATE_EXISTING_FORBIDDEN) — автоматический retry с v1.
- Fallback только для APPLY (plan остаётся по выбранному протоколу).
- Trace: `protocol_default`, `protocol_attempts`, `protocol_fallback_reason`.
- Лог: `[trace] PROTOCOL_FALLBACK from=v2 to=v1 reason=ERR_...`

**Compatibility:** Default protocol — v2. Apply может fallback на v1 при специфичных кодах ошибок (ERR_PATCH_APPLY_FAILED, ERR_NON_UTF8_FILE, ERR_V2_UPDATE_EXISTING_FORBIDDEN).

### Метрики для анализа (grep по trace / логам)

- `fallback_rate = fallback_count / apply_count`
- `fallback_rate_excluding_non_utf8` — исключить ERR_NON_UTF8_FILE (не провал v2, ограничение данных)
- Распределение причин fallback:
  - ERR_PATCH_APPLY_FAILED
  - ERR_NON_UTF8_FILE
  - ERR_V2_UPDATE_EXISTING_FORBIDDEN

Trace-поля: `protocol_repair_attempt` (0|1), `protocol_fallback_stage` (apply|preview|validate|schema).

Цель: понять, что мешает v2 стать единственным.

### Graduation criteria (когда отключать fallback / v2-only)

За последние 100 APPLY:

- `fallback_rate < 1%`
- **ERR_PATCH_APPLY_FAILED** < 1% и чаще лечится repair, чем fallback
- **ERR_V2_UPDATE_EXISTING_FORBIDDEN** стремится к 0 (после tightening/repair)
- **ERR_NON_UTF8_FILE** не считается «провалом v2» (ограничение формата; можно отдельно)
- Для честной оценки v2 использовать `fallback_rate_excluding_non_utf8`

Тогда: `PAPAYU_PROTOCOL_FALLBACK_TO_V1=0` и, при необходимости, v2-only.

**protocol_fallback_stage** (где произошло падение): `apply` (сейчас), `preview` (если preview patch не применился), `validate` (семантика), `schema` (валидация JSON) — добавить при расширении.

### Fallback: однократность и repair-first

- **Однократность:** в одном APPLY нельзя зациклиться; если v1 fallback тоже не помог — Err.
- **Repair-first:** для ERR_PATCH_APPLY_FAILED и ERR_V2_UPDATE_EXISTING_FORBIDDEN — сначала repair v2, потом fallback. Для ERR_NON_UTF8_FILE — fallback сразу.
- **Trace:** `protocol_repair_attempt` (0|1), `protocol_fallback_attempted`, `protocol_fallback_stage` (apply|preview|validate|schema).

### Еженедельный отчёт (grep/jq)

Пример пайплайна для анализа трасс (trace JSON в одной строке на файл):

```bash
# APPLY count
grep -l '"event":"LLM_PLAN_OK"' traces/*.json 2>/dev/null | wc -l

# fallback_count (protocol_fallback_attempted)
grep '"protocol_fallback_attempted":true' traces/*.json 2>/dev/null | wc -l

# breakdown по причинам
grep -oh '"protocol_fallback_reason":"[^"]*"' traces/*.json 2>/dev/null | sort | uniq -c

# repair_success (protocol_repair_attempt=0 и нет fallback в следующей трассе) — требует связки
jq -s '[.[] | select(.event=="LLM_PLAN_OK" and .protocol_repair_attempt==0)] | length' traces/*.json 2>/dev/null

# top paths по repair_injected_sha256
grep -oh '"repair_injected_paths":\[[^]]*\]' traces/*.json 2>/dev/null | sort | uniq -c | sort -rn | head -20
```


**System prompt v2** (`FIX_PLAN_SYSTEM_PROMPT_V2`): жёсткие правила PATCH_FILE, base_sha256, object-only, NO_CHANGES. Включается при `PAPAYU_PROTOCOL_VERSION=2` и режиме fix-plan/fixit.

**Формат FILE-блока v2:**
```
FILE[path/to/file.py] (sha256=7f3f2a0c9f8b1a0c9b4c0f9e3d8a4b2d8c9e7f1a0b3c4d5e6f7a8b9c0d1e2f3a):
<content>
```

sha256 — от полного содержимого файла; **не обрезается** при context-diet. Модель копирует его в `base_sha256` для PATCH_FILE.

### Prompt rules (оптимизация v2)

- Патч должен быть **минимальным** — меняй только нужные строки, не форматируй файл целиком.
- Каждый `@@` hunk должен иметь 1–3 строки контекста до/после изменения.
- Не делай массовых форматирований и EOL-изменений.
- Если файл не UTF-8 или слишком большой/генерируемый — верни PLAN (actions=[]) и запроси альтернативу.

**Авто-эскалация при ERR_PATCH_APPLY_FAILED** (опционально): при repair retry добавить «Увеличь контекст hunks до 3 строк, не меняй соседние блоки.»

---

## PATCH_FILE engine (реализовано)

- **Модуль `patch`:** sha256_hex, is_valid_sha256_hex, looks_like_unified_diff, apply_unified_diff_to_text (diffy)
- **tx::apply_patch_file_impl:** проверка base_sha256 → применение diff → EOL нормализация → запись
- **Preview:** preview_patch_file проверяет base_sha256 и применимость, возвращает patch в DiffItem
- **Коды ошибок:** ERR_PATCH_NOT_UNIFIED, ERR_BASE_MISMATCH, ERR_PATCH_APPLY_FAILED, ERR_BASE_SHA256_INVALID, ERR_NON_UTF8_FILE
- **Repair hints:** REPAIR_ERR_* для repair flow / UI

---

## ERR_NON_UTF8_FILE и ERR_V2_UPDATE_EXISTING_FORBIDDEN

**ERR_NON_UTF8_FILE:** PATCH_FILE работает только по UTF-8 тексту. Для бинарных/не-UTF8 файлов — только CREATE_FILE (если явно нужно), иначе отказ/PLAN. Сообщение для UI: «Файл не UTF-8. PATCH_FILE недоступен. Перейди в PLAN и выбери другой подход.»

**ERR_V2_UPDATE_EXISTING_FORBIDDEN:** В v2 UPDATE_FILE запрещён для существующих файлов. Семантический гейт: если UPDATE_FILE и файл существует → ошибка. Repair: «Сгенерируй PATCH_FILE вместо UPDATE_FILE».

---

## Рекомендации для v2

- В v2 модификация существующих файлов **по умолчанию** через `PATCH_FILE`.
- `base_sha256` обязателен для `PATCH_FILE` и проверяется приложением.
- При `ERR_BASE_MISMATCH` требуется новый PLAN (файл изменился).
- В APPLY отсутствие изменений оформляется через `NO_CHANGES:` и `actions: []`.

---

## Примеры v2 ответов

### PLAN (v2): план без изменений

```json
{
  "actions": [],
  "summary": "Диагноз: падает из-за неверной обработки None.\nПлан:\n1) Прочитать src/parser.py вокруг функции parse().\n2) Добавить проверку на None и поправить тест.\nПроверка: pytest -q",
  "context_requests": [
    { "type": "read_file", "path": "src/parser.py", "start_line": 1, "end_line": 260 },
    { "type": "read_file", "path": "tests/test_parser.py", "start_line": 1, "end_line": 200 }
  ],
  "memory_patch": {}
}
```

### APPLY (v2): PATCH_FILE на существующий файл

`base_sha256` должен совпасть с хэшем текущего файла.

```json
{
  "actions": [
    {
      "kind": "PATCH_FILE",
      "path": "src/parser.py",
      "base_sha256": "7f3f2a0c9f8b1a0c9b4c0f9e3d8a4b2d8c9e7f1a0b3c4d5e6f7a8b9c0d1e2f3a",
      "patch": "--- a/src/parser.py\n+++ b/src/parser.py\n@@ -41,6 +41,10 @@ def parse(value):\n-    return value.strip()\n+    if value is None:\n+        return \"\"\n+    return value.strip()\n"
    },
    {
      "kind": "PATCH_FILE",
      "path": "tests/test_parser.py",
      "base_sha256": "0a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0",
      "patch": "--- a/tests/test_parser.py\n+++ b/tests/test_parser.py\n@@ -10,7 +10,7 @@ def test_parse_none():\n-    assert parse(None) is None\n+    assert parse(None) == \"\"\n"
    }
  ],
  "summary": "Исправлено: parse(None) теперь возвращает пустую строку. Обновлён тест.\nПроверка: pytest -q",
  "context_requests": [],
  "memory_patch": {}
}
```

### APPLY (v2): создание файлов (как в v1)

```json
{
  "actions": [
    { "kind": "CREATE_DIR", "path": "src" },
    {
      "kind": "CREATE_FILE",
      "path": "README.md",
      "content": "# My Project\n\nRun: `make run`\n"
    }
  ],
  "summary": "Созданы папка src и README.md.",
  "context_requests": [],
  "memory_patch": {}
}
```

### APPLY (v2): NO_CHANGES

```json
{
  "actions": [],
  "summary": "NO_CHANGES: Код уже соответствует требованиям, правки не нужны.\nПроверка: pytest -q",
  "context_requests": [],
  "memory_patch": {}
}
```

---

## Ошибки движка v2

| Код | Когда | Действие |
|-----|-------|----------|
| `ERR_BASE_MISMATCH` | Файл изменился между PLAN и APPLY, sha256 не совпал | Вернуться в PLAN, перечитать файл, обновить base_sha256 |
| `ERR_PATCH_APPLY_FAILED` | Hunks не применились (контекст не совпал) | Вернуться в PLAN, запросить более точный контекст, перегенерировать патч |
| `ERR_PATCH_NOT_UNIFIED` | LLM прислал не unified diff | Repair-ретрай с требованием unified diff |
