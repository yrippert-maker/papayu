# План Protocol v3

План развития протокола — без внедрения. v2 решает «перезапись файла» через PATCH_FILE, но патчи всё ещё бывают хрупкими.

---

## Вариант v3-A (рекомендуемый): EDIT_FILE с операциями

Новый action:

```json
{
  "kind": "EDIT_FILE",
  "path": "src/foo.py",
  "base_sha256": "...",
  "edits": [
    {
      "op": "replace",
      "anchor": "def parse(",
      "before": "return value.strip()",
      "after": "if value is None:\n    return \"\"\nreturn value.strip()"
    }
  ]
}
```

**Плюсы:**

- Устойчивее к line drift (якорь по содержимому, не по номерам строк)
- Проще валидировать «что именно поменялось»
- Меньше риска ERR_PATCH_APPLY_FAILED

**Минусы:**

- Нужен свой «якорный» редактор
- Якорь должен быть уникальным в файле

**MVP для v3:**

- Оставить PATCH_FILE как fallback
- Добавить EDIT_FILE только для текстовых файлов
- Engine: «найди anchor → проверь before → замени на after»
- base_sha256 остаётся обязательным

---

## Вариант v3-B: AST-level edits (язык-специфично)

Для Python/TS можно делать по AST (insert/delete/replace узлов). Плюсы: максимальная точность. Минусы: значительно больше работы, сложнее поддерживать, нужно знать язык.

---

## Совместимость v1/v2/v3

- v1: UPDATE_FILE, CREATE_FILE, …
- v2: + PATCH_FILE, base_sha256
- v3: + EDIT_FILE (якорные операции), PATCH_FILE как fallback

Выбор активного протокола по env. v3 совместим с v2 (EDIT_FILE — расширение).
