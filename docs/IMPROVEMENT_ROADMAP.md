# Практические рекомендации по улучшению papa-yu

Упорядочено по эффекту/риску. Привязано к стеку: Rust, Tauri, CI в GitHub Actions, `cargo test` + golden traces, частичные SSRF-защиты, нет формализованных инцидентов/метрик.

---

## 1) Самое важное: закрыть класс рисков SSRF / небезопасный fetch (Security, Critical/High)

### Что сделать

1. **Единая точка сетевого доступа** — вынести все HTTP-запросы в один модуль (`net::client`), запретить прямой `reqwest::get()` где попало.

2. **Политика allowlist + запрет приватных сетей**
   - разрешённые схемы: `https` (и `http` только если надо)
   - запрет `file://`, `ftp://`, `gopher://`, `data:` и т.п.
   - запрет IP: RFC1918, loopback, link-local
   - защита от DNS-rebind (резолвить и проверять IP)

3. **Таймауты и лимиты** — connect/read timeout, max size ответа, ограничение редиректов.

4. **Тесты на SSRF** — набор URL → ожидаемый "deny", golden traces для фиксации отказов.

---

## 2) Минимальная наблюдаемость и журнал инцидентов (Ops, High)

### MVP за 1–2 дня

1. **Единый структурированный лог** — JSON, уровни error/warn/info/debug, корреляционный id, без секретов.

2. **Метрики уровня приложения** — latency ключевых операций, количество ошибок по типам.

3. **`INCIDENTS.md`** — шаблон: дата, версия, симптом, impact, причина, фикс, тест на повтор.

---

## 3) Усилить CI/CD как quality gate (DevEx/Quality, High)

### Минимальный набор гейтов

1. `cargo fmt --check`, `cargo clippy -- -D warnings`
2. `cargo test` (включая golden traces)
3. `cargo deny`, `cargo audit` — supply chain
4. (Опционально) SBOM для релизов

---

## 4) Архитектурные границы (Architecture/Tech debt, Medium/High)

- Чёткие слои: `domain` (без IO) → `services` → `adapters` → `tauri_api`
- ADR для 3–5 ключевых решений

---

## 5) Качество кода (Medium)

- Лимиты сложности, `thiserror` для доменных ошибок, вычистка dead code.

---

## 6) Производительность (Medium)

- Выделить 3–5 «дорогих» операций, измерять время/память, микробенчи (`criterion`).

---

## Приоритизированный roadmap

| Фаза | Срок | Действия |
|------|------|----------|
| Quick wins | 1–5 дней | SSRF: единая точка + denylist + таймауты; CI: fmt/clippy/test + cargo audit/deny; INCIDENTS.md + логи |
| Mid-term | 1–3 нед | Архитектурные границы; ADR; метрики по 3–5 операциям |
| Long-term | 1–2 мес | SBOM; property-based тесты; формализация SLO |

> **Выполнено (2025-01-31):** см. `docs/IMPROVEMENT_REPORT.md`

---

## Приложение: ответы на запрос данных для точного плана

### 5–10 строк: функции fetch/скачивание/импорт и источник URL

| Функция / модуль | URL откуда | Защита |
|------------------|------------|--------|
| `online_research/fetch.rs` → `fetch_url_safe()` | URL из ответа **Tavily Search API** (результаты поиска) | ✅ SSRF: localhost, RFC1918, link-local, `user:pass@`, max 2048 символов |
| `online_research/search.rs` | POST `https://api.tavily.com/search` — фиксированный URL | ✅ Не извне |
| `llm_planner.rs`, `weekly_report.rs`, `domain_notes/distill.rs`, `online_research/llm.rs` | `PAPAYU_LLM_API_URL` из env (OpenAI/Ollama) | ⚠️ Конфиг, не от пользователя |

**Единственный «внешний» URL-поток:** Tavily возвращает URL в результатах поиска → `fetch_url_safe()` их скачивает. Уже есть `is_url_allowed()` и лимиты.

### Хранение данных и синхронизация

- **Файлы JSON**, без БД:
  - `store/`: `projects.json`, `project_profiles.json`, `sessions.json` в `app_data_dir`
  - `.papa-yu/notes/domain_notes.json` — заметки по проекту
  - `.papa-yu/cache/online_search_cache.json` — кеш Tavily
  - `.papa-yu/traces/*.json` — трассировки
  - `.papa-yu/project.json` — настройки проекта
- **Синхронизации нет** — только локальные файлы.

### 3 главные боли (по коду и статусу)

1. **llm_planner.rs** — большой модуль, протоколы v1/v2/v3, fallback-логика, repair, memory patch. Сложно тестировать и менять.
2. **PATCH/EDIT apply** — ERR_EDIT_AMBIGUOUS, ERR_EDIT_BEFORE_NOT_FOUND, base_sha256 mismatch; fallback v3→v2→v1 добавляет ветвления.
3. **Golden traces** — при изменении JSON Schema нужно обновлять `schema_hash` во всех фикстурах; легко забыть и сломать CI.
