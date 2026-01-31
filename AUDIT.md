# Полный аудит приложения PAPA YU

**Дата:** 2026-01-28  
**Цель:** проверка компонентов, связей UI ↔ backend, исправление отображения блока выбора пути к папкам и заключение.

---

## 1. Структура проекта

| Путь | Назначение |
|------|------------|
| `src/` | React (Vite) — UI |
| `src/pages/Tasks.tsx` | Страница «Задачи» — анализ проекта, выбор папок, preview/apply/undo/redo |
| `src/pages/Dashboard.tsx` | Страница «Панель управления» |
| `src/App.tsx` | Роутинг, Layout (header + main) |
| `src-tauri/src/` | Rust (Tauri) — команды, tx, types |

---

## 2. Проверенные компоненты

### 2.1 UI

- **App.tsx** — маршруты `/` (Tasks), `/control-panel` (Dashboard). Layout: header с навигацией, main с `overflow: visible` (исправлено).
- **Tasks.tsx** — блок «Путь к папке проекта»:
  - Расположен **первым** под заголовком «Анализ проекта».
  - Секция с `data-section="path-selection"` и классом `tasks-sources`.
  - Две кнопки: **«Выбрать папку»** (основная синяя), **«+ Добавить ещё папку»**.
  - Список выбранных папок или текст «Папки не выбраны. Нажмите кнопку «Выбрать папку» выше.».
  - Ниже: поле ввода пути и кнопка «Отправить».
- **index.css** — правило для `.tasks-sources[data-section="path-selection"]`: `display: block !important`, `visibility: visible !important`, чтобы блок не скрывался.

### 2.2 Связи UI → Backend (invoke)

| Действие в UI | Команда Tauri | Файл Rust |
|---------------|---------------|-----------|
| Загрузка списка папок при монтировании | `get_folder_links` | folder_links.rs |
| Сохранение списка папок | `set_folder_links` (links: { paths }) | folder_links.rs |
| Анализ + preview + apply (пакет) | `run_batch_cmd` (payload: paths, confirm_apply, auto_check, selected_actions) | run_batch.rs |
| Состояние undo/redo | `get_undo_redo_state_cmd` | undo_last.rs, tx/store.rs |
| Откат | `undo_last` | undo_last.rs |
| Повтор | `redo_last` | redo_last.rs |
| Генерация плана (v2.4) | `generate_actions` | generate_actions.rs |

Выбор папки через диалог: `open({ directory: true })` из `@tauri-apps/plugin-dialog` — плагин зарегистрирован в `lib.rs` (`tauri_plugin_dialog::init()`).

---

## 3. Backend (Rust)

### 3.1 Зарегистрированные команды (lib.rs)

- `analyze_project_cmd`, `preview_actions_cmd`, `apply_actions_cmd`, `run_batch_cmd`
- `undo_last`, `undo_available`, `redo_last`, `get_undo_redo_state_cmd`
- `generate_actions`
- `get_folder_links`, `set_folder_links`

### 3.2 Модули

- **commands/** — analyze_project, apply_actions, preview_actions, run_batch, undo_last, redo_last, generate_actions, folder_links, auto_check.
- **tx/** — limits (preflight), store (undo/redo stacks), mod (snapshot_before, rollback_tx, apply_actions_to_disk, collect_rel_paths, write_manifest, read_manifest, etc.).
- **types** — ApplyPayload, ApplyResult, TxManifest, Action, ActionKind, AnalyzeReport, BatchPayload, BatchEvent, etc.

### 3.3 Folder links

- `FolderLinks { paths: Vec<String> }` — сериализуется в `app_data_dir/folder_links.json`.
- `load_folder_links`, `save_folder_links` — используются в `get_folder_links` / `set_folder_links`.

Связь с UI: при загрузке Tasks вызывается `get_folder_links` и при необходимости обновляется `folderLinks`; при добавлении/удалении папки вызывается `set_folder_links`. Формат `{ links: { paths } }` соответствует типу `FolderLinks`.

---

## 4. Внесённые исправления

1. **Блок выбора пути к папке (Tasks.tsx)**
   - Секция «Путь к папке проекта» вынесена в начало страницы (сразу под заголовком).
   - Заголовок секции: «Путь к папке проекта», подпись с указанием нажать кнопку или ввести путь.
   - Кнопки «Выбрать папку» и «+ Добавить ещё папку» оформлены заметно (размер, контраст, тень у основной).
   - Добавлены `className="tasks-sources"` и `data-section="path-selection"` для стилей и отладки.
   - Секция с рамкой, фоном и `minHeight: 140px`, чтобы блок всегда занимал место и был виден.
   - В строке ввода оставлены только поле пути и «Отправить» (дублирующая кнопка «Выбрать папку» убрана, чтобы не путать с блоком выше).

2. **Layout (App.tsx)**  
   - Для `main` заданы `overflow: visible` и `minHeight: 0`, чтобы контент не обрезался.

3. **Глобальные стили (index.css)**  
   - Добавлено правило для `.tasks-sources[data-section="path-selection"]`: блок принудительно видим.

---

## 5. Рекомендации после обновления кода

- Перезапустить приложение: `cd papa-yu/src-tauri && cargo tauri dev`.
- В браузере/WebView сделать жёсткое обновление (Ctrl+Shift+R / Cmd+Shift+R), чтобы подтянуть новый UI без кэша.
- Если используется только фронт (Vite): перезапустить `npm run dev` и обновить страницу.

После этого в начале страницы «Задачи» должен отображаться блок «Путь к папке проекта» с кнопками «Выбрать папку» и «+ Добавить ещё папку» и списком выбранных папок.

---

## 6. Заключение

- **Компоненты:** App, Tasks, Dashboard, Layout и точки входа (main.tsx, index.html) проверены; маршруты и вложенность корректны.
- **Связи UI ↔ backend:** вызовы `get_folder_links`, `set_folder_links`, `run_batch_cmd`, `get_undo_redo_state_cmd`, `undo_last`, `redo_last` соответствуют зарегистрированным командам и типам (FolderLinks, BatchPayload, ApplyPayload и т.д.).
- **Исправления:** блок выбора пути к папкам сделан первым и визуально выделен; добавлены гарантии видимости через разметку и CSS; дублирование кнопки убрано.
- **Ошибки:** явных ошибок в компонентах и связях не выявлено. Если на экране по-прежнему не видно кнопок и блока, наиболее вероятны кэш сборки или WebView — выполнить перезапуск и жёсткое обновление по п. 5.

Аудит выполнен. Состояние: **исправления внесены, рекомендации по обновлению даны.**
