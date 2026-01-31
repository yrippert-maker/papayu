//! LLM-планировщик: генерация плана действий через OpenAI-совместимый API.
//!
//! Конфигурация через переменные окружения:
//! - `PAPAYU_LLM_API_URL` — URL API (например https://api.openai.com/v1/chat/completions или http://localhost:11434/v1/chat/completions для Ollama)
//! - `PAPAYU_LLM_API_KEY` — API-ключ (опционально для локальных API вроде Ollama)
//! - `PAPAYU_LLM_MODEL` — модель (по умолчанию gpt-4o-mini для OpenAI, для Ollama — например llama3.2)
//! - `PAPAYU_LLM_MODE` — режим: `chat` (инженер-коллега) или `fixit` (обязан вернуть патч + проверку); по умолчанию `chat`
//! - `PAPAYU_LLM_STRICT_JSON` — если `1`/`true`: добавляет `response_format: { type: "json_schema", ... }` (OpenAI Structured Outputs; Ollama может не поддерживать)
//! - `PAPAYU_LLM_TEMPERATURE` — температура генерации (по умолчанию 0 для детерминизма)
//! - `PAPAYU_LLM_MAX_TOKENS` — макс. токенов ответа (по умолчанию 65536)
//! - `PAPAYU_TRACE` — если `1`/`true`: пишет трассу в `.papa-yu/traces/<trace_id>.json`

use crate::context;
use crate::memory;
use crate::types::{Action, ActionKind, AgentPlan};
use jsonschema::JSONSchema;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

const SCHEMA_RAW: &str = include_str!("../../config/llm_response_schema.json");

pub(crate) fn schema_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_RAW.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Clone, serde::Serialize)]
struct ResponseFormatJsonSchema {
    #[serde(rename = "type")]
    ty: String,
    json_schema: ResponseFormatJsonSchemaInner,
}

#[derive(Clone, serde::Serialize)]
struct ResponseFormatJsonSchemaInner {
    name: String,
    schema: serde_json::Value,
    strict: bool,
}

#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormatJsonSchema>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
}

/// Пишет лог-ивент в stderr (формат: [trace_id] EVENT key=value ...).
fn log_llm_event(trace_id: &str, event: &str, pairs: &[(&str, String)]) {
    let line = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(" ");
    eprintln!("[{}] {} {}", trace_id, event, line);
}

const INPUT_CHARS_FOR_CAP: usize = 80_000;
const MAX_TOKENS_WHEN_LARGE_INPUT: u32 = 4096;

/// Маскирует секреты в строке (raw_content) при PAPAYU_TRACE_RAW=1.
fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    let mut pos = 0;
    // sk-... (OpenAI keys) — маскируем все вхождения
    while let Some(start) = out[pos..].find("sk-") {
        let abs_start = pos + start;
        let after = &out[abs_start + 3..];
        let rest_len = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').count();
        let end = abs_start + 3 + rest_len.min(50);
        if end <= out.len() {
            out.replace_range(abs_start..end, "__REDACTED_API_KEY__");
            pos = abs_start + 18; // len("__REDACTED_API_KEY__")
        } else {
            break;
        }
    }
    // Bearer token
    pos = 0;
    while let Some(start) = out[pos..].find("Bearer ") {
        let abs_start = pos + start;
        let after = &out[abs_start + 7..];
        let rest_len = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.').count();
        let end = abs_start + 7 + rest_len.min(60);
        if end <= out.len() {
            out.replace_range(abs_start..end, "__REDACTED_BEARER__");
            pos = abs_start + 17; // len("__REDACTED_BEARER__")
        } else {
            break;
        }
    }
    out
}

/// Сохраняет трассу в .papa-yu/traces/<trace_id>.json при PAPAYU_TRACE=1.
/// По умолчанию raw_content не сохраняется (риск секретов); PAPAYU_TRACE_RAW=1 — сохранять (с маскировкой).
fn write_trace(project_path: &str, trace_id: &str, trace: &mut serde_json::Value) {
    // Добавляем config_snapshot для воспроизводимости
    let config_snapshot = serde_json::json!({
        "schema_version": LLM_PLAN_SCHEMA_VERSION,
        "schema_hash": schema_hash(),
        "strict_json": std::env::var("PAPAYU_LLM_STRICT_JSON").unwrap_or_default(),
        "trace_raw": std::env::var("PAPAYU_TRACE_RAW").unwrap_or_default(),
        "normalize_eol": std::env::var("PAPAYU_NORMALIZE_EOL").unwrap_or_default(),
        "memory_autopatch": std::env::var("PAPAYU_MEMORY_AUTOPATCH").unwrap_or_default(),
        "max_tokens": std::env::var("PAPAYU_LLM_MAX_TOKENS").unwrap_or_default(),
        "temperature": std::env::var("PAPAYU_LLM_TEMPERATURE").unwrap_or_default(),
        "timeout_sec": std::env::var("PAPAYU_LLM_TIMEOUT_SEC").unwrap_or_default(),
    });
    if std::env::var("PAPAYU_TRACE")
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
    {
        let trace_raw = std::env::var("PAPAYU_TRACE_RAW")
            .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        if !trace_raw {
            if let Some(obj) = trace.as_object_mut() {
                if let Some(raw) = obj.remove("raw_content") {
                    if let Some(s) = raw.as_str() {
                        obj.insert("raw_content_redacted".into(), serde_json::Value::Bool(true));
                        let preview: String = s.chars().take(200).collect();
                        obj.insert("raw_content_preview".into(), serde_json::Value::String(format!("{}... ({} chars)", preview, s.len())));
                    }
                }
            }
        } else if let Some(obj) = trace.as_object_mut() {
            if let Some(raw) = obj.get("raw_content").and_then(|v| v.as_str()) {
                let redacted = redact_secrets(raw);
                obj.insert("raw_content".into(), serde_json::Value::String(redacted));
            }
        }

        if let Some(obj) = trace.as_object_mut() {
            obj.insert("config_snapshot".into(), config_snapshot);
        }

        if let Ok(root) = Path::new(project_path).canonicalize() {
            let trace_dir = root.join(".papa-yu").join("traces");
            let _ = fs::create_dir_all(&trace_dir);
            let trace_file = trace_dir.join(format!("{}.json", trace_id));
            let _ = fs::write(&trace_file, serde_json::to_string_pretty(trace).unwrap_or_default());
        }
    }
}

/// System prompt: режим Chat (инженер-коллега).
pub const CHAT_SYSTEM_PROMPT: &str = r#"Ты — мой инженерный ассистент внутри программы для создания, анализа и исправления кода.
Оператор один: я. Общайся как с коллегой-человеком: естественно, кратко, без канцелярщины и без самопрезентаций.

Главная цель: давать точные, проверяемые ответы по программированию и работе с проектом.
Стиль: "что вижу → что предлагаю → что сделать".

Ключевые правила:
- Не выдумывай факты о проекте. Если ты не читал файл/лог/результат — так и скажи.
- Никогда не утверждай, что ты что-то запускал/проверял, если не вызывал инструмент и не видел вывод.
- Если данных не хватает — задай максимум 2 уточняющих вопроса. Если можно двигаться без уточнений — двигайся.
- Если предлагаешь изменения — показывай конкретный patch/diff и объясняй 2–5 короткими пунктами "почему так".
- Всегда предлагай шаг проверки (тест/команда/репро).
- Если есть риск (удаление данных, миграции, security) — предупреждай и предлагай безопасный вариант.

Инструменты:
- Используй инструменты для чтения файлов, поиска, логов, тестов и применения патчей, когда это повышает точность.
- Сначала собирай факты (read/search/logs), потом делай выводы, потом патч."#;

/// System prompt: режим Fix-it (обязан вернуть патч + проверку).
pub const FIXIT_SYSTEM_PROMPT: &str = r#"Ты — режим Fix-it внутри моей IDE-программы.
Твоя задача: минимальным и безопасным изменением исправить проблему и дать проверяемые шаги.

Выход должен содержать:
1) Краткий диагноз (1–3 пункта)
2) Patch/diff (обязательно)
3) Команды проверки (обязательно)
4) Риски/побочки (если есть)

Правила:
- Не выдумывай содержимое файлов/логов — сначала прочитай их через инструменты.
- Не делай широкие рефакторы без запроса: исправляй минимально.
- Если не хватает данных, можно задать 1 вопрос; иначе действуй."#;

/// Формальная версия схемы ответа (для воспроизводимости и будущего v2).
pub const LLM_PLAN_SCHEMA_VERSION: u32 = 1;

/// System prompt: режим Fix-plan (один JSON, context_requests, план → подтверждение → применение).
/// Режим через user.output_format: "plan" = только план, "apply" = действия.
pub const FIX_PLAN_SYSTEM_PROMPT: &str = r#"Ты — инженерный ассистент внутри программы для создания, анализа и исправления кода. Оператор один: я.
Всегда отвечай ОДНИМ валидным JSON-объектом. Никакого текста вне JSON.

Режимы (смотри user.output_format в ENGINEERING_MEMORY):
- user.output_format == "plan" (Fix-plan): НЕ предлагай применять изменения. Верни actions пустым массивом [].
  Опиши диагноз и пошаговый план в summary. Если нужно больше данных — заполни context_requests.
- user.output_format == "apply" (Apply): Верни actions (или proposed_changes.actions) с конкретными изменениями файлов/директорий.
  summary: что изменено и как проверить (используй project.default_test_command если задан).
  Если изменений не требуется — верни actions: [] и summary, начинающийся с "NO_CHANGES:" (строго).

Если output_format не задан или "patch_first"/"plan_first" — верни actions как обычно (массив или объект с actions).

Правила:
- Не выдумывай содержимое файлов/логов. Если нужно — запроси через context_requests.
- Никогда не утверждай, что тесты/команды запускались, если их не запускало приложение.
- Если данных не хватает — задай максимум 2 вопроса в questions и/или добавь context_requests.
- Минимальные изменения. Без широких рефакторингов без явного запроса.

Схема JSON (всегда либо массив actions, либо объект):
- actions: массив { kind, path, content } — kind: CREATE_FILE|CREATE_DIR|UPDATE_FILE|DELETE_FILE|DELETE_DIR
- proposed_changes.actions: альтернативное место для actions
- summary: string (диагноз + план для plan, что сделано для apply)
- context_requests: [{ type: "read_file"|"search"|"logs"|"env", path?, start_line?, end_line?, query?, glob?, source?, last_n? }]
- memory_patch: object (только ключи из whitelist: user.*, project.*)"#;

/// Возвращает system prompt по режиму (PAPAYU_LLM_MODE: chat | fixit | fix-plan).
fn get_system_prompt_for_mode() -> &'static str {
    let mode = std::env::var("PAPAYU_LLM_MODE").unwrap_or_else(|_| "chat".into());
    match mode.trim().to_lowercase().as_str() {
        "fixit" | "fix-it" | "fix_it" => FIXIT_SYSTEM_PROMPT,
        "fix-plan" | "fix_plan" => FIX_PLAN_SYSTEM_PROMPT,
        _ => CHAT_SYSTEM_PROMPT,
    }
}

/// Проверяет, включён ли LLM-планировщик (задан URL).
pub fn is_llm_configured() -> bool {
    std::env::var("PAPAYU_LLM_API_URL")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Строит промпт для LLM: путь, полное содержимое проекта (все файлы), отчёт, цель, стиль дизайна и опционально контекст трендов.
/// ИИ настроен так, чтобы самостоятельно использовать дизайн и тренды при предложениях.
fn build_prompt(
    path: &str,
    report_json: &str,
    user_goal: &str,
    project_content: Option<&str>,
    design_style: Option<&str>,
    trends_context: Option<&str>,
) -> String {
    let content_block = project_content
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\n\nПолное содержимое файлов проекта (анализируй всё, не только три файла):\n{}\n", s))
        .unwrap_or_else(|| "\n\nПроект пуст или папка не найдена. Можешь создавать программу с нуля: полную структуру (package.json, src/, конфиги, исходники, README, .gitignore и т.д.).\n".to_string());

    let content_empty = project_content
        .map(|s| {
            s.trim().is_empty()
                || s.contains("пуста")
                || s.contains("не найдена")
                || s.contains("нет релевантных")
        })
        .unwrap_or(true);
    let create_from_scratch = content_empty
        || user_goal.to_lowercase().contains("с нуля")
        || user_goal.to_lowercase().contains("from scratch")
        || user_goal.to_lowercase().contains("создать проект");

    let extra = if create_from_scratch {
        "\nВажно: пользователь может просить создать проект с нуля. Предлагай полный набор файлов и папок (package.json, src/index.ts, README.md, .gitignore, конфиги и т.д.) в виде массива действий CREATE_DIR и CREATE_FILE."
    } else {
        ""
    };

    let design_block = design_style
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let lower = s.to_lowercase();
            let hint = if lower.contains("material") || lower.contains("материал") {
                "Применяй Material Design: компоненты и гайдлайны из material.io, структура и стили в духе Material UI (MUI)."
            } else if lower.contains("tailwind") || lower.contains("shadcn") || lower.contains("shadcn/ui") {
                "Применяй Tailwind CSS и/или shadcn/ui: утилитарные классы, компоненты из shadcn (радиусы, тени, типографика из сторонних ресурсов shadcn/ui)."
            } else if lower.contains("bootstrap") {
                "Применяй Bootstrap: сетка, компоненты, утилиты из Bootstrap (getbootstrap.com)."
            } else if lower.contains("сторонн") || lower.contains("third-party") || lower.contains("внешн") {
                "Используй дизайн из сторонних ресурсов: популярные UI-библиотеки, дизайн-системы (Material, Ant Design, Chakra, Radix и т.д.), подключай через npm/CDN и применяй в разметке и стилях."
            } else {
                "Применяй свой дизайн ИИ: современный, читаемый UI, консистентные отступы, типографика и цвета; при создании с нуля добавляй CSS/конфиг под выбранный стиль."
            };
            format!("\n\nСтиль дизайна: {}. {}", s.trim(), hint)
        })
        .unwrap_or_else(|| "\n\nДизайн: самостоятельно применяй современный консистентный дизайн при создании или изменении UI — свой (ИИ) или из известных систем (Material, Tailwind/shadcn, Bootstrap). Делай это по умолчанию, без явного выбора пользователем.".to_string());

    let trends_block = trends_context
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\n\nИспользуй самостоятельно актуальные рекомендации и тренды (учитывай при предложениях и улучшениях):\n{}\n", s))
        .unwrap_or_else(String::new);

    format!(
        r#"Контекст задачи и проекта (используй только эти данные; роль и правила заданы в system message).

ПОЛНОМОЧИЯ: read/write файлов (через план действий), формирование плана под git/PR. «Готово» = план содержит код/патчи + шаги проверки (lint, test). Реальное выполнение делает приложение по твоему плану.

ЕДИНЫЙ КОНТУР РАЗРАБОТКИ (обязательный порядок по возможности):
1. Прочитай ТЗ → сформируй PLAN.md (файлы, модули, шаги, критерии готовности).
2. Создай каркас проекта (scaffold).
3. Реализуй MVP.
4. Добавь тесты.
5. Обеспечь запуск: make lint, make test, make build (или эквивалент: npm run lint/test/build, pytest, mvn test и т.д.).
6. Если что-то падает — план должен предусматривать исправления до успеха; итог — CHANGELOG или отчёт.

РЕПОЗИТОРИЙ-ШАБЛОН: единая структура команд (агент всегда знает, что делать):
- make setup (или npm install / pip install / mvn install)
- make lint — ruff/eslint/spotless по стеку
- make test — pytest/vitest/junit по стеку
- make run — запуск приложения
- make build — сборка артефакта
При создании проекта добавляй Makefile (или явно npm/pnpm/mvn/gradle скрипты) с этими целями.

ПО СТЕКУ (минимум):
- Python: ruff check, pytest, black/ruff format; CLI Typer/Click, сайт FastAPI+uvicorn, десктоп PySide6.
- JS/TS: npm run lint (eslint), npm test (vitest/jest), npm run build, npm run dev; CLI commander/oclif, сайт React/Vue+Vite, десктоп Electron/Tauri.
- Java: mvn test / gradle test, spotless/checkstyle, mvn package; CLI Picocli, сайт Spring Boot, десктоп JavaFX.

БОЕВОЙ ШАБЛОН PYTHON + FASTAPI (САЙТ) — используй при создании сайта на FastAPI:
Структура: app/ (__init__.py, main.py), app/api/ (__init__.py, health.py), app/core/ (__init__.py, config.py), tests/ (__init__.py, test_health.py), .github/workflows/ci.yml, .gitignore, Makefile, README.md, pyproject.toml, ruff.toml.
Команды: make setup (pip install -e ".[dev]"), make lint (ruff check . + ruff format --check .), make test (pytest), make run (uvicorn app.main:app --reload --host 0.0.0.0 --port 8000), make build (python -c "import app.main; print('ok')").
Файлы: pyproject.toml — project name/version, requires-python >=3.11, dependencies fastapi>=0.110, uvicorn[standard]>=0.27, pydantic>=2.6; dev: pytest>=8.0, httpx>=0.27, ruff>=0.6; [tool.pytest.ini_options] testpaths=["tests"] addopts="-q". ruff.toml — line-length=100 target-version="py311" [lint] select=["E","F","I","B","UP"]. Makefile — цели setup, lint, test, run, build как выше. app/main.py — FastAPI(title, version), include_router(health_router). app/api/health.py — APIRouter(tags=["health"]), GET /health возвращает {{"status":"ok"}}. app/core/config.py — pydantic BaseModel Settings (app_name). tests/test_health.py — TestClient(app), GET /health, assert status_code 200, json == {{"status":"ok"}}. .github/workflows/ci.yml — checkout, setup-python 3.11, make setup, make lint, make test, make build. README.md — make setup, make run, ссылка на /health, make lint, make test.
Контракт для FastAPI: (1) Всегда начинай с PLAN.md (что меняешь, файлы, критерии готовности). (2) Любая фича = код + тест. (3) После каждого шага — make lint, make test; если упало — фикси до зелёного. (4) Итог: PR/патч + отчёт (что сделано, как запустить, как проверить). Интернет для FastAPI: только официальные доки FastAPI/Uvicorn/Pydantic, PyPI, официальные примеры; правило «нашёл решение → подтверди make test».

ШАБЛОН PLAN.md (в корень при планировании): заголовок «План работ»; секции: Контекст (репо, цель, ограничения), Требования DoD (чеклист: функциональность, тесты make test, линт make lint, README, CI зелёный), Архитектура/Дизайн (модули app/main, app/api, app/core, app/db при БД; решения: БД SQLite/SQLAlchemy 2.x, миграции Alembic, тесты pytest + отдельная тестовая БД), План изменений по шагам (1 scaffold БД/миграции 2 сущность 3 CRUD endpoints 4 тесты 5 README), Риски/Вопросы.

ПРИМЕР ФИЧИ CRUD (FastAPI + SQLite + Alembic + тесты): зависимости — sqlalchemy>=2.0, dev: alembic>=1.13. Слой БД: app/db/session.py (create_engine sqlite, SessionLocal), app/db/base.py (DeclarativeBase), app/db/deps.py (get_db yield Session), app/db/models.py (Item: id, name, description). Схемы: app/api/schemas.py (ItemCreate, ItemUpdate, ItemOut Pydantic). Роутер: app/api/items.py (prefix /items, POST/GET/GET list/PATCH/DELETE, Depends(get_db), 404 если не найден). main.py: include_router(items_router), Base.metadata.create_all(bind=engine) для dev. Тесты: tests/conftest.py (tempfile sqlite, override get_db, TestClient), tests/test_items.py (test_items_crud: create 201, get 200, list, patch, delete 204, get 404). Makefile: migrate = alembic upgrade head. README: секция DB migrations (alembic upgrade head). Alembic: alembic init, env.py — импорт Base и models, target_metadata=Base.metadata; alembic revision --autogenerate -m "create items", alembic upgrade head.

ПРОТОКОЛ «ДОБАВИТЬ ФИЧУ» (для агента): (1) Создай/обнови PLAN.md (DoD + список файлов). (2) Реализуй минимальный endpoint + тест. (3) Запусти make test → исправь до зелёного. (4) make lint. (5) Обнови README. (6) Итог: один PR, CI зелёный.

ИНТЕРНЕТ: используй только для официальной документации (docs.*, GitHub, PyPI, npm, Maven Central), проверки версий и примеров API. Любую найденную команду/конфиг — проверять запуском тестов/сборки.

КОНТРАКТ (жёсткие правила):
1. Всегда начинай с PLAN.md (архитектура, файлы, команды, критерии готовности).
2. Всегда добавляй/обновляй README.md (setup, run, test).
3. Любая фича = код + тест.
4. После изменений всегда предусматривай запуск make lint и make test (и make build если есть).
5. Если неясно — делай разумное допущение, фиксируй в PLAN.md и README.md.
6. Не добавляй зависимости без явной причины.
7. Итог: план действий ведёт к PR/патчу + краткому отчёту «что сделано / как проверить».

КРИТИЧЕСКИ ВАЖНО: При вводе пользователя выполняй команду в ПЕРВУЮ ОЧЕРЕДЬ (например: «помоги создать программу», «добавь README», «создай проект с нуля»). Формируй план действий (массив действий). НЕ предлагай сначала анализ — сразу план по запросу.
Форматы: (1) scaffold — структура, зависимости, базовые модули. (2) автокодер по ТЗ — фичи, тесты, документация. (3) репо/патчи — тесты, линтер, PR. (4) скрипты/автоматизации. Выбирай по формулировке пользователя.
Верни ТОЛЬКО валидный JSON: либо массив действий, либо объект {{ "actions": [...], "memory_patch": {{ "user.preferred_style": "brief", "project.default_test_command": "pytest -q" }} }} — memory_patch только если пользователь явно просит запомнить настройки (команды тестов, линтера, стиль и т.д.).
Формат каждого элемента actions: {{ "kind": "CREATE_FILE" | "CREATE_DIR" | "UPDATE_FILE" | "DELETE_FILE" | "DELETE_DIR", "path": "относительный/путь", "content": "опционально для CREATE_FILE/UPDATE_FILE" }}.
Создавай программы с нуля (PLAN.md, README.md, Makefile/скрипты, код, тесты) или изменяй существующие файлы. Учитывай всё содержимое файлов при анализе.
{}
{}
Путь проекта: {}
Цель пользователя: {}
{}
Отчёт анализа (JSON):
{}
{}
"#,
        design_block, trends_block, path, user_goal, content_block, report_json, extra
    )
}

const REPAIR_PROMPT: &str = r#"
Верни ТОЛЬКО валидный JSON строго по схеме. Никаких комментариев, пояснений и текста вне JSON.
НЕ добавляй никаких новых полей. Предпочти объект с actions (не массив).
Исправь предыдущий ответ — он не прошёл валидацию.
"#;

const REPAIR_PROMPT_PLAN_ACTIONS_MUST_BE_EMPTY: &str = r#"
В режиме PLAN actions обязан быть пустым массивом [].
Верни объект с "actions": [] и "summary" (диагноз + план шагов).
"#;

/// Компилирует JSON Schema для локальной валидации (один раз).
fn compiled_response_schema() -> Option<JSONSchema> {
    let schema: serde_json::Value = serde_json::from_str(include_str!("../../config/llm_response_schema.json")).ok()?;
    JSONSchema::options().compile(&schema).ok()
}

/// Локальная валидация ответа против схемы. Best-effort: если схема не компилируется — пропускаем.
fn validate_json_against_schema(value: &serde_json::Value) -> Result<(), String> {
    let Some(compiled) = compiled_response_schema() else {
        return Ok(()); // схема не загружена — не валидируем
    };
    compiled.validate(value).map_err(|errs| {
        let msgs: Vec<String> = errs.map(|e| e.to_string()).collect();
        format!("JSON schema validation failed: {}", msgs.join("; "))
    })
}

/// Извлекает JSON из ответа (убирает обёртку ```json ... ``` при наличии).
fn extract_json_from_content(content: &str) -> Result<&str, String> {
    let content = content.trim();
    if let Some(start) = content.find("```json") {
        let after = &content[start + 7..];
        let end = after
            .find("```")
            .map(|i| i)
            .unwrap_or(after.len());
        Ok(after[..end].trim())
    } else if let Some(start) = content.find("```") {
        let after = &content[start + 3..];
        let end = after
            .find("```")
            .map(|i| i)
            .unwrap_or(after.len());
        Ok(after[..end].trim())
    } else {
        Ok(content)
    }
}

/// Нормализует path и проверяет запрещённые сегменты.
fn validate_path(path: &str, idx: usize) -> Result<(), String> {
    if path.contains('\0') {
        return Err(format!("actions[{}].path invalid: contains NUL (ERR_INVALID_PATH)", idx));
    }
    if path.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(format!(
            "actions[{}].path invalid: contains control characters (ERR_INVALID_PATH)",
            idx
        ));
    }
    let normalized = path.replace('\\', "/");
    let trimmed = normalized.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Err(format!(
            "actions[{}].path invalid: path is empty or '.' (ERR_INVALID_PATH)",
            idx
        ));
    }
    if trimmed.starts_with('/') || trimmed.starts_with("//") {
        return Err(format!(
            "actions[{}].path invalid: absolute path not allowed ({}) (ERR_INVALID_PATH)",
            idx, path
        ));
    }
    if trimmed.len() >= 2 && trimmed.chars().nth(1) == Some(':') {
        return Err(format!(
            "actions[{}].path invalid: Windows drive letter not allowed ({}) (ERR_INVALID_PATH)",
            idx, path
        ));
    }
    if trimmed.starts_with('~') {
        return Err(format!(
            "actions[{}].path invalid: tilde not allowed ({}) (ERR_INVALID_PATH)",
            idx, path
        ));
    }
    for (seg_i, seg) in trimmed.split('/').enumerate() {
        if seg == ".." {
            return Err(format!(
                "actions[{}].path invalid: '..' segment not allowed ({}) (ERR_INVALID_PATH)",
                idx, path
            ));
        }
        if seg == "." && seg_i > 0 {
            return Err(format!(
                "actions[{}].path invalid: '.' as path segment not allowed ({}) (ERR_INVALID_PATH)",
                idx, path
            ));
        }
    }
    Ok(())
}

/// Проверяет конфликты действий на один path (CREATE+UPDATE, DELETE+UPDATE и т.д.).
fn validate_action_conflicts(actions: &[Action]) -> Result<(), String> {
    use std::collections::HashMap;
    let mut by_path: HashMap<String, Vec<ActionKind>> = HashMap::new();
    for a in actions {
        let path = a.path.replace('\\', "/").trim().to_string();
        by_path.entry(path).or_default().push(a.kind.clone());
    }
    for (path, kinds) in by_path {
        let has_create = kinds.contains(&ActionKind::CreateFile);
        let has_update = kinds.contains(&ActionKind::UpdateFile);
        let has_delete_file = kinds.contains(&ActionKind::DeleteFile);
        let has_delete_dir = kinds.contains(&ActionKind::DeleteDir);
        if has_create && has_update {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has both CREATE_FILE and UPDATE_FILE",
                path
            ));
        }
        if (has_delete_file || has_delete_dir) && (has_create || has_update) {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has conflicting DELETE and CREATE/UPDATE",
                path
            ));
        }
    }
    Ok(())
}

/// Извлекает пути файлов, прочитанных в plan (FILE[path]: или === path === в plan_context).
fn extract_files_read_from_plan_context(plan_context: &str) -> std::collections::HashSet<String> {
    let mut paths = std::collections::HashSet::new();
    let mut search = plan_context;
    // FILE[path]: — из fulfill_context_requests
    while let Some(start) = search.find("FILE[") {
        search = &search[start + 5..];
        if let Some(end) = search.find("]:") {
            let path = search[..end].trim().replace('\\', "/");
            if !path.is_empty() {
                paths.insert(path);
            }
            search = &search[end + 2..];
        } else {
            break;
        }
    }
    search = plan_context;
    // === path === — из project_content
    while let Some(start) = search.find("=== ") {
        search = &search[start + 4..];
        if let Some(end) = search.find(" ===") {
            let path = search[..end].trim().replace('\\', "/");
            if !path.is_empty() && !path.contains('\n') {
                paths.insert(path);
            }
            search = &search[end + 4..];
        } else {
            break;
        }
    }
    paths
}

/// APPLY-режим: каждый UPDATE_FILE должен ссылаться на файл, прочитанный в plan.
fn validate_update_without_base(
    actions: &[Action],
    plan_context: Option<&str>,
) -> Result<(), String> {
    let Some(ctx) = plan_context else { return Ok(()) };
    let read_paths = extract_files_read_from_plan_context(ctx);
    for (i, a) in actions.iter().enumerate() {
        if a.kind == ActionKind::UpdateFile {
            let path = a.path.replace('\\', "/").trim().to_string();
            if !read_paths.contains(&path) {
                return Err(format!(
                    "ERR_UPDATE_WITHOUT_BASE: UPDATE_FILE path '{}' not read in plan (actions[{}]). \
                    В PLAN-цикле должен быть context_requests.read_file для этого path.",
                    path, i
                ));
            }
        }
    }
    Ok(())
}

const MAX_PATH_LEN: usize = 240;
const MAX_ACTIONS: usize = 200;
const MAX_TOTAL_CONTENT_BYTES: usize = 5 * 1024 * 1024; // 5MB
const MAX_CONTENT_NON_PRINTABLE_RATIO: f32 = 0.1; // >10% non-printable = reject

/// Проверяет content на NUL и pseudo-binary.
fn validate_content(content: &str, idx: usize) -> Result<(), String> {
    if content.contains('\0') {
        return Err(format!(
            "actions[{}].content invalid: contains NUL (ERR_PSEUDO_BINARY)",
            idx
        ));
    }
    let len = content.chars().count();
    if len == 0 {
        return Ok(());
    }
    let non_printable = content
        .chars()
        .filter(|c| !c.is_ascii_graphic() && *c != '\n' && *c != '\r' && *c != '\t' && *c != ' ')
        .count();
    let ratio = non_printable as f32 / len as f32;
    if ratio > MAX_CONTENT_NON_PRINTABLE_RATIO {
        return Err(format!(
            "actions[{}].content invalid: >{}% non-printable (ERR_PSEUDO_BINARY)",
            idx,
            (MAX_CONTENT_NON_PRINTABLE_RATIO * 100.0) as u32
        ));
    }
    Ok(())
}

/// Валидирует actions: path, content, конфликты, лимиты.
fn validate_actions(actions: &[Action]) -> Result<(), String> {
    if actions.len() > MAX_ACTIONS {
        return Err(format!(
            "ERR_TOO_MANY_ACTIONS: {} > {} (max_actions)",
            actions.len(),
            MAX_ACTIONS
        ));
    }
    let mut total_bytes = 0usize;
    for (i, a) in actions.iter().enumerate() {
        validate_path(&a.path, i)?;
        if a.path.len() > MAX_PATH_LEN {
            return Err(format!(
                "actions[{}].path invalid: length {} > {} (ERR_PATH_TOO_LONG)",
                i, a.path.len(), MAX_PATH_LEN
            ));
        }
        match a.kind {
            ActionKind::CreateFile | ActionKind::UpdateFile => {
                let content = a.content.as_ref().map(|s| s.as_str()).unwrap_or("");
                if content.trim().is_empty() {
                    return Err(format!(
                        "actions[{}].content required for {} (ERR_CONTENT_REQUIRED)",
                        i,
                        match a.kind {
                            ActionKind::CreateFile => "CREATE_FILE",
                            ActionKind::UpdateFile => "UPDATE_FILE",
                            _ => unreachable!(),
                        }
                    ));
                }
                validate_content(content, i)?;
                total_bytes += content.len();
            }
            _ => {}
        }
    }
    if total_bytes > MAX_TOTAL_CONTENT_BYTES {
        return Err(format!(
            "ERR_CONTENT_TOO_LARGE: total {} bytes > {} (max_total_bytes)",
            total_bytes, MAX_TOTAL_CONTENT_BYTES
        ));
    }
    validate_action_conflicts(actions)?;
    Ok(())
}

/// Парсит массив действий из JSON; нормализует kind в допустимые значения.
fn parse_actions_from_json(json_str: &str) -> Result<Vec<Action>, String> {
    let raw: Vec<serde_json::Value> =
        serde_json::from_str(json_str).map_err(|e| format!("JSON: {}", e))?;
    let mut actions = Vec::new();
    for (i, v) in raw.iter().enumerate() {
        let obj = v
            .as_object()
            .ok_or_else(|| format!("action[{}] is not an object", i))?;
        let kind_str = obj
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("CREATE_FILE");
        let kind = match kind_str.to_uppercase().as_str() {
            "CREATE_FILE" => ActionKind::CreateFile,
            "CREATE_DIR" => ActionKind::CreateDir,
            "UPDATE_FILE" => ActionKind::UpdateFile,
            "DELETE_FILE" => ActionKind::DeleteFile,
            "DELETE_DIR" => ActionKind::DeleteDir,
            _ => ActionKind::CreateFile,
        };
        let path = obj
            .get("path")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("unknown_{}", i));
        let content = obj
            .get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        actions.push(Action { kind, path, content });
    }
    Ok(actions)
}

/// Результат парсинга ответа LLM: actions, memory_patch, summary (для Fix-plan), context_requests для следующего раунда.
struct PlanParseResult {
    actions: Vec<Action>,
    memory_patch: Option<HashMap<String, serde_json::Value>>,
    summary_override: Option<String>,
    context_requests: Option<Vec<serde_json::Value>>,
}

/// Парсит ответ LLM: массив действий, объект { actions, memory_patch } или Fix-plan { mode, summary, proposed_changes.actions, context_requests, ... }.
fn parse_plan_response(json_str: &str) -> Result<PlanParseResult, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON: {}", e))?;
    let (actions_value, memory_patch, summary_override, context_requests) = if value.is_array() {
        (value, None, None, None)
    } else if let Some(obj) = value.as_object() {
        let actions_value = obj
            .get("proposed_changes")
            .and_then(|pc| pc.get("actions").cloned())
            .or_else(|| obj.get("actions").cloned())
            .unwrap_or_else(|| serde_json::Value::Array(vec![]));
        let memory_patch = obj.get("memory_patch").and_then(|v| v.as_object()).map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<_, _>>()
        });
        let summary_override = obj.get("summary").and_then(|v| v.as_str()).map(String::from);
        let context_requests = obj.get("context_requests").and_then(|v| v.as_array()).map(|a| {
            a.iter().cloned().collect::<Vec<_>>()
        });
        (actions_value, memory_patch, summary_override, context_requests)
    } else {
        return Err("expected JSON array or object with 'actions'".into());
    };
    let actions_str = serde_json::to_string(&actions_value).map_err(|e| e.to_string())?;
    let actions = parse_actions_from_json(&actions_str)?;
    Ok(PlanParseResult {
        actions,
        memory_patch,
        summary_override,
        context_requests,
    })
}

const MAX_CONTEXT_ROUNDS: u32 = 2;

/// Вызывает LLM API и возвращает план (AgentPlan).
/// Автосбор контекста: env + project prefs в начало user message; при context_requests — до MAX_CONTEXT_ROUNDS раундов.
/// output_format_override: "plan" | "apply" — для двухфазного Plan→Apply.
/// last_plan_for_apply, last_context_for_apply: при переходе из Plan в Apply (user сказал "ok").
const DEFAULT_MAX_TOKENS: u32 = 16384;

pub async fn plan(
    user_prefs_path: &Path,
    project_prefs_path: &Path,
    path: &str,
    report_json: &str,
    user_goal: &str,
    project_content: Option<&str>,
    design_style: Option<&str>,
    trends_context: Option<&str>,
    output_format_override: Option<&str>,
    last_plan_for_apply: Option<&str>,
    last_context_for_apply: Option<&str>,
) -> Result<AgentPlan, String> {
    let trace_id = Uuid::new_v4().to_string();

    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL not set")?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL is empty".into());
    }

    let model = std::env::var("PAPAYU_LLM_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();

    let mem = memory::load_memory(user_prefs_path, project_prefs_path);
    let mut memory_block = memory::build_memory_block(&mem);
    // Переопределение режима для Plan→Apply
    if let Some(of) = output_format_override {
        if of == "plan" || of == "apply" {
            memory_block.push_str(&format!("\n\nРЕЖИМ_ДЛЯ_ЭТОГО_ЗАПРОСА: {} (соблюдай строго)", of));
        }
    }
    let system_prompt = get_system_prompt_for_mode();
    let system_content = format!("{}{}\n\nLLM_PLAN_SCHEMA_VERSION={}", system_prompt, memory_block, LLM_PLAN_SCHEMA_VERSION);

    let project_root = Path::new(path);
    let base_context = context::gather_base_context(project_root, &mem);
    let prompt_body = build_prompt(path, report_json, user_goal, project_content, design_style, trends_context);
    // Эвристики автосбора: Traceback, ImportError и т.д.
    let auto_from_message = context::gather_auto_context_from_message(
        project_root,
        &format!("{}\n{}", user_goal, report_json),
    );
    let mut user_message = format!("{}{}{}", base_context, prompt_body, auto_from_message);

    // Переход Plan→Apply: инжектируем сохранённый план и контекст
    if output_format_override == Some("apply") {
        if let Some(plan_json) = last_plan_for_apply {
            let mut apply_prompt = String::from("\n\n--- РЕЖИМ APPLY ---\nПользователь подтвердил план. Применяй изменения согласно плану ниже. Верни actions с конкретными правками файлов.\n\nПЛАН:\n");
            apply_prompt.push_str(plan_json);
            if let Some(ctx) = last_context_for_apply {
                apply_prompt.push_str("\n\nСОБРАННЫЙ_КОНТЕКСТ:\n");
                apply_prompt.push_str(ctx);
            }
            user_message.push_str(&apply_prompt);
        }
    }

    let timeout_sec = std::env::var("PAPAYU_LLM_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(90);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_sec))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let mut round = 0u32;

    let use_strict_json = std::env::var("PAPAYU_LLM_STRICT_JSON")
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let temperature = std::env::var("PAPAYU_LLM_TEMPERATURE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(0.0);

    let input_chars = system_content.len() + user_message.len();
    let configured_max_tokens = std::env::var("PAPAYU_LLM_MAX_TOKENS")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_TOKENS);
    let max_tokens = if input_chars > INPUT_CHARS_FOR_CAP {
        configured_max_tokens.min(MAX_TOKENS_WHEN_LARGE_INPUT)
    } else {
        configured_max_tokens
    };

    let provider = api_url
        .split('/')
        .nth(2)
        .unwrap_or("unknown")
        .split(':')
        .next()
        .unwrap_or("unknown");

    let response_format = if use_strict_json {
        let schema_json: serde_json::Value = serde_json::from_str(include_str!("../../config/llm_response_schema.json"))
            .unwrap_or_else(|_| serde_json::json!({}));
        Some(ResponseFormatJsonSchema {
            ty: "json_schema".to_string(),
            json_schema: ResponseFormatJsonSchemaInner {
                name: "papa_yu_response".to_string(),
                schema: schema_json,
                strict: true,
            },
        })
    } else {
        None
    };

    let mut repair_done = false;
    let mut skip_response_format = false; // capability detection: fallback при ошибке response_format
    let mut context_cache = context::ContextCache::new();

    let (last_actions, last_summary_override, last_plan_json, last_context_for_return) = loop {
        let effective_response_format = if skip_response_format {
            None
        } else {
            response_format.clone()
        };

        let body = ChatRequest {
            model: model.trim().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_content.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message.clone(),
                },
            ],
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            top_p: Some(1.0),
            presence_penalty: Some(0.0),
            frequency_penalty: Some(0.0),
            response_format: effective_response_format,
        };

        log_llm_event(
            &trace_id,
            "LLM_REQUEST_SENT",
            &[
                ("model", model.trim().to_string()),
                ("schema_version", LLM_PLAN_SCHEMA_VERSION.to_string()),
                ("strict_json", (!skip_response_format && use_strict_json).to_string()),
                ("provider", provider.to_string()),
                ("token_budget", max_tokens.to_string()),
                ("input_chars", input_chars.to_string()),
            ],
        );

        let mut req = client.post(api_url).json(&body);
        if let Some(key) = &api_key {
            if !key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key.trim()));
            }
        }

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                log_llm_event(&trace_id, "LLM_REQUEST_TIMEOUT", &[("timeout_sec", timeout_sec.to_string())]);
            }
            format!("Request: {}", e)
        })?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("Response body: {}", e))?;

        if !status.is_success() {
            // Capability detection: если strict_json и ошибка — возможно response_format не поддерживается
            if use_strict_json && !skip_response_format {
                let lower = text.to_lowercase();
                if lower.contains("response_format")
                    || lower.contains("json_schema")
                    || lower.contains("unknown field")
                    || lower.contains("not supported")
                {
                    skip_response_format = true;
                    log_llm_event(
                        &trace_id,
                        "LLM_RESPONSE_FORMAT_FALLBACK",
                        &[
                            ("reason", "provider_error".to_string()),
                            ("status", status.as_str().to_string()),
                        ],
                    );
                    continue;
                }
            }
            return Err(format!("API error {}: {}", status, text));
        }

        log_llm_event(
            &trace_id,
            if repair_done { "LLM_RESPONSE_REPAIR_RETRY" } else { "LLM_RESPONSE_OK" },
            &[("round", round.to_string())],
        );

        let chat: ChatResponse =
            serde_json::from_str(&text).map_err(|e| format!("Response JSON: {}", e))?;
        let content = chat
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| "No choices in API response".to_string())?;

        // Парсинг JSON: best-effort (извлечь из markdown при наличии)
        let json_str = match extract_json_from_content(content) {
            Ok(s) => s,
            Err(e) if !repair_done => {
                log_llm_event(&trace_id, "VALIDATION_FAILED", &[("code", "ERR_JSON_EXTRACT".to_string()), ("reason", e.clone())]);
                user_message.push_str(&format!(
                    "\n\n---\n{REPAIR_PROMPT}\n\nRaw output:\n{content}"
                ));
                repair_done = true;
                continue;
            }
            Err(e) => {
                let mut trace_val = serde_json::json!({ "trace_id": trace_id, "raw_content": content, "error": e, "event": "VALIDATION_FAILED" });
                write_trace(path, &trace_id, &mut trace_val);
                return Err(e);
            }
        };

        // Десериализация в Value
        let value: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) if !repair_done => {
                user_message.push_str(&format!(
                    "\n\n---\nERR_JSON_PARSE: {}\n\n{REPAIR_PROMPT}\n\nRaw output:\n{content}",
                    e
                ));
                repair_done = true;
                continue;
            }
            Err(e) => return Err(format!("JSON parse: {}", e)),
        };

        // Локальная валидация схемы (best-effort при strict выкл; обязательна при strict вкл)
        if let Err(e) = validate_json_against_schema(&value) {
            log_llm_event(&trace_id, "VALIDATION_FAILED", &[("code", "ERR_SCHEMA_VALIDATION".to_string()), ("reason", e.clone())]);
            if !repair_done {
                user_message.push_str(&format!(
                    "\n\n---\nERR_SCHEMA_VALIDATION: {}\n\n{REPAIR_PROMPT}\n\nRaw output:\n{content}",
                    e
                ));
                repair_done = true;
                continue;
            }
            let mut trace_val = serde_json::json!({ "trace_id": trace_id, "raw_content": content, "validated_json": json_str, "error": e, "event": "VALIDATION_FAILED" });
            write_trace(path, &trace_id, &mut trace_val);
            return Err(e);
        }

        let parsed = parse_plan_response(json_str)?;

        // Жёсткая валидация режимов: PLAN → actions=[], APPLY → actions непустой (если нужны изменения)
        let mode: &str = output_format_override.unwrap_or_else(|| {
            let s = mem.user.output_format.trim();
            if s.is_empty() { "" } else { mem.user.output_format.as_str() }
        });
        if mode == "plan" && !parsed.actions.is_empty() {
            if !repair_done {
                user_message.push_str(&format!(
                    "\n\n---\n{REPAIR_PROMPT_PLAN_ACTIONS_MUST_BE_EMPTY}\n\nRaw output:\n{content}"
                ));
                repair_done = true;
                continue;
            }
            return Err("В режиме PLAN actions обязан быть []".to_string());
        }
        if mode == "apply" && parsed.actions.is_empty() {
            let summary = parsed.summary_override.as_deref().unwrap_or("");
            let no_changes = summary.trim().starts_with("NO_CHANGES:");
            if !no_changes && !repair_done {
                user_message.push_str(&format!(
                    "\n\n---\nERR_APPLY_EMPTY_ACTIONS: В режиме APPLY при пустом actions summary обязан начинаться с \"NO_CHANGES:\". Raw output:\n{content}"
                ));
                repair_done = true;
                continue;
            }
            if !no_changes {
                return Err("В режиме APPLY при пустом actions summary обязан начинаться с NO_CHANGES:".to_string());
            }
        }

        // PAPAYU_MEMORY_AUTOPATCH=1 — применять memory_patch; иначе игнорировать (только по явному согласию)
        let autopatch = std::env::var("PAPAYU_MEMORY_AUTOPATCH")
            .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        if autopatch {
            if let Some(patch) = &parsed.memory_patch {
                let (new_user, new_project) =
                    memory::apply_memory_patch(patch, &mem.user, &mem.project);
                let _ = memory::save_user_prefs(user_prefs_path, &new_user);
                let _ = memory::save_project_prefs(project_prefs_path, &new_project);
            }
        }

        let context_requests = parsed.context_requests.as_deref().unwrap_or(&[]);
        if !context_requests.is_empty() && round < MAX_CONTEXT_ROUNDS {
            let fulfilled = context::fulfill_context_requests(
                project_root,
                context_requests,
                200,
                Some(&mut context_cache),
                Some(&trace_id),
            );
            user_message.push_str(&fulfilled);
            round += 1;
            continue;
        }

        break (parsed.actions, parsed.summary_override, json_str.to_string(), user_message.clone());
    };

    // Строгая валидация: path, content, конфликты, UPDATE_WITHOUT_BASE
    if let Err(e) = validate_actions(&last_actions) {
        log_llm_event(&trace_id, "VALIDATION_FAILED", &[("code", "ERR_ACTIONS".to_string()), ("reason", e.clone())]);
        let mut trace_val = serde_json::json!({ "trace_id": trace_id, "validated_json": last_plan_json, "error": e, "event": "VALIDATION_FAILED" });
        write_trace(path, &trace_id, &mut trace_val);
        return Err(e);
    }
    let mode_for_update_base = output_format_override
        .filter(|s| !s.is_empty())
        .or_else(|| if mem.user.output_format.trim().is_empty() { None } else { Some(mem.user.output_format.as_str()) });
    if mode_for_update_base == Some("apply") {
        if let Err(e) = validate_update_without_base(&last_actions, last_context_for_apply) {
            log_llm_event(&trace_id, "VALIDATION_FAILED", &[("code", "ERR_UPDATE_WITHOUT_BASE".to_string()), ("reason", e.clone())]);
            let mut trace_val = serde_json::json!({ "trace_id": trace_id, "validated_json": last_plan_json, "error": e, "event": "VALIDATION_FAILED" });
            write_trace(path, &trace_id, &mut trace_val);
            return Err(e);
        }
    }

    let mode_for_plan_json = output_format_override
        .filter(|s| !s.is_empty())
        .or_else(|| if mem.user.output_format.is_empty() { None } else { Some(mem.user.output_format.as_str()) });
    let is_plan_mode = mode_for_plan_json == Some("plan");
    let plan_json = is_plan_mode.then_some(last_plan_json.clone());
    let plan_context = is_plan_mode.then_some(last_context_for_return.clone());

    let mut trace_val = serde_json::json!({
        "trace_id": trace_id,
        "event": "LLM_PLAN_OK",
        "schema_version": LLM_PLAN_SCHEMA_VERSION,
        "model": model.trim(),
        "provider": provider,
        "actions_count": last_actions.len(),
        "validated_json": last_plan_json,
    });
    write_trace(path, &trace_id, &mut trace_val);

    Ok(AgentPlan {
        ok: true,
        summary: last_summary_override
            .unwrap_or_else(|| format!("План от LLM: {} действий.", last_actions.len())),
        actions: last_actions,
        error: None,
        error_code: None,
        plan_json,
        plan_context,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        extract_files_read_from_plan_context, parse_actions_from_json, schema_hash, validate_actions,
        validate_update_without_base, FIX_PLAN_SYSTEM_PROMPT, LLM_PLAN_SCHEMA_VERSION,
    };
    use crate::types::{Action, ActionKind};

    #[test]
    fn test_schema_version_is_one() {
        assert_eq!(LLM_PLAN_SCHEMA_VERSION, 1);
    }

    #[test]
    fn test_schema_hash_non_empty() {
        let h = schema_hash();
        assert!(!h.is_empty());
        assert_eq!(h.len(), 64); // sha256 hex
    }

    #[test]
    fn test_system_prompt_contains_schema_version() {
        let system_content = format!(
            "{}\n\nLLM_PLAN_SCHEMA_VERSION={}",
            FIX_PLAN_SYSTEM_PROMPT, LLM_PLAN_SCHEMA_VERSION
        );
        assert!(system_content.contains("LLM_PLAN_SCHEMA_VERSION=1"));
    }

    #[test]
    fn test_validate_actions_empty() {
        assert!(validate_actions(&[]).is_ok());
    }

    #[test]
    fn test_validate_actions_valid_create_file() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "README.md".to_string(),
            content: Some("# Project".to_string()),
        }];
        assert!(validate_actions(&actions).is_ok());
    }

    #[test]
    fn test_validate_actions_rejects_parent_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "../etc/passwd".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_absolute_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "/etc/passwd".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_path_ending_with_dotdot() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "a/..".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_windows_drive() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "C:/foo/bar".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_unc_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "//server/share/file".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_dot_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateDir,
            path: ".".to_string(),
            content: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_dot_segment() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "a/./b".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_allows_relative_prefix() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "./src/main.rs".to_string(),
            content: Some("fn main() {}".to_string()),
        }];
        assert!(validate_actions(&actions).is_ok());
    }

    #[test]
    fn test_validate_actions_rejects_conflict_create_update() {
        let actions = vec![
            Action {
                kind: ActionKind::CreateFile,
                path: "foo.txt".to_string(),
                content: Some("a".to_string()),
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("b".to_string()),
            },
        ];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_conflict_delete_update() {
        let actions = vec![
            Action {
                kind: ActionKind::DeleteFile,
                path: "foo.txt".to_string(),
                content: None,
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("b".to_string()),
            },
        ];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_extract_files_from_plan_context() {
        let ctx = "FILE[src/main.rs]:\nfn main() {}\n\n=== README.md ===\n# Project\n";
        let paths = extract_files_read_from_plan_context(ctx);
        assert!(paths.contains("src/main.rs"));
        assert!(paths.contains("README.md"));
    }

    #[test]
    fn test_validate_update_without_base_ok() {
        let ctx = "FILE[foo.txt]:\nold\n\n=== bar.txt ===\ncontent\n";
        let actions = vec![
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("new".to_string()),
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "bar.txt".to_string(),
                content: Some("updated".to_string()),
            },
        ];
        assert!(validate_update_without_base(&actions, Some(ctx)).is_ok());
    }

    #[test]
    fn test_validate_update_without_base_err() {
        let ctx = "FILE[foo.txt]:\nold\n";
        let actions = vec![Action {
            kind: ActionKind::UpdateFile,
            path: "unknown.txt".to_string(),
            content: Some("new".to_string()),
        }];
        assert!(validate_update_without_base(&actions, Some(ctx)).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_tilde_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "~/etc/passwd".to_string(),
            content: Some("x".to_string()),
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_requires_content_for_create_file() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "README.md".to_string(),
            content: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_parse_actions_from_json_array() {
        let json = r#"[{"kind":"CREATE_FILE","path":"a.txt","content":"x"}]"#;
        let actions = parse_actions_from_json(json).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].path, "a.txt");
    }

    #[test]
    fn test_parse_actions_from_json_object() {
        let json = r#"{"actions":[{"kind":"CREATE_DIR","path":"src"}]}"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let actions_value = raw.get("actions").cloned().unwrap();
        let actions_str = serde_json::to_string(&actions_value).unwrap();
        let actions = parse_actions_from_json(&actions_str).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].path, "src");
    }
}
