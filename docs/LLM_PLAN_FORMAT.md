# Стек papa-yu и JSON-контракт ответа (план)

## На чём написан papa-yu

| Слой      | Стек                | Примечание                          |
|-----------|---------------------|-------------------------------------|
| **Backend** | **Rust** (Tauri)    | Команды, LLM, FS, apply, undo, tx  |
| **Frontend** | **TypeScript + React** (Vite) | UI, запросы к Tauri (invoke)     |

Не Python/Node/Go — бэкенд полностью на Rust; фронт — React/Vite.

---

## JSON Schema для response_format

Полная схема для `response_format` (OpenAI Responses API и др.) — см. `docs/papa_yu_response_schema.json`.

Схема для Chat Completions (`response_format: { type: "json_schema", ... }`) — `src-tauri/config/llm_response_schema.json`. Включается через `PAPAYU_LLM_STRICT_JSON=1`.

**Поведение strict / best-effort:**
- **strict включён** — приложение отправляет `response_format` в API; при ответе, не проходящем JSON schema, локально отклоняет и выполняет 1 авто-ретрай с repair-подсказкой.
- **strict выключен или провайдер не поддерживает** — best-effort парсинг (извлечение из ```json ... ```), затем локальная валидация; при неудаче — тот же repair-ретрай.

---

## Текущий JSON-контракт ответа (план от LLM)

LLM должен вернуть **только валидный JSON** — либо массив действий, либо объект с полями.

### Принимаемые форматы

1. **Массив действий** — `[{ kind, path, content? }, ...]`
2. **Объект** — `{ actions?, proposed_changes.actions?, summary?, context_requests?, memory_patch? }`

### Формат Action (элемент массива)

| Поле     | Тип    | Обязательность | Описание |
|----------|--------|----------------|----------|
| `kind`   | string | да             | Один из: `CREATE_FILE`, `CREATE_DIR`, `UPDATE_FILE`, `DELETE_FILE`, `DELETE_DIR` |
| `path`   | string | да             | Относительный путь от корня проекта (без `../`, без абсолютных путей, без `~`) |
| `content`| string | да для CREATE_FILE, UPDATE_FILE | Содержимое файла; макс. ~1MB на файл; для `CREATE_DIR`/`DELETE_*` не используется |

**Ограничения на path:** no `../`, no абсолютные (`/`, `C:\`), no `~`. Локальная валидация отклоняет.

**DELETE_*:** требует подтверждения пользователя в UI (кнопка «Применить»).

**Plan→Apply без кнопок:**
- Префиксы: `plan: <текст>` → режим Plan; `apply: <текст>` → режим Apply.
- Триггеры перехода: `ok`, `ок`, `apply`, `применяй`, `да` — при наличии сохранённого плана переключают на Apply.
- По умолчанию: «исправь/почини» → Plan; «создай/сгенерируй» → Apply.

**APPLY без изменений (каноничный маркер):**
- Если изменений не требуется — верни `actions: []` и `summary`, **начинающийся с `NO_CHANGES:`** (строго).
- Пример: `"summary": "NO_CHANGES: Проверка завершена, правок не требуется."`

**Конфликты действий:**
- Один path не должен иметь несовместимых действий: CREATE_FILE + UPDATE_FILE, DELETE + CREATE/UPDATE.
- Порядок применения: CREATE_DIR → CREATE_FILE/UPDATE_FILE → DELETE_FILE → DELETE_DIR.

**Пути:**
- Запрещены: абсолютные (`/`, `//`), Windows drive (`C:/`), UNC (`//server/share`), `~`, сегменты `..` и `.`, пустой или только `.`.
- Лимиты: max_path_len=240, max_actions=200, max_total_content_bytes=5MB.

**ERR_UPDATE_WITHOUT_BASE:**
- В режиме APPLY каждый UPDATE_FILE должен ссылаться на файл, прочитанный в Plan (FILE[path]: или === path === в plan_context).

**Protected paths (denylist):**
- `.env`, `*.pem`, `*.key`, `*.p12`, `id_rsa*`, `**/secrets/**` — запрещены для UPDATE/DELETE.

**Content:**
- Запрещён NUL (`\0`), >10% non-printable = ERR_PSEUDO_BINARY.

**EOL:**
- `PAPAYU_NORMALIZE_EOL=lf` — нормализовать \r\n→\n, trailing newline.

**Наблюдаемость:**
- Каждый propose имеет `trace_id` (UUID). Лог-ивенты в stderr: `LLM_REQUEST_SENT` (token_budget, input_chars), `LLM_RESPONSE_OK`, `VALIDATION_FAILED`, `LLM_REQUEST_TIMEOUT`, `LLM_RESPONSE_FORMAT_FALLBACK`.
- `PAPAYU_TRACE=1` — трасса в `.papa-yu/traces/<trace_id>.json`. По умолчанию raw_content не сохраняется (риск секретов); `PAPAYU_TRACE_RAW=1` — сохранять с маскировкой sk-/Bearer. В трассе — `config_snapshot`.

**Параметры генерации:** temperature=0, max_tokens=16384 (авто-кэп: при input>80k → 4096), top_p=1, presence_penalty=0, frequency_penalty=0. `PAPAYU_LLM_TIMEOUT_SEC=90`. Capability detection: при ошибке response_format — retry без него.

**Версия схемы:** `LLM_PLAN_SCHEMA_VERSION=1` — в system prompt и trace; для будущей поддержки v1/v2 при расширении kinds/полей. `x_schema_version` в llm_response_schema.json. `schema_hash` (sha256) в config_snapshot.

**Кеш контекста:** read_file/search/logs/env кешируются в пределах plan-цикла. Логи: CONTEXT_CACHE_HIT, CONTEXT_CACHE_MISS.

**Контекст-диета:** см. раздел «Контекст-диета» ниже.

**Trace:** при `PAPAYU_TRACE=1` в трассу добавляются `context_stats` (context_files_count, context_files_dropped_count, context_total_chars, context_logs_chars, context_truncated_files_count) и `cache_stats` (hits/misses по типам env/logs/read/search, hit_rate).

### Fix-plan режим (user.output_format)

- **PLAN** (`plan`): `actions` пустой массив `[]`, `summary` обязателен (диагноз + шаги + команды проверки), при необходимости — `context_requests`.
- **APPLY** (`apply`): `actions` непустой, если нужны изменения; иначе пустой + `summary` «изменений не требуется». `summary` — что сделано и как проверить (используй `project.default_test_command` если задан).

### Пример ответа (объект)

```json
{
  "actions": [
    { "kind": "CREATE_FILE", "path": "README.md", "content": "# Project\n\n## Run\n\n`make run`\n" },
    { "kind": "CREATE_DIR", "path": "src" }
  ],
  "summary": "Созданы README и папка src.",
  "context_requests": [],
  "memory_patch": {}
}
```

### Пример Fix-plan (plan-режим)

```json
{
  "actions": [],
  "summary": "Диагноз: ...\nПлан:\n1) ...\n2) ...\nПроверка: pytest -q",
  "context_requests": [
    { "type": "read_file", "path": "src/app.py", "start_line": 1, "end_line": 240 }
  ],
  "memory_patch": { "user.output_format": "plan" }
}
```

### Как приложение обрабатывает ответ

1. Парсит JSON из ответа (извлекает из ```json ... ``` при наличии).
2. Берёт `actions` из корня или `proposed_changes.actions`.
3. Валидирует: path (no `../`, no absolute), content обязателен для CREATE_FILE/UPDATE_FILE.
4. `summary` используется если есть; иначе формируется в коде.
5. `context_requests` — выполняется в следующем раунде (до MAX_CONTEXT_ROUNDS).
6. `memory_patch` — применяется только ключи из whitelist.

---

---

## Контекст-диета (поведение рантайма)

Контекст может быть урезан для контроля стоимости токенов и стабильности ответов.

**Env-переменные лимитов:**
| Переменная | По умолчанию | Описание |
|------------|--------------|----------|
| `PAPAYU_CONTEXT_MAX_FILES` | 8 | Макс. число FILE/SEARCH/LOGS/ENV блоков в FULFILLED_CONTEXT |
| `PAPAYU_CONTEXT_MAX_FILE_CHARS` | 20000 | Макс. символов на один файл (read_file) |
| `PAPAYU_CONTEXT_MAX_TOTAL_CHARS` | 120000 | Макс. символов всего блока FULFILLED_CONTEXT |
| `PAPAYU_CONTEXT_MAX_LOG_CHARS` | 12000 | Резерв для логов (в текущей реализации не используется) |

**Порядок урезания:** при нехватке budget — search hits, logs; FILE-блоки (запрошенные read_file) — последними; для priority=0 файлов гарантируется минимум 4k chars даже при нехватке total budget.

**Truncation:** при превышении MAX_FILE_CHARS — head+tail (60/40) с маркером `...[TRUNCATED N chars]...`.

**Лог:** `CONTEXT_DIET_APPLIED files=N dropped=M truncated=T total_chars=C` при dropped>0 или truncated>0.

**Trace:** в `context_stats` — context_files_count, context_files_dropped_count, context_total_chars, context_logs_chars, context_truncated_files_count.

---

## context_requests (типы запросов)

| type       | Обязательные поля | Описание |
|------------|-------------------|----------|
| `read_file`| path              | Прочитать файл (опционально start_line, end_line) |
| `search`   | query             | Поиск по проекту (опционально glob) |
| `logs`     | source            | Логи (приложение ограничено) |
| `env`      | —                 | Информация об окружении |

---

## Автосбор контекста (до первого вызова LLM)

Эвристики по содержимому user_goal и отчёта:

- **Traceback / Exception** → извлекаются пути и номера строк, читаются файлы ±80 строк вокруг
- **ImportError / ModuleNotFoundError / cannot find module** → добавляются ENV + содержимое pyproject.toml, requirements.txt, package.json, poetry.lock

---

## Типы в Rust (справочно)

- `Action`: `{ kind: ActionKind, path: String, content: Option<String> }`
- `ActionKind`: enum `CreateFile | CreateDir | UpdateFile | DeleteFile | DeleteDir` (сериализуется в SCREAMING_SNAKE_CASE)
- `AgentPlan`: `{ ok: bool, summary: String, actions: Vec<Action>, error?: String, error_code?: String }`

---

## memory_patch + whitelist + пример промпта (под этот контракт)

### 1) memory_patch (что подставлять в промпт как контекст «памяти»)

Хранить в приложении (файл/БД/локальное хранилище) и подставлять в system или в начало user-сообщения:

```json
{
  "preferred_style": "коротко, по делу",
  "default_language": "python",
  "test_command": "pytest -q",
  "lint_command": "ruff check .",
  "format_command": "ruff format .",
  "project_root_hint": "src/ — код, tests/ — тесты"
}
```

В промпте: один абзац, например:  
«Память: стиль — коротко по делу; язык по умолчанию — python; тесты — pytest -q; линт — ruff check .; структура — src/, tests/.»

### 2) whitelist (разрешённые пути для действий)

При парсинге плана и перед apply проверять: все `path` должны быть относительно корня проекта и не выходить за его пределы. Дополнительно можно ограничить типы файлов/папок.

Пример whitelist (Rust/конфиг):

- Разрешены расширения для CREATE_FILE/UPDATE_FILE: `.py`, `.ts`, `.tsx`, `.js`, `.jsx`, `.json`, `.md`, `.yaml`, `.yml`, `.toml`, `.css`, `.html`, `.sql`, `.sh`, `.env.example`, без расширения — только известные имена: `README`, `Makefile`, `.gitignore`, `.editorconfig`, `Dockerfile`.
- Запрещены пути: содержащие `..`, абсолютные пути, `.env` (без .example), `*.key`, `*.pem`, каталоги `node_modules/`, `.git/`, `__pycache__/`.

Файл конфига (например `config/llm_whitelist.json`):

```json
{
  "allowed_extensions": [".py", ".ts", ".tsx", ".js", ".jsx", ".json", ".md", ".yaml", ".yml", ".toml", ".css", ".html", ".sql", ".sh"],
  "allowed_no_extension": ["README", "Makefile", ".gitignore", ".editorconfig", "Dockerfile", "LICENSE"],
  "forbidden_paths": [".env", "*.key", "*.pem", "node_modules", ".git", "__pycache__"],
  "forbidden_prefixes": ["..", "/"]
}
```

### 3) Пример промпта (фрагмент под твой JSON-контракт)

Добавить в промпт явное напоминание формата ответа:

```text
Верни ТОЛЬКО валидный JSON — массив действий, без markdown и пояснений.
Формат каждого элемента: { "kind": "CREATE_FILE" | "CREATE_DIR" | "UPDATE_FILE" | "DELETE_FILE" | "DELETE_DIR", "path": "относительный/путь", "content": "опционально для CREATE_FILE/UPDATE_FILE" }.
Пример: [{"kind":"CREATE_FILE","path":"README.md","content":"# Project\n"},{"kind":"CREATE_DIR","path":"src"}]
```

Это уже есть в build_prompt; при добавлении memory_patch в начало user-сообщения можно добавить блок:

```text
Память (предпочтения оператора): preferred_style=коротко по делу; default_language=python; test_command=pytest -q; lint_command=ruff check .; format_command=ruff format .; структура проекта: src/, tests/.
```

После применения whitelist при apply отклонять действия с path вне whitelist и возвращать в AgentPlan error с кодом FORBIDDEN_PATH.

---

## Инженерная память (Engineering Memory)

Память разделена на три слоя; в промпт подставляется только устойчивый минимум (~1–2 KB).

### A) User prefs (оператор)

- Расположение: `app_data_dir()/papa-yu/preferences.json` (локально).
- Поля: `preferred_style` (brief|normal|verbose), `ask_budget` (0..2), `risk_tolerance` (low|medium|high), `default_language`, `output_format` (patch_first|plan_first).

### B) Project prefs (для репо)

- Расположение: в репо `.papa-yu/project.json` (шарится между машинами при коммите).
- Поля: `default_test_command`, `default_lint_command`, `default_format_command`, `package_manager`, `build_command`, `src_roots`, `test_roots`, `ci_notes`.

### C) Session state

- В памяти процесса (не в файлах): current_task_goal, current_branch, recent_files, recent_errors. В текущей реализации не подставляется в промпт.

### MEMORY BLOCK в промпте

Добавляется в **system message** после основного system prompt:

```text
ENGINEERING_MEMORY (trusted by user; update only when user requests):
{"user":{"preferred_style":"brief","ask_budget":1,"risk_tolerance":"medium","default_language":"python"},"project":{"default_test_command":"pytest -q","default_lint_command":"ruff check .","default_format_command":"ruff format .","src_roots":["src"],"test_roots":["tests"]}}

Use ENGINEERING_MEMORY as defaults. If user explicitly asks to change — suggest updating memory and show new JSON.
```

### Ответ с memory_patch

Если пользователь просит «запомни, что тесты запускать так-то», LLM может вернуть объект:

```json
{
  "actions": [],
  "memory_patch": {
    "project.default_test_command": "pytest -q",
    "user.preferred_style": "brief"
  }
}
```

Приложение применяет только ключи из whitelist и сохраняет в `preferences.json` / `.papa-yu/project.json`.

**Безопасность memory_patch:** при парсинге удаляются все ключи не из whitelist; валидируются типы (`ask_budget` int, `src_roots` массив строк и т.д.). Рекомендуется применять patch только при явной просьбе пользователя.

### Whitelist memory_patch (ключи через точку)

- `user.preferred_style`, `user.ask_budget`, `user.risk_tolerance`, `user.default_language`, `user.output_format`
- `project.default_test_command`, `project.default_lint_command`, `project.default_format_command`, `project.package_manager`, `project.build_command`, `project.src_roots`, `project.test_roots`, `project.ci_notes`

Примеры файлов: см. `docs/preferences.example.json` и `docs/project.example.json`.
