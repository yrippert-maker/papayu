# PAPA YU v2.4.4

Десктопное приложение для анализа проекта и автоматических исправлений (README, .gitignore, tests/, структура) с **транзакционным apply**, **реальным undo** и **autoCheck с откатом**.

## Единственная папка проекта

Вся разработка, сборка и запуск ведутся из **этой папки** (например `/Users/.../Desktop/papa-yu`). ТЗ и спецификации лежат отдельно в папке **папа-ю** на рабочем столе (не переносятся). Подробнее: `docs/ЕДИНАЯ_ПАПКА_ПРОЕКТА.md`.

**Запуск без терминала:** двойной клик по `PAPA YU.command` (только запуск) или по `PAPA YU — Сборка и запуск.command` (сборка + запуск).

## Требования

- Node.js 18+
- Rust 1.70+
- npm

## Запуск

```bash
cd papa-yu
npm install
npm run tauri dev
```

Из корня проекта можно также: `cd src-tauri && cargo tauri dev`.

**Если в окне видно «Could not fetch a valid…»** — фронт не загрузился. Запускайте приложение только так: в терминале из папки проекта выполните `npm run tauri dev` (это поднимает и Vite, и Tauri). Не открывайте скомпилированный .app без dev-сервера, если хотите видеть интерфейс.

## Сборка

```bash
npm run tauri build
```

## v2.4.4 — что реализовано

### Анализ и профиль

- **Анализ по пути** — выбор папки или ввод пути вручную; анализ возвращает отчёт (findings, recommendations, actions, action_groups, fix_packs).
- **Профиль по пути** — автоматическое определение типа проекта (React/Vite, Next.js, Node, Rust, Python) и лимитов (max_actions_per_tx, timeout_sec, max_files). Профиль и лимиты отображаются в форме.

### Применение и откат

- **Транзакционное apply** — перед применением создаётся снимок; после apply выполняется autoCheck (cargo check / npm run build и т.д.) с таймаутом из профиля. При падении проверки — автоматический откат.
- **Лимиты профиля** — в `apply_actions_tx` и `run_batch` проверяется число действий против `max_actions_per_tx`; при превышении возвращается ошибка TOO_MANY_ACTIONS. Таймаут проверок задаётся из профиля.
- **Undo/Redo** — откат последней транзакции и повтор; состояние отображается в UI.

### Безопасность

- **Защита путей** — запрещено изменение служебных путей (.git, node_modules, target, dist и т.д.) и бинарных файлов; разрешены только текстовые расширения (см. guard в коде).
- **Подтверждение** — применение только при явном подтверждении пользователя (user_confirmed).
- **Allowlist команд** — в verify и auto_check выполняются только разрешённые команды с фиксированными аргументами (конфиг в `src-tauri/config/verify_allowlist.json`).

### UX

- **Папки и файлы** — выбор папки, прикрепление файлов (с фильтром расширений: .ts, .tsx, .rs, .py, .json, .toml и др.), ручной ввод пути.
- **История сессий** — по выбранному проекту отображается список сессий (дата, количество событий); после agentic run список обновляется.
- **Горячие клавиши** — Ctrl+Enter (Cmd+Enter на Mac): отправить/запустить анализ; Escape: сбросить превью изменений.
- **Тёмная тема** — переключатель в боковой панели; выбор сохраняется в localStorage; поддержка системных настроек темы.
- **Экспорт/импорт настроек** — кнопки «Экспорт» и «Импорт» в боковой панели для сохранения и восстановления всех настроек (проекты, профили, сессии, папки) в JSON-файл.

### Режимы

- **Batch** — анализ → превью → при необходимости применение с проверками (одна команда `run_batch`).
- **Исправить автоматически (agentic run)** — цикл: анализ → план → превью → применение → проверка; при неудаче проверки — откат и повтор в пределах max_attempts.
- **Безопасные исправления в один клик** — генерация безопасных действий по отчёту → превью → применение с проверкой.
- **Предложить исправления** — план по отчёту и цели: при наличии настройки LLM — вызов внешнего API (OpenAI/Ollama и др.), иначе эвристика.

### LLM-планировщик (опционально)

Для кнопки «Предложить исправления» можно включить генерацию плана через OpenAI-совместимый API. Задайте переменные окружения перед запуском приложения:

- **`PAPAYU_LLM_API_URL`** — URL API (обязательно), например:
  - OpenAI: `https://api.openai.com/v1/chat/completions`
  - Ollama (локально): `http://localhost:11434/v1/chat/completions`
- **`PAPAYU_LLM_API_KEY`** — API-ключ (для OpenAI и облачных API; для Ollama можно не задавать).
- **`PAPAYU_LLM_MODEL`** — модель (по умолчанию `gpt-4o-mini`), для Ollama — например `llama3.2`.
- **`PAPAYU_LLM_STRICT_JSON`** — при `1`/`true` добавляет `response_format: { type: "json_schema", ... }` (OpenAI Structured Outputs; Ollama может не поддерживать).

**Поведение strict / best-effort:**
- Если strict включён: приложение отправляет `response_format` в API; при невалидном ответе — локальная валидация схемы отклоняет и выполняется **1 авто-ретрай** с repair-подсказкой («Верни ТОЛЬКО валидный JSON…»).
- Если strict выключен или провайдер не поддерживает: **best-effort** парсинг (извлечение из markdown), затем локальная валидация схемы; при неудаче — тот же repair-ретрай.

Пример для Ollama (без ключа, локально):

```bash
export PAPAYU_LLM_API_URL="http://localhost:11434/v1/chat/completions"
export PAPAYU_LLM_MODEL="llama3.2"
npm run tauri dev
```

После этого кнопка «Предложить исправления» будет строить план через выбранный LLM.

Если `PAPAYU_LLM_API_URL` не задан или пуст, используется встроенная эвристика (README, .gitignore, LICENSE, .env.example по правилам).

### Online Research (опционально)

Команда `research_answer_cmd`: поиск через Tavily → fetch страниц (SSRF-safe) → LLM summarize с источниками. Вызов через `researchAnswer(query)` на фронте.

**Env:**
- **`PAPAYU_ONLINE_RESEARCH=1`** — включить режим (по умолчанию выключен)
- **`PAPAYU_TAVILY_API_KEY`** — API-ключ Tavily (tavily.com)
- **`PAPAYU_ONLINE_MODEL`** — модель для summarize (по умолчанию из PAPAYU_LLM_MODEL)
- **`PAPAYU_ONLINE_MAX_SOURCES`** — макс. результатов поиска (default 5)
- **`PAPAYU_ONLINE_MAX_PAGES`** — макс. страниц для fetch (default 4)
- **`PAPAYU_ONLINE_PAGE_MAX_BYTES`** — лимит размера страницы (default 200000)
- **`PAPAYU_ONLINE_TIMEOUT_SEC`** — таймаут fetch (default 20)

**Use as context:** после online research кнопка «Use as context (once)» добавляет ответ в следующий PLAN/APPLY. Лимиты:
- **`PAPAYU_ONLINE_CONTEXT_MAX_CHARS`** — макс. символов online summary (default 8000)
- **`PAPAYU_ONLINE_CONTEXT_MAX_SOURCES`** — макс. источников (default 10)
- Online summary режется первым при превышении `PAPAYU_CONTEXT_MAX_TOTAL_CHARS`.

**Auto-use (X4):**
- **`PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT=1`** — если включено, online research результат автоматически используется как контекст для повторного `proposeActions` без участия пользователя (default 0).
- Защита от циклов: максимум 1 auto-chain на один запрос (goal).
- UI: при auto-use показывается метка "Auto-used ✓"; кнопка "Disable auto-use" отключает для текущего проекта (сохраняется в localStorage).

### Тестирование

- **Юнит-тесты (Rust)** — тесты для `detect_project_type`, `get_project_limits`, `is_protected_file`, `is_text_allowed` (см. `src-tauri/src/commands/get_project_profile.rs` и `apply_actions_tx.rs`). Запуск: `cd src-tauri && cargo test`.
- **E2E сценарий** — описание сценария «анализ → применение → undo» и критерии успеха см. в `docs/E2E_SCENARIO.md`.

### Архитектура

- **Фронт:** React 18, Vite 5, TypeScript; типы в `src/lib/types.ts`, единый API-слой в `src/lib/tauri.ts`; компоненты PathSelector, AgenticResult, хук useUndoRedo.
- **Бэкенд:** Tauri 2, Rust; команды в `src-tauri/src/commands/`, транзакции и undo/redo в `tx/`, verify с таймаутом в `verify.rs`.

## Документация

- `docs/IMPROVEMENTS.md` — рекомендации по улучшениям.
- `docs/E2E_SCENARIO.md` — E2E сценарий и критерии успеха.
- `docs/ОЦЕНКА_И_СЦЕНАРИЙ_РАССКАЗА.md` — оценка необходимости обновлений и сценарий рассказа о программе по модулям.
- `CHANGELOG.md` — история изменений по версиям.
