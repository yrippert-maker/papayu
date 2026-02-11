# Контракты UI ↔ Tauri

Единый источник правды для вызовов команд и форматов ответов.

---

## Стандарт ответов команд

Рекомендуемый формат (по возможности):

- **Успех:** `{ ok: true, ...data }` или возврат типа `AnalyzeReport`, `PreviewResult`, `ApplyResult`, `UndoResult`.
- **Ошибка:** `Result::Err(String)` или поле `ok: false` с `error`, `error_code`, при необходимости `details`.

Текущие команды уже возвращают типы с полями `ok`, `error`, `error_code` где применимо.

---

## Команды (invoke)

| Команда | Вход | Выход | Файл UI |
|---------|------|-------|---------|
| `analyze_project` | `{ path: string }` | `AnalyzeReport` | lib/analyze.ts |
| `preview_actions` | `{ payload: { path, actions } }` | `PreviewResult` | Tasks.tsx |
| `apply_actions` | `{ payload: { path, actions } }` | `ApplyResult` | Tasks.tsx |
| `undo_last` | `{ path: string }` | `UndoResult` | Tasks.tsx |
| `get_app_info` | — | `AppInfo { version, app_data_dir, app_config_dir }` | Diagnostics.tsx |

---

## События (listen)

| Событие | Payload | Где эмитится | Где слушается |
|---------|---------|--------------|----------------|
| `analyze_progress` | `string` (сообщение) | analyze_project, apply_actions, preview_actions, undo_last | Tasks.tsx |

Типы payload в будущем можно версионировать (например, `{ v: 1, message: string }`) при изменении формата.

---

## Apply / Undo (транзакционность)

- **apply_actions:** создаёт snapshot перед применением; при ошибке откатывает изменения (revert_snapshot). Сессия хранится в `app_data_dir/history/<session_id>`.
- **undo_last:** восстанавливает последнюю сессию из `last_session.txt`. Откат атомарный по сессии.

Рекомендация: при расширении — сохранять единый формат манифеста сессии (список путей + действия) для воспроизводимости.
