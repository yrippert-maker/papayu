# Changelog

Все значимые изменения в проекте PAPA YU фиксируются в этом файле.

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.0.0/).

---

## [2.4.4] — 2025-01-31

### Protocol stability (v1)

- **Schema version:** `LLM_PLAN_SCHEMA_VERSION=1`, `x_schema_version` в схеме, `schema_hash` (sha256) в trace.
- **Версионирование:** при изменении контракта ответа LLM — увеличивать schema_version; trace содержит schema_version и schema_hash для воспроизводимости.
- **Рекомендуемый тег:** `v1.0.0` или `v0.x` — зафиксировать «стабильный релиз» перед введением v2.

### Добавлено

- **UX:** история сессий по проекту — блок «История сессий» с раскрывающимся списком сессий (дата, количество событий, последнее сообщение); обновление списка после agentic run.
- **UX:** в блоке профиля отображаются лимиты (max_actions_per_tx, timeout_sec).
- **UX:** фильтр расширений в диалоге «Прикрепить файл» (исходники и конфиги: .ts, .tsx, .js, .jsx, .rs, .py, .json, .toml, .md, .yml, .yaml, .css, .html, .xml).
- **UX:** горячие клавиши — Ctrl+Enter (Cmd+Enter): отправить/запустить анализ; Escape: сбросить превью изменений.
- **UX:** тёмная тема — переключатель в боковой панели, CSS-переменные для обоих режимов, сохранение выбора в localStorage, поддержка системных настроек.
- **UX:** экспорт/импорт настроек — кнопки в боковой панели для сохранения и восстановления всех настроек (проекты, профили, сессии, папки) в JSON-файл.
- **Тестирование:** юнит-тесты в Rust для `detect_project_type`, `get_project_limits`, `is_protected_file`, `is_text_allowed`, `settings_export` (18 тестов).
- **Тестирование:** тестовые фикстуры в `tests/fixtures/` — минимальные проекты для E2E тестирования (minimal-node, minimal-rust).
- **Документация:** E2E сценарий в `docs/E2E_SCENARIO.md`; обновлён README до v2.4.4; README для тестов в `tests/README.md`.
- **Контекст прикреплённых файлов:** в отчёт и batch передаётся список прикреплённых файлов (`attached_files` в `BatchPayload` и `AnalyzeReport`); фронт передаёт его при вызове `runBatchCmd`.
- **LLM-планировщик:** при заданном `PAPAYU_LLM_API_URL` команда «Предложить исправления» вызывает OpenAI-совместимый API (OpenAI, Ollama и др.); ответ парсится в план действий (CREATE_FILE, CREATE_DIR и т.д.). Без настройки — эвристический план по отчёту.
- **Бэкенд:** команды `export_settings` и `import_settings` для резервного копирования и переноса настроек между машинами.
- **Конфиг:** расширенный allowlist команд verify (`verify_allowlist.json`) — добавлены cargo clippy, tsc --noEmit, mypy, pytest --collect-only.
- **Инфраструктура:** инициализирован Git-репозиторий с улучшенным .gitignore.
- **Preview diff в propose flow:** после получения плана автоматически вызывается `preview_actions`, diffs отображаются в UI.
- **ERR_UPDATE_WITHOUT_BASE:** в режиме APPLY UPDATE_FILE разрешён только для файлов, прочитанных в Plan (FILE[path] или === path ===).
- **Protected paths:** denylist для `.env`, `*.pem`, `*.key`, `*.p12`, `id_rsa*`, `**/secrets/**`.
- **Content validation:** запрет NUL, >10% non-printable = ERR_PSEUDO_BINARY; лимиты max_path_len=240, max_actions=200, max_total_content_bytes=5MB.
- **EOL:** `PAPAYU_NORMALIZE_EOL=lf` — нормализация \r\n→\n и trailing newline.
- **Наблюдаемость:** trace_id (UUID) на каждый propose; лог-ивенты LLM_REQUEST_SENT, LLM_RESPONSE_OK, VALIDATION_FAILED, APPLY_SUCCESS, APPLY_ROLLBACK, PREVIEW_READY.
- **Трассировка:** `PAPAYU_TRACE=1` — запись в `.papa-yu/traces/<trace_id>.json`.
- **Детерминизм LLM:** temperature=0, max_tokens=65536, top_p=1, presence_penalty=0, frequency_penalty=0 (PAPAYU_LLM_TEMPERATURE, PAPAYU_LLM_MAX_TOKENS).
- **Capability detection:** при ошибке API response_format — автоматический retry без response_format (Ollama и др.).
- **Schema version:** `x_schema_version` в llm_response_schema.json; schema_hash (sha256) в trace; LLM_PLAN_SCHEMA_VERSION в prompt.
- **Кеш контекста:** read_file/search/logs/env кешируются в plan-цикле; CONTEXT_CACHE_HIT/MISS.
- **Контекст-диета:** PAPAYU_CONTEXT_MAX_FILES=8, MAX_FILE_CHARS=20k, MAX_TOTAL_CHARS=120k; head+tail truncation; MIN_CHARS_FOR_PRIORITY0=4k; CONTEXT_DIET_APPLIED.
- **Trace:** context_stats (files_count, dropped, total_chars, logs_chars, truncated) и cache_stats (hits/misses по env/logs/read/search, hit_rate).
- **Кеш logs:** ключ Logs включает `last_n` — разные last_n не пересекаются.
- **Golden traces:** эталонные fixtures в `docs/golden_traces/v1/` — формат protocol/request/context/result (без raw_content). Тест `golden_traces_v1_validate` валидирует schema_version, schema_hash, JSON schema, validate_actions, NO_CHANGES при apply+empty. Конвертер `trace_to_golden` (cargo run --bin trace_to_golden).
- **Compatibility matrix:** в PROTOCOL_V1.md — Provider Compatibility таблица и 5 поведенческих гарантий.
- **PROTOCOL_V2_PLAN.md:** план v2 (PATCH_FILE, REPLACE_RANGE, base_sha256).
- **make/npm shortcuts:** `make golden` (trace→fixture), `make test-protocol` (golden_traces_v1_validate).
- **CI:** `.github/workflows/protocol-check.yml` — golden_traces_v1_validate на push/PR.
- **Политика golden traces:** в docs/golden_traces/README.md — когда/как обновлять, при смене schema_hash.
- **Protocol v2 schema (plumbing):** `llm_response_schema_v2.json` — object-only, PATCH_FILE, base_sha256. `PAPAYU_PROTOCOL_VERSION=1|2` (default 1). schema_version и schema_hash динамические в trace.

### Изменено

- Лимиты профиля применяются в `apply_actions_tx` и `run_batch` — при превышении `max_actions_per_tx` возвращается ошибка TOO_MANY_ACTIONS.
- Таймаут проверок в verify и auto_check задаётся из профиля (`timeout_sec`); в `verify_project` добавлен таймаут на выполнение каждой проверки (spawn + try_wait + kill при превышении).
- Синхронизированы версии в package.json, Cargo.toml и tauri.conf.json.

---

## [2.4.3] — ранее

### Реализовано

- Профиль по пути (тип проекта, лимиты, goal_template).
- Agentic run — цикл анализ → план → превью → применение → проверка → откат при ошибке.
- Прикрепление файлов, кнопка «Прикрепить файл».
- Guard опасных изменений (is_protected_file, is_text_allowed).
- Подтверждение Apply (user_confirmed).
- Единый API-слой (src/lib/tauri.ts), типы в src/lib/types.ts.
- Компоненты PathSelector, AgenticResult, хук useUndoRedo.
- Транзакционное apply с snapshot и откатом при падении auto_check.
- Undo/Redo по последней транзакции.
- Единый batch endpoint (run_batch): analyze → preview → apply (при confirmApply) → autoCheck.

---

## [2.3.2] — ранее

- Apply + Real Undo (snapshot в userData/history, откат при падении check).
- AutoCheck для Node, Rust, Python.
- Actions: README, .gitignore, tests/, .env.example.
- UX: двухфазное применение, кнопки «Показать исправления», «Применить», «Отмена», «Откатить последнее».
- Folder Links (localStorage + userData/folder_links.json).
- Брендинг PAPA YU, минимальный размер окна 1024×720.
