# Тест AUTO_ROLLBACK (v2.3.3)

Проверка: первый шаг применяется, второй падает → откат первого шага, в UI сообщения «Обнаружены ошибки. Откатываю изменения…» и «Изменения привели к ошибкам, откат выполнен.»

## Формат payload в papa-yu

- Команда: `apply_actions` (или `apply_actions_cmd`).
- Payload: `ApplyPayload` с полями **`root_path`**, **`actions`**, **`auto_check`**.
- В **`actions`** поле **`kind`** в формате **SCREAMING_SNAKE_CASE**: `CREATE_FILE`, `UPDATE_FILE`, `DELETE_FILE`, `CREATE_DIR`, `DELETE_DIR`.

## Вариант 1 — падение на safe_join (..)

Подставь свой путь в `root_path` и вызови apply с `actions` из `test-auto-rollback-payload.json`:

1. Создаётся `papayu_test_ok.txt`.
2. Второй action с путём `../../forbidden.txt` → `safe_join` возвращает ошибку.
3. Rollback удаляет `papayu_test_ok.txt`.
4. Ответ: `ok: false`, `error_code: "AUTO_ROLLBACK_DONE"`, `failed_at: 1`.

## Вариант 2 — падение через ОС (permission denied)

Используй `test-auto-rollback-fs-payload.json`: второй шаг пишет в `/System/...` — в papa-yu абсолютный путь отсекается в `safe_join`, так что отказ будет до записи в ФС, результат тот же (AUTO_ROLLBACK_DONE + откат).

## Запуск

```bash
cd ~/Desktop/papa-yu/src-tauri && cargo tauri dev
```

Проверку можно делать через UI (предпросмотр → применить с действиями, которые содержат запрещённый путь) или через invoke с payload из JSON выше.
