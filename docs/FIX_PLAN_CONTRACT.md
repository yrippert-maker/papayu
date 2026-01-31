# Fix-plan оркестратор: контракты JSON и автосбор контекста

papa-yu — **Rust/Tauri**, не Python. Ниже — текущий JSON-ответ, расширенный контракт Fix-plan/Apply и как это встроено в приложение.

---

## 1) Текущий JSON-ответ (как есть сейчас)

Модель возвращает **один** JSON. Приложение парсит и применяет действия по подтверждению пользователя.

### Вариант A: массив действий

```json
[
  { "kind": "CREATE_FILE", "path": "README.md", "content": "# Project\n" },
  { "kind": "CREATE_DIR", "path": "src" }
]
```

### Вариант B: объект с actions + memory_patch

```json
{
  "actions": [
    { "kind": "UPDATE_FILE", "path": "src/main.py", "content": "..." }
  ],
  "memory_patch": {
    "project.default_test_command": "pytest -q"
  }
}
```

### Поля элемента actions

| Поле     | Тип    | Обязательность | Описание |
|----------|--------|----------------|----------|
| `kind`   | string | да             | `CREATE_FILE` \| `CREATE_DIR` \| `UPDATE_FILE` \| `DELETE_FILE` \| `DELETE_DIR` |
| `path`   | string | да             | Относительный путь от корня проекта |
| `content`| string | нет            | Для CREATE_FILE / UPDATE_FILE |

### Результат в приложении

- `AgentPlan`: `{ ok, summary, actions, error?, error_code? }`
- `memory_patch` применяется по whitelist и сохраняется в `preferences.json` / `.papa-yu/project.json`

---

## 2) Расширенный контракт: Fix-plan и Apply

Один JSON-объект с полем `mode`. Приложение понимает оба формата (текущий и расширенный).

### Режим fix-plan (только план, применение после подтверждения)

```json
{
  "mode": "fix-plan",
  "summary": "Коротко: почему падает и что делаем",
  "questions": ["Нужен ли тест на X?"],
  "context_requests": [
    { "type": "read_file", "path": "src/x.py", "start_line": 1, "end_line": 220 },
    { "type": "search", "query": "SomeSymbol", "glob": "**/*.py" },
    { "type": "logs", "source": "runtime", "last_n": 200 }
  ],
  "plan": [
    { "step": "Диагностика", "details": "..." },
    { "step": "Правка", "details": "..." },
    { "step": "Проверка", "details": "Запустить pytest -q" }
  ],
  "proposed_changes": {
    "patch": "unified diff (optional в fix-plan)",
    "actions": [
      { "kind": "UPDATE_FILE", "path": "src/x.py", "content": "..." }
    ],
    "commands_to_run": ["pytest -q"]
  },
  "risks": ["Затрагивает миграции"],
  "memory_patch": {
    "project.default_test_command": "pytest -q"
  }
}
```

- Если есть `context_requests`, приложение подтягивает контекст (read_file, search, logs) и повторяет запрос к модели (до 2 раундов).
- Действия для UI/apply берутся из `proposed_changes.actions` (если есть), иначе из корневого `actions` (обратная совместимость).

### Режим apply (после «ок» пользователя)

```json
{
  "mode": "apply",
  "summary": "Что применяем",
  "patch": "unified diff (обязательно при применении diff)",
  "commands_to_run": ["pytest -q", "ruff check ."],
  "verification": ["Ожидаем: все тесты зелёные"],
  "rollback": ["git checkout -- <files>"],
  "memory_patch": {}
}
```

В текущей реализации применение идёт по списку **actions** (CREATE_FILE/UPDATE_FILE/…). Поле `patch` (unified diff) зарезервировано под будущую поддержку `apply_patch` в бэкенде.

---

## 3) System prompt под один JSON (Fix-plan)

Ядро, которое вставляется при режиме Fix-plan (переменная `PAPAYU_LLM_MODE=fix-plan` или отдельный промпт):

```text
Ты — инженерный ассистент внутри программы для создания, анализа и исправления кода. Оператор один: я.
Всегда отвечай ОДНИМ валидным JSON-объектом. Никакого текста вне JSON.

Режимы:
- "fix-plan": предлагаешь план и (опционально) proposed_changes (actions, patch, commands_to_run). Ничего не применяешь.
- "apply": выдаёшь финальный patch и команды для применения/проверки (после подтверждения оператора).

Правила:
- Не выдумывай содержимое файлов/логов. Если нужно — запроси через context_requests.
- Никогда не утверждай, что тесты/команды запускались, если их не запускало приложение.
- Если данных не хватает — задай максимум 2 вопроса в questions и/или добавь context_requests.
- Минимальные изменения. Без широких рефакторингов без явного запроса.

ENGINEERING_MEMORY:
{...вставляется приложением...}

Схема JSON:
- mode: "fix-plan" | "apply" (или опусти для обратной совместимости — тогда ожидается массив actions или объект с actions)
- summary: string
- questions: string[]
- context_requests: [{ type: "read_file"|"search"|"logs"|"env", path?, start_line?, end_line?, query?, glob?, source?, last_n? }]
- plan: [{ step, details }]
- proposed_changes: { patch?, actions?, commands_to_run? }
- patch: string (обязательно в apply при применении diff)
- commands_to_run: string[]
- verification: string[]
- risks: string[]
- rollback: string[]
- memory_patch: object (только ключи из whitelist)
```

---

## 4) Автосбор контекста (без tools)

Приложение **до** первого запроса к модели собирает базовый контекст и подставляет в user-сообщение:

### Базовый набор

- **env**: версия Python/Node/OS, venv, менеджер зависимостей (если определимо по проекту).
- **project prefs**: команды тестов/линта/формата из `.papa-yu/project.json` (уже в ENGINEERING_MEMORY).
- **recent_files**: список недавно открытых/изменённых файлов из `report_json` (если передан).
- **logs**: последние N строк логов (runtime/build) — если приложение имеет к ним доступ.

### При ошибке/stacktrace в запросе

- Распарсить пути и номера строк из Traceback.
- Добавить в контекст фрагменты файлов ±80 строк вокруг указанных строк.
- При «падает тест X» — подтянуть файл теста и (по возможности) тестируемый модуль.

Эвристики реализованы в Rust: см. `context::gather_base_context`, `context::fulfill_context_requests`.

---

## 5) JSON Schema для response_format (OpenAI Chat Completions)

Используется endpoint **Chat Completions** (`/v1/chat/completions`). Для строгого JSON можно передать `response_format: { type: "json_schema", json_schema: { ... } }` (если провайдер поддерживает).

Пример схемы под объединённый контракт (и текущий, и Fix-plan):

```json
{
  "name": "papa_yu_plan_response",
  "strict": true,
  "schema": {
    "type": "object",
    "properties": {
      "mode": { "type": "string", "enum": ["fix-plan", "apply"] },
      "summary": { "type": "string" },
      "questions": { "type": "array", "items": { "type": "string" } },
      "context_requests": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "type": { "type": "string", "enum": ["read_file", "search", "logs", "env"] },
            "path": { "type": "string" },
            "start_line": { "type": "integer" },
            "end_line": { "type": "integer" },
            "query": { "type": "string" },
            "glob": { "type": "string" },
            "source": { "type": "string" },
            "last_n": { "type": "integer" }
          }
        }
      },
      "plan": {
        "type": "array",
        "items": { "type": "object", "properties": { "step": { "type": "string" }, "details": { "type": "string" } } }
      },
      "actions": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "kind": { "type": "string", "enum": ["CREATE_FILE", "CREATE_DIR", "UPDATE_FILE", "DELETE_FILE", "DELETE_DIR"] },
            "path": { "type": "string" },
            "content": { "type": "string" }
          },
          "required": ["kind", "path"]
        }
      },
      "proposed_changes": {
        "type": "object",
        "properties": {
          "patch": { "type": "string" },
          "actions": { "type": "array", "items": { "$ref": "#/definitions/action" } },
          "commands_to_run": { "type": "array", "items": { "type": "string" } }
        }
      },
      "patch": { "type": "string" },
      "commands_to_run": { "type": "array", "items": { "type": "string" } },
      "verification": { "type": "array", "items": { "type": "string" } },
      "risks": { "type": "array", "items": { "type": "string" } },
      "rollback": { "type": "array", "items": { "type": "string" } },
      "memory_patch": { "type": "object", "additionalProperties": true }
    },
    "additionalProperties": true
  }
}
```

Для «только массив actions» схему можно упростить или использовать два варианта (массив vs объект) на стороне парсера — текущий парсер в Rust принимает и массив, и объект с `actions` и `memory_patch`.

---

## 6) Режим в приложении

Переменная окружения **`PAPAYU_LLM_MODE`**:
- `chat` (по умолчанию) — инженер-коллега, ответ массив/объект с `actions`.
- `fixit` — обязан вернуть патч и проверку (текущий FIXIT prompt).
- **`fix-plan`** — один JSON с `mode`, `summary`, `context_requests`, `plan`, `proposed_changes`, `memory_patch`; автосбор контекста и до 2 раундов по `context_requests`.

ENGINEERING_MEMORY подставляется в system prompt приложением (см. `memory::build_memory_block`).

---

## 7) Как подключить в UI

- **«Fix (plan)»** / текущий сценарий: вызов `propose_actions` → показ `summary`, `plan`, `risks`, `questions`, превью по `proposed_changes.actions` или `actions`.
- **«Применить»**: вызов `apply_actions_tx` с выбранными `actions` (из ответа модели). Память уже обновлена по `memory_patch` при парсинге ответа.

Flow «сначала план → подтверждение → применение» обеспечивается тем, что приложение не применяет действия до явного подтверждения пользователя; модель может отдавать как короткий формат (массив/actions), так и расширенный (mode fix-plan + proposed_changes).

---

## 8) Инженерная память

- MEMORY BLOCK подставляется в system prompt.
- Модель заполняет `commands_to_run` из `project.default_test_command` и т.п.
- При явной просьбе «запомни …» модель возвращает `memory_patch`; приложение применяет его по whitelist и сохраняет в файлы.

Whitelist и логика — в `src-tauri/src/memory.rs`.
