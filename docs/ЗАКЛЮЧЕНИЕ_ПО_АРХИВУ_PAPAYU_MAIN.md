# Заключение по анализу архива papayu-main.zip

**Дата анализа:** 8 февраля 2026  
**Источник:** `/Users/yrippertgmail.com/Downloads/papayu-main.zip`  
**Коммит в архиве:** db21971761ff9305a92bd365c5f20481d32a8aca

---

## 1. Общая характеристика

Архив содержит **форк/альтернативную версию PAPA YU** с другой архитектурой и набором функций. Это **десктопное приложение Tauri 2 + React**, объединяющее:

- **Ядро PAPA YU** — анализ проектов, preview/apply/undo
- **Модули Mura Menasa ERP** — регламенты, ТМЦ/закупки, финансы, персонал
- **Инфраструктурные страницы** — Policy Engine, Audit Logger, Secrets Guard, Updates, Diagnostics

---

## 2. Структура проекта

| Путь | Назначение |
|------|------------|
| `desktop/` | Tauri + React (основное приложение) |
| `desktop/src-tauri/` | Rust backend (команды, типы) |
| `desktop/ui/` | React UI (Vite, TypeScript, Tailwind) |
| `desktop-core/` | Отдельный слой (Node/TS) — **пустой** |
| `desktop-core/tools/project-auditor/` | `index.ts` — **0 байт** (заглушка) |
| `docs/` | CONTRACTS.md, частично повреждённые файлы при распаковке |

---

## 3. Backend (Rust)

### 3.1 Команды Tauri

| Команда | Назначение |
|---------|------------|
| `analyze_project` | Анализ папки, findings, recommendations, actions |
| `preview_actions` | Превью изменений (diff) |
| `apply_actions` | Применение с snapshot и откатом при ошибке |
| `undo_last` | Откат последней сессии |
| `get_app_info` | Версия, app_data_dir |

**Отсутствуют** (по сравнению с papa-yu на Desktop):  
`run_batch`, `agentic_run`, `generate_actions_from_report`, `propose_actions`, `redo_last`, `get_folder_links`, `set_folder_links`, `get_project_profile`, `trends`, `weekly_report`, `domain_notes`, `settings_export`, `verify_project`, `auto_check`.

### 3.2 Анализатор (analyze_project.rs)

- **~750 строк** — детальный сканер с `ScanState`
- **Правила:** README, .gitignore, .env, LICENSE, tests/, много файлов в корне, глубокая вложенность, ESLint, Clippy, тип проекта
- **Прогресс:** эмит `analyze_progress` на стадиях
- **Лимиты:** MAX_FILES=50_000, MAX_DURATION_SECS=60
- **Типы:** `AnalyzeReport`, `ProjectContext`, `LlmContext`, `ReportStats`, `Finding`, `Recommendation`

### 3.3 Транзакционность (apply_actions)

- Snapshot перед применением
- `revert_snapshot` при ошибке
- Сессии в `app_data_dir/history/<session_id>`
- `last_session.txt` для undo

**Нет:** auto_check (cargo check / npm run build), лимитов из профиля, user_confirmed, двухстекового undo/redo.

---

## 4. Frontend (React)

### 4.1 Страницы

| Маршрут | Страница | Реализация |
|---------|----------|------------|
| `/tasks` | Tasks | Основной экран — анализ, превью, apply, undo |
| `/reglamenty` | Reglamenty | Регламенты (АРМАК, ФАА, ЕАСА) |
| `/tmc-zakupki` | TMCZakupki | ТМЦ и закупки |
| `/finances` | Finances | Финансы |
| `/personnel` | Personnel | Персонал |
| `/control-panel` | Dashboard | Панель управления |
| `/policies` | PolicyEngine | Движок политик |
| `/audit` | AuditLogger | Журнал аудита |
| `/secrets` | SecretsGuard | Защита секретов |
| `/updates` | Updates | Обновления (tauri-plugin-updater) |
| `/diagnostics` | Diagnostics | Версии, пути, логи |

### 4.2 Стек

- React 19, Vite 7, TypeScript 5.9
- Tailwind CSS, anime.js, lucide-react, zustand
- tauri-plugin-dialog, tauri-plugin-updater, tauri-plugin-process

### 4.3 Tasks.tsx

- **~38 000 строк** (очень большой файл)
- Чат, история, выбор папки, анализ, превью, apply, undo
- Поле «Чат с агентом» — заглушка: «Ответ ИИ агента будет отображаться здесь»

---

## 5. CI/CD

- **ci.yml:** lint (ESLint), TypeScript check, `cargo check`
- **Нет:** `cargo test`, `cargo clippy`, `cargo fmt`, `cargo audit`
- **release.yml:** сборка релизов по тегам `v*`

---

## 6. Сравнение с papa-yu (Desktop)

| Аспект | papayu-main | papa-yu (Desktop) |
|--------|-------------|-------------------|
| Структура | desktop/ + desktop-core/ | src/ + src-tauri/ (единая папка) |
| Команды Rust | 5 | 20+ |
| Agentic run | ❌ | ✅ |
| LLM planner | ❌ | ✅ |
| Undo/Redo | 1 шаг | Двухстековый |
| AutoCheck | ❌ | ✅ (cargo check, npm build) |
| Профиль проекта | Базовый | Детальный (лимиты, goal_template) |
| Online Research | ❌ | ✅ (Tavily) |
| Domain notes | ❌ | ✅ |
| Trends | ❌ | ✅ |
| ERP-страницы | ✅ (заглушки) | ❌ |
| Plugin updater | ✅ | ❌ |
| CI | lint + check | fmt + clippy + test + audit + frontend build |

---

## 7. Выводы

### 7.1 Сильные стороны архива

1. **Широкая оболочка** — маршруты для ERP (Регламенты, ТМЦ, Финансы, Персонал) и инфраструктуры (Audit, Secrets, Diagnostics, Updates).
2. **Архитектура** — CONTRACTS.md фиксирует контракты UI ↔ Tauri.
3. **Транзакционность** — snapshot + revert при ошибке apply.
4. **Прогресс** — эмит событий на стадиях анализа.
5. **Современный стек** — React 19, Vite 7, Tauri 2.9.

### 7.2 Слабые стороны и риски

1. **desktop-core пустой** — `project-auditor/index.ts` = 0 байт, слой не реализован.
2. **ERP-страницы** — скорее заглушки, реальной логики (БД, API) нет.
3. **Chat Agent** — заглушка, ИИ не подключён.
4. **CI** — нет тестов, clippy, audit, что снижает надёжность.
5. **Tasks.tsx** — 38k строк, монолитный, сложно поддерживать.
6. **Нет LLM/агента** — в отличие от papa-yu, нет propose_actions, agentic_run.

### 7.3 Рекомендация

Архив **papayu-main** — это **более ранняя/параллельная ветка** с акцентом на ERP-оболочку и минимальный набор команд анализа. Для **продуктового PAPA YU** (анализ + автоисправления + agentic run) **текущая papa-yu** (Desktop) значительно функциональнее.

При необходимости объединения:
- взять из papayu-main: структуру маршрутов ERP, CONTRACTS.md, tauri-plugin-updater;
- сохранить из papa-yu: agentic_run, LLM planner, AutoCheck, undo/redo стек, domain notes, trends.

---

*Документ создан по результатам анализа архива papayu-main.zip.*
