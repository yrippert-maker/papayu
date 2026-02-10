# Product Limits — papa-yu

## Not designed for

- **Real-time / low-latency processing** — операция планирования и применения занимает секунды.
- **High-concurrency server workloads** — desktop-приложение, один активный контекст.
- **Untrusted plugin execution** — нет sandbox для произвольного кода.
- **Enterprise SSO / RBAC** — аутентификация и авторизация не в scope.

## Known constraints

- **LLM planner** — предполагает структурированный ввод и хорошо сформированные промпты.
- **File PATCH/EDIT** — опирается на детерминированный контекст; anchor/before/after должны точно соответствовать файлу.
- **Golden traces** — отражают только протоколы v1, v2, v3; при смене схемы нужен пересчёт `schema_hash`.

## Critical failures

Следующие события считаются **критическими отказами**:

| Событие | Impact | Условия |
|---------|--------|---------|
| **Corrupted workspace state** | Потеря или повреждение файлов проекта | Сбой во время apply, откат не сработал |
| **Silent data loss в EDIT_FILE** | Некорректная замена без явной ошибки | Неоднозначный anchor/before, ERR_EDIT_AMBIGUOUS не сработал |
| **Network access outside allowlist** | SSRF, утечка данных | Обход net::fetch_url_safe |
| **Secrets in trace** | Утечка ключей/токенов | Полные URL с query, логи с credentials |

## Supported vs unsupported

- **Supported:** анализ и правка локальных проектов, batch-режим, undo/redo, online research (Tavily), domain notes.
- **Unsupported:** работа с удалёнными репозиториями напрямую, выполнение произвольных скриптов, интеграция с внешними CI без адаптеров.
