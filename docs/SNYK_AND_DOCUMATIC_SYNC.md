# Синхронизация ИИ-агента с Snyk Code и Documatic

Интеграция с **Snyk Code** (анализ и дополнение кода) и **Documatic** (архитектура и структурирование) для передачи контекста в agent-sync и ИИ-агента.

---

## 1. Snyk Code

[Snyk Code](https://docs.snyk.io/scan-with-snyk/snyk-code) выполняет статический анализ кода на уязвимости и проблемы безопасности. Результаты подмешиваются в **agent-sync** и доступны агенту в Cursor / Claude Code.

### Включение

1. Получите API-токен в [Snyk](https://app.snyk.io/account): Account Settings → General → API Token (или создайте Service Account).
2. Узнайте **Organization ID** (в настройках организации или в URL: `app.snyk.io/org/<org_id>`).
3. Опционально: если в Snyk импортирован конкретный проект — скопируйте **Project ID** (в карточке проекта).
4. Задайте переменные окружения:

```bash
export PAPAYU_AGENT_SYNC=1
export PAPAYU_SNYK_SYNC=1
export PAPAYU_SNYK_TOKEN="ваш-токен"
# или
export SNYK_TOKEN="ваш-токен"

export PAPAYU_SNYK_ORG_ID="uuid-организации"
# опционально — только issues этого проекта
export PAPAYU_SNYK_PROJECT_ID="uuid-проекта"
```

### Поведение

- При каждом **анализе проекта** (кнопка «Анализировать» и т.п.) приложение при включённом `PAPAYU_SNYK_SYNC` запрашивает у Snyk REST API список **code**-issues по организации (и по проекту, если задан `PAPAYU_SNYK_PROJECT_ID`).
- Результаты записываются в **`.papa-yu/agent-sync.json`** в поле **`snyk_findings`** (массив: title, details, path). Агент в IDE может читать этот файл и учитывать замечания Snyk при предложениях.

### Ограничения

- Нужен проект, уже импортированный в Snyk (через UI или интеграцию с Git). Локальный анализ только по пути без импорта в Snyk через этот API не запускается.
- Используется REST API Snyk: `GET /rest/orgs/{org_id}/issues?type=code&...`. Версия API: `2024-04-02~experimental`.

---

## 2. Documatic (архитектура и структурирование)

[Documatic](https://www.documatic.com/) — поиск и документация по кодовой базе (расширение VS Code и веб-платформа). Публичного REST API для вызова из PAPA YU нет, поэтому интеграция — **через общий файл архитектуры**, который агент читает из agent-sync.

### Настройка

1. Экспортируйте или сохраните описание архитектуры/структуры проекта в файл в репозитории, например:
   - **`.papa-yu/architecture.md`** (по умолчанию),
   - или укажите свой путь через переменную **`PAPAYU_DOCUMATIC_ARCH_PATH`** (относительно корня проекта).

2. Содержимое можно:
   - сформировать вручную,
   - сгенерировать в Documatic (если есть экспорт) и скопировать в этот файл,
   - собрать из других инструментов (диаграммы, списки модулей и т.д.).

3. Переменные окружения:

```bash
export PAPAYU_AGENT_SYNC=1
# по умолчанию читается .papa-yu/architecture.md
# свой путь (относительно корня проекта):
# export PAPAYU_DOCUMATIC_ARCH_PATH="docs/architecture.md"
```

### Поведение

- При записи **agent-sync** приложение читает файл архитектуры (если он есть) и добавляет его содержимое в **`architecture_summary`** в **`.papa-yu/agent-sync.json`** (обрезается до 16 000 символов). ИИ-агент в Cursor / Claude Code может использовать это для анализа и структурирования архитектуры при предложениях.

---

## 3. Структура agent-sync.json

При включённых интеграциях файл **`.papa-yu/agent-sync.json`** может выглядеть так:

```json
{
  "path": "/path/to/project",
  "updated_at": "2026-02-09T12:00:00Z",
  "narrative": "Краткий вывод анализа PAPA YU...",
  "findings_count": 3,
  "actions_count": 5,
  "snyk_findings": [
    {
      "title": "SQL injection",
      "details": "[high] ...",
      "path": "src/api/users.rs"
    }
  ],
  "architecture_summary": "# Архитектура\n\nМодули: ..."
}
```

- **snyk_findings** — при `PAPAYU_SNYK_SYNC=1` и успешном ответе Snyk API.
- **architecture_summary** — при наличии файла архитектуры (по умолчанию `.papa-yu/architecture.md` или путь из `PAPAYU_DOCUMATIC_ARCH_PATH`).

---

## 4. Краткий чеклист

| Задача | Действие |
|--------|----------|
| Snyk Code | Задать `PAPAYU_AGENT_SYNC=1`, `PAPAYU_SNYK_SYNC=1`, `PAPAYU_SNYK_TOKEN`, `PAPAYU_SNYK_ORG_ID` (и при необходимости `PAPAYU_SNYK_PROJECT_ID`). Импортировать проект в Snyk. |
| Documatic / архитектура | Положить описание архитектуры в `.papa-yu/architecture.md` (или задать `PAPAYU_DOCUMATIC_ARCH_PATH`). Включить `PAPAYU_AGENT_SYNC=1`. |
| Агент в IDE | Настроить правило/скрипт: читать `.papa-yu/agent-sync.json` и учитывать `narrative`, `snyk_findings`, `architecture_summary` при предложениях. |

---

*См. также: `docs/CLAUDE_AND_AGENT_SYNC.md`, `env.openai.example`.*
