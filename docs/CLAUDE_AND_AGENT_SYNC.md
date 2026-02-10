# Claude и синхронизация с агентом (Claude Code / Cursor)

Настройка PAPA YU для работы с Claude и автоматической синхронизации состояния с IDE-агентом (Cursor, Claude Code и т.п.).

---

## 1. Использование Claude как LLM

PAPA YU вызывает **OpenAI-совместимый** API. Claude можно подключить двумя способами.

### Вариант A: OpenRouter (рекомендуется)

[OpenRouter](https://openrouter.ai/) даёт единый API для разных моделей, включая Claude. Формат запросов совпадает с OpenAI.

1. Зарегистрируйтесь на [openrouter.ai](https://openrouter.ai/).
2. Создайте API-ключ.
3. Задайте переменные окружения:

```bash
export PAPAYU_LLM_API_URL="https://openrouter.ai/api/v1/chat/completions"
export PAPAYU_LLM_API_KEY="sk-or-v1-ваш-ключ"
export PAPAYU_LLM_MODEL="anthropic/claude-3.5-sonnet"
```

Или для Claude 3 Opus:

```bash
export PAPAYU_LLM_MODEL="anthropic/claude-3-opus"
```

4. Запуск: `npm run tauri dev` (или через `start-with-openai.sh`, подставив эти переменные в `.env.openai`).

Кнопка **«Предложить исправления»** будет вызывать Claude через OpenRouter.

### Вариант B: Прямой API Anthropic

Нативный API Anthropic (Messages API) использует другой формат запросов. В текущей версии PAPA YU его поддержка не реализована — используйте OpenRouter (вариант A).

---

## 2. Мульти-провайдер: сбор от нескольких ИИ и оптимальное решение

Чтобы агент собирал ответы от **нескольких ИИ** (Claude, OpenAI и др.), анализировал их и выдавал один оптимальный план, задайте переменную **PAPAYU_LLM_PROVIDERS** — JSON-массив провайдеров.

### Формат PAPAYU_LLM_PROVIDERS

```json
[
  { "url": "https://api.openai.com/v1/chat/completions", "model": "gpt-4o-mini", "api_key": "sk-..." },
  { "url": "https://openrouter.ai/api/v1/chat/completions", "model": "anthropic/claude-3.5-sonnet", "api_key": "sk-or-v1-..." }
]
```

- **url** — OpenAI-совместимый endpoint.
- **model** — имя модели.
- **api_key** — опционально; если не указан, используется **PAPAYU_LLM_API_KEY**.

Запросы к провайдерам выполняются **параллельно**. Результаты объединяются в один план.

### Агрегация

- **Без агрегатора** (по умолчанию): планы объединяются в Rust: действия по одному пути дедуплицируются, итог — один план с объединённым списком действий.
- **С агрегатором-ИИ**: задайте **PAPAYU_LLM_AGGREGATOR_URL** (и при необходимости **PAPAYU_LLM_AGGREGATOR_KEY**, **PAPAYU_LLM_AGGREGATOR_MODEL**). ИИ-агрегатор получит все планы и вернёт один оптимальный в том же JSON-формате.

Пример (одна строка в `.env.openai`):

```bash
# Мульти-провайдер: Claude + OpenAI, без отдельного агрегатора
export PAPAYU_LLM_PROVIDERS='[{"url":"https://openrouter.ai/api/v1/chat/completions","model":"anthropic/claude-3.5-sonnet","api_key":"sk-or-v1-..."},{"url":"https://api.openai.com/v1/chat/completions","model":"gpt-4o-mini","api_key":"sk-..."}]'

# Опционально: отдельная модель для слияния планов
# PAPAYU_LLM_AGGREGATOR_URL=https://api.openai.com/v1/chat/completions
# PAPAYU_LLM_AGGREGATOR_KEY=sk-...
# PAPAYU_LLM_AGGREGATOR_MODEL=gpt-4o-mini
```

Если **PAPAYU_LLM_PROVIDERS** задан и не пустой, обычный одиночный вызов **PAPAYU_LLM_API_URL** не используется для планирования — вместо него выполняется мульти-провайдерный сценарий.

---

## 3. Автоматическая синхронизация с агентом (Claude Code / Cursor)

Идея: после каждого анализа PAPA YU записывает краткое состояние в файл проекта. Агент в IDE (Cursor, Claude Code) может читать этот файл и учитывать контекст.

### Включение записи sync-файла

Задайте переменную окружения:

```bash
export PAPAYU_AGENT_SYNC=1
```

После каждого успешного анализа в корне **проекта** (путь, который вы анализировали) создаётся или обновляется файл:

```
<путь_проекта>/.papa-yu/agent-sync.json
```

Содержимое (пример):

```json
{
  "path": "/Users/you/project",
  "updated_at": "2026-02-08T12:00:00Z",
  "narrative": "Я проанализировал проект...",
  "findings_count": 3,
  "actions_count": 5
}
```

- **path** — путь к проекту.
- **updated_at** — время последнего анализа (ISO 8601).
- **narrative** — краткий человекочитаемый вывод.
- **findings_count** / **actions_count** — число находок и действий.
(При необходимости можно расширить полями `report_md_preview` и др.)

### Как использовать в Cursor / Claude Code

1. **Правило в Cursor**  
   В `.cursor/rules` или в настройках можно добавить правило: «Перед правками проверяй `.papa-yu/agent-sync.json` в корне проекта — там последний анализ PAPA YU (narrative, findings_count, actions_count). Учитывай это при предложениях.»

2. **Чтение из кода/скрипта**  
   Агент или скрипт может читать `./.papa-yu/agent-sync.json` и использовать поля для контекста или логики.

3. **Обратная связь (по желанию)**  
   Можно вручную создать `.papa-yu/agent-request.json` с полем `"action": "analyze"` и путём — в будущих версиях PAPA YU сможет обрабатывать такие запросы (сейчас только запись sync-файла реализована).

---

## 4. Онлайн-взаимодействие

- **LLM** уже работает онлайн: запросы к OpenRouter/OpenAI идут по HTTPS.
- **Синхронизация с агентом** — локальная: файл `.papa-yu/agent-sync.json` на диске; Cursor/Claude Code читает его локально.
- **Расширение (будущее)** — опциональный локальный HTTP-сервер в PAPA YU (например, `127.0.0.1:3939`) с эндпоинтами `POST /analyze`, `GET /report` для вызова из скриптов или агента. Пока достаточно файловой синхронизации.

---

## 5. Краткий чеклист

| Шаг | Действие |
|-----|----------|
| 1 | Задать `PAPAYU_LLM_API_URL`, `PAPAYU_LLM_API_KEY`, `PAPAYU_LLM_MODEL` (OpenRouter + Claude). |
| 2 | При необходимости задать `PAPAYU_AGENT_SYNC=1` для записи `.papa-yu/agent-sync.json`. |
| 3 | Запустить PAPA YU, выполнить анализ проекта. |
| 4 | В Cursor/Claude Code добавить правило или логику чтения `.papa-yu/agent-sync.json`. |

---

**Snyk Code и Documatic:** для дополнения анализа кода (Snyk) и структурирования архитектуры (Documatic) см. **`docs/SNYK_AND_DOCUMATIC_SYNC.md`**.

*См. также `docs/OPENAI_SETUP.md`, `env.openai.example`.*
