# Отладка EDIT_FILE на реальном файле (чеклист)

Этот документ — практический чеклист для end-to-end проверки v3 EDIT_FILE в papa-yu:
propose → preview → apply → (repair / fallback) → golden trace.

---

## Предварительные условия

### Включить трассы и протокол v3
Рекомендуемые переменные окружения:

- PAPAYU_TRACE=1
- PAPAYU_PROTOCOL_VERSION=3
- PAPAYU_LLM_STRICT_JSON=1 (если провайдер поддерживает response_format)
- PAPAYU_MEMORY_AUTOPATCH=0 (на время отладки, чтобы исключить побочные эффекты)
- PAPAYU_NORMALIZE_EOL=lf (если используешь нормализацию EOL)

Для Online fallback/notes (опционально):
- PAPAYU_ONLINE_RESEARCH=1
- PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT=1 (если хочешь тестировать auto-use)
- PAPAYU_TAVILY_API_KEY=...

---

## Цель проверки (Definition of Done)

Сценарий считается успешно пройденным, если:
1) v3 выдаёт APPLY с EDIT_FILE (и/или PATCH_FILE как fallback внутри v3),
2) preview показывает diff, apply применяет изменения,
3) base_sha256 проверяется, base mismatch ловится и чинится repair'ом (sha-injection),
4) ошибки anchor/before/ambiguous воспроизводимы и дают корректные коды ERR_EDIT_*,
5) golden traces v3 проходят (make test-protocol / cargo test golden_traces).

---

## Быстрый E2E сценарий (минимальный)

### Шаг 1 — выбрать простой файл
Выбери небольшой UTF-8 файл (лучше < 2000 строк), например:
- src/*.rs
- src/lib/*.ts
- любой текстовый конфиг (не secrets)

Избегай:
- бинарных/сжатых файлов
- автогенерации (dist/, build/, vendor/)
- protected paths (.env, *.pem, secrets/)

### Шаг 2 — PLAN
В UI:
- ввод: `plan: исправь <конкретная правка>`
или просто текст с явным "fix", чтобы сработала эвристика PLAN.

Ожидаемо:
- actions=[] (PLAN режим)
- summary объясняет, какой файл будет правиться и какие anchors будут использованы

### Шаг 3 — APPLY (OK)
Нажми OK / "apply" / "да".

Ожидаемо:
- actions содержит EDIT_FILE
- EDIT_FILE включает:
  - base_sha256 (64 hex)
  - edits[] (min 1)
  - anchor и before должны быть точными фрагментами из файла

### Шаг 4 — PREVIEW
Preview должен:
- показать unified diff
- bytes_before/bytes_after заполнены (если у тебя это в DiffItem)

Если preview падает — это уже диагностируемая ошибка (см. разделы ниже).

### Шаг 5 — APPLY
Apply должен:
- применить изменения
- записать файл
- если включён auto_check/run_tests — пройти (или корректно откатиться)
- в trace появится APPLY_SUCCESS или APPLY_ROLLBACK

---

## Где смотреть диагностику

### stderr события (runtime)
По trace_id в stderr:
- LLM_REQUEST_SENT / LLM_RESPONSE_OK / LLM_RESPONSE_REPAIR_RETRY
- VALIDATION_FAILED code=...
- PREVIEW_READY ...
- APPLY_SUCCESS / APPLY_ROLLBACK
- PROTOCOL_FALLBACK ... (если был)

### Трассы в .papa-yu/traces/
- основной propose trace: .papa-yu/traces/<trace_id>.json
- online research: online_<uuid>.json (если включено)

Ищи поля:
- protocol_default / protocol_attempts / protocol_fallback_reason / protocol_repair_attempt
- repair_injected_sha256, repair_injected_paths
- notes_injected (если notes включены)
- online_context_injected / online_context_dropped
- context_stats / cache_stats

---

## Типовые ошибки EDIT_FILE и как чинить

### ERR_NON_UTF8_FILE
Причина:
- файл не UTF-8 (байтовый/смешанная кодировка)

Действие:
- v3 должен fallback'нуть (обычно сразу) к v2 или отказаться и попросить альтернативу.
- если это код/текст — проверь, что файл реально UTF-8.

### ERR_EDIT_BASE_MISMATCH (или ERR_EDIT_BASE_SHA256_INVALID)
Причина:
- base_sha256 не совпал с текущим содержимым файла
- или base_sha256 не 64 hex

Ожидаемое поведение:
- repair prompt должен подставить правильный sha256 из контекста:
  `FILE[path] (sha256=...)`
- trace: repair_injected_sha256=true, repair_injected_paths=[path]

Как воспроизвести:
- вручную измени файл между PLAN и APPLY
- или подложи неправильный base_sha256 в фикстуре/в тесте

### ERR_EDIT_ANCHOR_NOT_FOUND
Причина:
- anchor строка отсутствует в файле

Чиним:
- anchor должен быть буквальным кусочком из `FILE[...]` блока
- лучше выбирать "устойчивый" anchor: сигнатура функции, имя класса, уникальный комментарий

### ERR_EDIT_BEFORE_NOT_FOUND
Причина:
- before не найден в окне вокруг anchor (±4000 chars по твоей текущей реализации)

Чиним:
- before должен быть рядом с anchor (не из другого участка файла)
- увеличить точность: добавить контекст в before (несколько слов/строк)

### ERR_EDIT_AMBIGUOUS
Причина:
- before встречается больше одного раза в окне вокруг anchor

Чиним:
- сделать before длиннее/уникальнее
- сделать anchor более узким/уникальным
- если в твоей реализации поддержан occurrence (для before), укажи occurrence явно; если нет — уточняй before.

### ERR_EDIT_APPLY_FAILED
Причина:
- внутренний сбой применения (невалидные индексы, неожиданные boundary, и т.п.)
- чаще всего: крайние случаи UTF-8 границ или очень большие вставки

Чиним:
- сократить before/after до минимального фрагмента
- избегать массовых замен/реформатирования
- если повторяется — добавь golden trace и воспроизведение

---

## Проверка repair-first и fallback (v3 → v2)

### Repair-first
Для ошибок из V3_REPAIR_FIRST:
- первый retry: repair_attempt=0
- второй (если не помог): fallback repair_attempt=1 → protocol override = 2

Проверяй в trace:
- protocol_repair_attempt: 0/1
- protocol_fallback_reason
- protocol_fallback_stage (обычно apply)

### Immediate fallback
Для ошибок из V3_IMMEDIATE_FALLBACK:
- fallback сразу (без repair), если так настроено

---

## Как сделать Golden trace из реального запуска

1) Убедись, что PAPAYU_TRACE=1
2) Выполни сценарий (PLAN→APPLY)
3) Найди trace_id в stderr (или в .papa-yu/traces/)
4) Сгенерируй fixture:
   - make golden TRACE_ID=<id>
   или
   - cargo run --bin trace_to_golden -- <trace_id> docs/golden_traces/v3/NNN_name.json
5) Прогон:
   - make test-protocol
   или
   - cargo test golden_traces

Совет:
- Делай отдельные golden traces для:
  - ok apply edit
  - base mismatch repair injected sha
  - anchor not found
  - no changes

---

## Реальные edge cases (на что смотреть)

1) Несколько одинаковых anchors в файле:
   - occurrence должен выбрать правильный (если модель указала)
2) before содержит повторяющиеся шаблоны:
   - ambiguity ловится, и это нормально
3) Window ±4000 chars не покрывает before:
   - значит before слишком далеко от anchor — модель ошиблась
4) Большие after-вставки:
   - риск превышения лимитов/перформанса
5) EOL normalization:
   - следи, чтобы diff не "красил" весь файл из-за CRLF→LF

---

## Мини-набор команд для быстрой диагностики

- Прогнать протокол-тесты:
  - make test-protocol

- Прогнать всё:
  - make test-all

- Посмотреть свежие traces:
  - ls -lt .papa-yu/traces | head

- Найти ошибки по коду:
  - rg "ERR_EDIT_" -n .papa-yu/traces
