# План Protocol v2

Минимальный набор изменений для v2 — без «воды».

---

## 3.1. Главная цель v2

Снизить риск/стоимость «UPDATE_FILE целиком» и улучшить точность правок:
- частичные патчи,
- «операции редактирования» вместо полной перезаписи.

---

## 3.2. Минимальный набор изменений

### A) Новый action kind: `PATCH_FILE`

Вместо полного `content`, передаётся unified diff:

```json
{ "kind": "PATCH_FILE", "path": "src/app.py", "patch": "@@ -1,3 +1,4 @@\n..." }
```

- Валидация патча локально.
- Применение патча транзакционно.
- Preview diff становится тривиальным.

### B) Новый action kind: `REPLACE_RANGE`

Если unified diff сложен:

```json
{
  "kind": "REPLACE_RANGE",
  "path": "src/app.py",
  "start_line": 120,
  "end_line": 180,
  "content": "новый блок"
}
```

Плюсы: проще валидировать. Минусы: зависит от line numbers (хрупко при изменениях).

### C) «Base hash» для UPDATE/PATCH

Исключить race (файл изменился между plan/apply):

```json
{ "kind": "PATCH_FILE", "path": "...", "base_sha256": "...", "patch": "..." }
```

Если hash не совпал → Err и переход в PLAN.

---

## 3.3. Совместимость v1/v2

- `schema_version=1` → нынешний формат (UPDATE_FILE, CREATE_FILE, …).
- `schema_version=2` → допускает `PATCH_FILE` / `REPLACE_RANGE` и расширенные поля.

В коде:
- Компилировать обе схемы: `llm_response_schema.json` (v1), `llm_response_schema_v2.json`.
- Выбор активной по env: `PAPAYU_PROTOCOL_VERSION=1|2` (default 1).
- Валидация/парсер: сначала проверить schema v2 (если включена), иначе v1.

---

## 3.4. Порядок внедрения v2 без риска

1. Добавить v2 schema + валидаторы + apply engine, **не включая по умолчанию**.
2. Добавить «LLM prompt v2» (рекомендовать PATCH_FILE вместо UPDATE_FILE).
3. Прогнать на своих проектах и собрать golden traces v2.
4. Когда стабильно — сделать v2 дефолтом, сохранив совместимость v1.
