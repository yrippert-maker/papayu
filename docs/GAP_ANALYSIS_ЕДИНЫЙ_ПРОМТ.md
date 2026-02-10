# Сопоставление PAPA YU с Единым рабочим промтом

**Источник:** `Единый_рабочий_промт.docx` (консолидация 16 ТЗ, февраль 2026)  
**Проект:** papa-yu v2.4.5 (Tauri + React)

---

## 1. Расхождение: Electron vs Tauri

| Спецификация | papa-yu |
|--------------|---------|
| Backend внутри Electron | **Tauri 2** (Rust backend) |
| REST API (GET /health, POST /tasks...) | **IPC-команды** (analyze_project, apply_actions_tx...) |
| Node.js в процессе | Без Node в runtime |

**Риск в документе:** «Двойственность Electron/Tauri» — Medium.  
**Рекомендация:** Оставить Tauri. Arch соответствует идее «UI + Backend = один процесс».

---

## 2. Definition of Done (MVP) — чеклист

| Критерий | Статус |
|----------|--------|
| Открываю приложение двойным кликом | ✅ `PAPA YU.app` |
| Сразу вижу экран Product Chat | ⚠️ Tasks — сложный экран, не «чистый Chat» |
| «⚡ Анализировать папку» — выбор каталога | ✅ pickFolder |
| Живой диалог со стадиями | ✅ agentic progress, события |
| Читаемый отчёт (findings, рекомендации) | ✅ |
| «⬇ Скачать отчёт» (JSON и MD) | ✅ |
| «Исправить автоматически» → preview → apply | ✅ |
| «Откатить» → файлы восстановлены | ✅ Undo |
| Выглядит как продукт, не dev-панель | ⚠️ На усмотрение |

---

## 3. UI: Product Chat

**Спецификация:** Один экран — Header + Chat + Composer.  
Без таблиц, без тех. панелей. Max-width 900px.

**Текущее состояние:** Tasks.tsx — много панелей (сессии, trends, weekly report, domain notes, project notes, fix groups, attachments). Ближе к «dashboard», чем к «chat».

**Рекомендация:** Вариант A — упростить до «Product Chat» (приоритет чата). Вариант B — оставить как есть, если продуктовая логика требует dashboard.

---

## 4. Persistence

| Спецификация | papa-yu |
|--------------|---------|
| userData/tasks.json | Проекты в `projects` (store.rs), сессии |
| userData/runs/&lt;runId&gt;.json | События в сессиях |
| userData/attachments/ | Нет upload ZIP — только folder |
| userData/artifacts/ | Отчёты в памяти / экспорт |
| userData/history/&lt;txId&gt;/ | tx/ (manifest, before/) |

**Gap:** Спецификация предполагает Upload ZIP. papa-yu — только выбор папки. Дополнить upload ZIP — фаза 2.

---

## 5. Auditor: правила анализа

**Спецификация:** минимум 15 правил (README, .env, tests, lockfile, дубликаты, utils/, components/, циклы, .editorconfig и т.д.).

**Текущее состояние:** Нужно проверить `analyze_project.rs` / rules — сколько правил реализовано.

---

## 6. Narrative — человеческий текст

**Спецификация:** Формат narrative:
> «Я проанализировал проект. Это React + Vite. Есть src/, нет tests/ — стоит добавить...»

**Текущее состояние:** В `report_md` и `narrative` — проверить тон (человеческий vs технический).

---

## 7. Safe Guards, лимиты, error codes

| Элемент | Спецификация | papa-yu |
|---------|--------------|---------|
| PATH_FORBIDDEN | .git, node_modules, target... | ✅ apply_actions_tx guard |
| LIMIT_EXCEEDED | max 50 actions, 2 MB, 50 files | ✅ limits.rs |
| AUTO_CHECK_FAILED_REVERTED | rollback при fail | ✅ |
| Error codes | TOOL_ID_REQUIRED, PATH_MISSING... | Частично (Rust Result) |

---

## 8. Бренд «PAPA YU»

**Спецификация:** Без дефисов, без «P PAPA YU», без «Tauri App».

**Проверено:** index.html, tauri.conf.json, Tasks.tsx, Cargo.toml — везде «PAPA YU».  
**Исключения:** docs/OPENAI_SETUP.md, start-with-openai.sh — «PAPA-YU» (мелко).

---

## 9. Части II–VI (вне PAPA YU)

| Часть | Содержание | Релевантность для papa-yu |
|-------|------------|---------------------------|
| II | Mura Menasa ERP | Отдельный продукт |
| III | Универсальный агент | Концепция, контракт агента |
| IV | Scorer, Deps Graph, Patches | Аналитический движок — фаза 3 |
| V | Due Diligence, Data Room, Seed | Инфраструктура продажи |
| VI | Риски, дорожная карта | Справочно |

---

## 10. Приоритетные задачи (Фаза 1 по документу)

| # | Задача | Статус | Действие |
|---|--------|--------|----------|
| 1 | Auditor v2: 15 правил + narrative + score | ✅ | Реализовано 15+ правил (README, .gitignore, .env, tests, lockfile, .editorconfig, scripts, empty dirs, large files, utils/, large dir, monolith, prettier, CI) |
| 2 | Folder analysis без ZIP | ✅ | Уже есть pickFolder |
| 3 | Undo (1 шаг) via snapshot | ✅ | Undo/Redo стек |
| 4 | Бренд PAPA YU везде | ⚠️ | Исправить OPENAI_SETUP, start-with-openai |
| 5 | CI: lint + test + build | ? | Проверить .github/workflows |
| 6 | README.md, ARCHITECTURE.md | ✅ | Есть |

---

## 11. Рекомендуемые первые шаги

1. **Аудит правил Auditor** — подсчитать реализованные правила, привести к 15+.
2. **Правки бренда** — заменить «PAPA-YU» на «PAPA YU» в docs и скриптах.
3. **Проверка CI** — убедиться, что lint + test + build выполняются.
4. **Опционально: режим Product Chat** — упрощённый UI как альтернативный вид (если требуется строгое соответствие спецификации).

---

*Документ создан автоматически по результатам сопоставления с Единым рабочим промтом.*
