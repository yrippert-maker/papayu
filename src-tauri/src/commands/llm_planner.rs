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
const SCHEMA_V2_RAW: &str = include_str!("../../config/llm_response_schema_v2.json");
const SCHEMA_V3_RAW: &str = include_str!("../../config/llm_response_schema_v3.json");

fn protocol_version(override_version: Option<u32>) -> u32 {
    crate::protocol::protocol_version(override_version)
}

pub(crate) fn schema_hash() -> String {
    schema_hash_for_version(protocol_version(None))
}

pub(crate) fn schema_hash_for_version(version: u32) -> String {
    let raw = if version == 3 {
        SCHEMA_V3_RAW
    } else if version == 2 {
        SCHEMA_V2_RAW
    } else {
        SCHEMA_RAW
    };
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn current_schema_version() -> u32 {
    protocol_version(None)
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
        let rest_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .count();
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
        let rest_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .count();
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
        "schema_version": current_schema_version(),
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
                        obj.insert(
                            "raw_content_preview".into(),
                            serde_json::Value::String(format!(
                                "{}... ({} chars)",
                                preview,
                                s.len()
                            )),
                        );
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
            let _ = fs::write(
                &trace_file,
                serde_json::to_string_pretty(trace).unwrap_or_default(),
            );
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

/// Формальная версия схемы v1 (для тестов и совместимости).
#[allow(dead_code)]
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

/// System prompt v2: Protocol v2 (PATCH_FILE, base_sha256, object-only).
pub const FIX_PLAN_SYSTEM_PROMPT_V2: &str = r#"Ты — инженерный ассистент внутри программы, работающей по Protocol v2.

Формат ответа:
- Всегда возвращай ТОЛЬКО валидный JSON, строго по JSON Schema v2.
- Корневой объект, поле "actions" обязательно.
- Никаких комментариев, пояснений или текста вне JSON.

Правила изменений файлов:
- UPDATE_FILE запрещён для существующих файлов — используй PATCH_FILE.
- Для изменения существующего файла ИСПОЛЬЗУЙ ТОЛЬКО PATCH_FILE.
- PATCH_FILE ОБЯЗАН содержать:
  - base_sha256 — точный sha256 текущей версии файла (из контекста)
  - patch — unified diff
- Если base_sha256 не совпадает или контекста недостаточно — верни PLAN и запроси context_requests.

Режимы:
- PLAN: actions ДОЛЖЕН быть пустым массивом [], summary обязателен.
- APPLY: если изменений нет — actions=[], summary НАЧИНАЕТСЯ с "NO_CHANGES:"; иначе actions непустой.

Контекст:
- Для каждого файла предоставляется его sha256 в формате FILE[path] (sha256=...).
- base_sha256 бери из строки FILE[path] (sha256=...) в контексте.

PATCH_FILE правила:
- Патч должен быть минимальным: меняй только нужные строки.
- Каждый @@ hunk должен иметь 1–3 строки контекста до/после изменения.
- Не делай массовых форматирований и EOL-изменений.

Когда нельзя PATCH_FILE:
- Если файл не UTF-8 или слишком большой/генерируемый — верни PLAN (actions=[]) и запроси альтернативу.

Запреты:
- Не добавляй новых полей. Не изменяй защищённые пути. Не придумывай base_sha256."#;

/// System prompt v3: Protocol v3 (EDIT_FILE по умолчанию, PATCH_FILE fallback).
pub const FIX_PLAN_SYSTEM_PROMPT_V3: &str = r#"Ты — инженерный ассистент внутри программы, работающей по Protocol v3.

Формат ответа:
- Всегда возвращай ТОЛЬКО валидный JSON, строго по JSON Schema v3.
- Корневой объект, поле "actions" обязательно.
- Никаких комментариев, пояснений или текста вне JSON.

Правила изменений файлов:
- Для существующих файлов используй EDIT_FILE, а не PATCH_FILE.
- base_sha256 бери из FILE[path] (sha256=...) в контексте.
- Правки минимальные: меняй только нужные строки, без форматирования файла.
- anchor должен быть устойчивым и уникальным (фрагмент кода/строки).
- before — точный фрагмент, который уже есть в файле рядом с anchor; after — заменяющий фрагмент.

EDIT_FILE:
- kind: EDIT_FILE, path, base_sha256 (64 hex), edits: [{ op: "replace", anchor, before, after, occurrence?, context_lines? }].
- anchor: строка для поиска в файле (уникальная или с occurrence).
- before/after: точное совпадение и замена в окне вокруг anchor.

Режимы:
- PLAN: actions ДОЛЖЕН быть пустым массивом [], summary обязателен.
- APPLY: если изменений нет — actions=[], summary НАЧИНАЕТСЯ с "NO_CHANGES:"; иначе actions непустой.

Запреты:
- Не добавляй новых полей. Не изменяй защищённые пути. Не придумывай base_sha256."#;

/// Возвращает system prompt по режиму и protocol_version.
fn get_system_prompt_for_mode() -> &'static str {
    let mode = std::env::var("PAPAYU_LLM_MODE").unwrap_or_else(|_| "chat".into());
    let ver = protocol_version(None);
    let use_v3 = ver == 3;
    let use_v2 = ver == 2;
    match mode.trim().to_lowercase().as_str() {
        "fixit" | "fix-it" | "fix_it" => {
            if use_v3 {
                FIX_PLAN_SYSTEM_PROMPT_V3
            } else if use_v2 {
                FIX_PLAN_SYSTEM_PROMPT_V2
            } else {
                FIXIT_SYSTEM_PROMPT
            }
        }
        "fix-plan" | "fix_plan" => {
            if use_v3 {
                FIX_PLAN_SYSTEM_PROMPT_V3
            } else if use_v2 {
                FIX_PLAN_SYSTEM_PROMPT_V2
            } else {
                FIX_PLAN_SYSTEM_PROMPT
            }
        }
        _ => CHAT_SYSTEM_PROMPT,
    }
}

/// Проверяет, нужен ли fallback на v1 для APPLY (при активном v2).
pub fn is_protocol_fallback_applicable(apply_error_code: &str, repair_attempt: u32) -> bool {
    crate::protocol::protocol_version(None) == 2
        && crate::protocol::protocol_fallback_enabled()
        && crate::protocol::should_fallback_to_v1(apply_error_code, repair_attempt)
}

/// Проверяет, нужен ли fallback v3→v2 для APPLY.
pub fn is_protocol_fallback_v3_to_v2_applicable(
    apply_error_code: &str,
    repair_attempt: u32,
) -> bool {
    crate::protocol::protocol_version(None) == 3
        && crate::protocol::should_fallback_to_v2(apply_error_code, repair_attempt)
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

/// v2 repair hints для PATCH_FILE (для repair flow / UI)
#[allow(dead_code)]
const REPAIR_ERR_PATCH_NOT_UNIFIED: &str =
    "ERR_PATCH_NOT_UNIFIED: patch должен быть unified diff (---/+++ и @@ hunks)";
#[allow(dead_code)]
const REPAIR_ERR_BASE_MISMATCH: &str =
    "ERR_BASE_MISMATCH: файл изменился, верни PLAN и запроси read_file заново";
#[allow(dead_code)]
const REPAIR_ERR_PATCH_APPLY_FAILED: &str = "ERR_PATCH_APPLY_FAILED: патч не применяется, верни PLAN и запроси больше контекста вокруг изменения";
#[allow(dead_code)]
const REPAIR_ERR_V2_UPDATE_EXISTING_FORBIDDEN: &str = "ERR_V2_UPDATE_EXISTING_FORBIDDEN: сгенерируй PATCH_FILE вместо UPDATE_FILE для существующего файла";

/// Шаблон для repair с подстановкой path и sha256 (ERR_BASE_SHA256_NOT_FROM_CONTEXT).
fn repair_err_base_sha256_not_from_context(path: &str, sha256: &str) -> String {
    format!(
        r#"ERR_BASE_SHA256_NOT_FROM_CONTEXT:
Для PATCH_FILE по пути "{}" base_sha256 должен быть ровно sha256 из контекста.
Используй это значение base_sha256: {}

Верни ТОЛЬКО валидный JSON по схеме v2.
Для изменения файла используй PATCH_FILE с base_sha256={} и unified diff в поле patch.
НЕ добавляй новых полей."#,
        path, sha256, sha256
    )
}

/// v3: repair для EDIT_FILE (ERR_EDIT_BASE_MISMATCH) — инжект sha из контекста.
fn repair_err_edit_base_mismatch(path: &str, sha256: &str) -> String {
    format!(
        r#"ERR_EDIT_BASE_SHA256_NOT_FROM_CONTEXT:
Для EDIT_FILE по пути "{}" base_sha256 должен быть ровно sha256 из контекста.
Используй это значение base_sha256: {}

Верни ТОЛЬКО валидный JSON по схеме v3.
Для изменения файла используй EDIT_FILE с base_sha256={} и edits (anchor/before/after).
НЕ добавляй новых полей."#,
        path, sha256, sha256
    )
}

/// Строит repair prompt с конкретным sha256 из контекста (v2 PATCH_FILE или v3 EDIT_FILE).
/// Возвращает Some((prompt, paths)), если нашли sha для действия с неверным base_sha256.
pub fn build_v2_patch_repair_prompt_with_sha(
    last_plan_context: &str,
    validated_json: &serde_json::Value,
) -> Option<(String, Vec<String>)> {
    use crate::context;
    use crate::patch;

    let ver = protocol_version(None);
    let actions = validated_json
        .get("proposed_changes")
        .and_then(|pc| pc.get("actions"))
        .or_else(|| validated_json.get("actions"))
        .and_then(|a| a.as_array())?;
    let sha_map = context::extract_file_sha256_from_context(last_plan_context);
    for a in actions {
        let obj = a.as_object()?;
        let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        let path = obj.get("path").and_then(|p| p.as_str())?;
        let base = obj.get("base_sha256").and_then(|b| b.as_str());
        let sha_ctx = sha_map.get(path)?;
        let needs_repair = match base {
            None => true,
            Some(b) if !patch::is_valid_sha256_hex(b) => true,
            Some(b) if b != sha_ctx.as_str() => true,
            _ => false,
        };
        if !needs_repair {
            continue;
        }
        if ver == 3 && kind.to_uppercase() == "EDIT_FILE" {
            let prompt = repair_err_edit_base_mismatch(path, sha_ctx);
            return Some((prompt, vec![path.to_string()]));
        }
        if ver == 2 && kind.to_uppercase() == "PATCH_FILE" {
            let prompt = repair_err_base_sha256_not_from_context(path, sha_ctx);
            return Some((prompt, vec![path.to_string()]));
        }
    }
    None
}

/// Компилирует JSON Schema для локальной валидации (v1 или v2 по protocol_version).
fn compiled_response_schema() -> Option<JSONSchema> {
    let raw = if protocol_version(None) == 2 {
        SCHEMA_V2_RAW
    } else {
        SCHEMA_RAW
    };
    let schema: serde_json::Value = serde_json::from_str(raw).ok()?;
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

/// Валидация против схемы конкретной версии (для golden traces).
#[allow(dead_code)]
fn compiled_schema_for_version(version: u32) -> Option<JSONSchema> {
    let raw = if version == 3 {
        SCHEMA_V3_RAW
    } else if version == 2 {
        SCHEMA_V2_RAW
    } else {
        SCHEMA_RAW
    };
    let schema: serde_json::Value = serde_json::from_str(raw).ok()?;
    JSONSchema::options().compile(&schema).ok()
}

/// Извлекает JSON из ответа (убирает обёртку ```json ... ``` при наличии).
fn extract_json_from_content(content: &str) -> Result<&str, String> {
    let content = content.trim();
    if let Some(start) = content.find("```json") {
        let after = &content[start + 7..];
        let end = after.find("```").map(|i| i).unwrap_or(after.len());
        Ok(after[..end].trim())
    } else if let Some(start) = content.find("```") {
        let after = &content[start + 3..];
        let end = after.find("```").map(|i| i).unwrap_or(after.len());
        Ok(after[..end].trim())
    } else {
        Ok(content)
    }
}

/// Нормализует path и проверяет запрещённые сегменты.
fn validate_path(path: &str, idx: usize) -> Result<(), String> {
    if path.contains('\0') {
        return Err(format!(
            "actions[{}].path invalid: contains NUL (ERR_INVALID_PATH)",
            idx
        ));
    }
    if path
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
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

/// Проверяет конфликты действий на один path (CREATE+UPDATE, PATCH+UPDATE, DELETE+UPDATE и т.д.).
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
        let has_patch = kinds.contains(&ActionKind::PatchFile);
        let has_edit = kinds.contains(&ActionKind::EditFile);
        let has_delete_file = kinds.contains(&ActionKind::DeleteFile);
        let has_delete_dir = kinds.contains(&ActionKind::DeleteDir);
        if has_create && has_update {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has both CREATE_FILE and UPDATE_FILE",
                path
            ));
        }
        // PATCH_FILE / EDIT_FILE конфликтуют с CREATE/UPDATE/DELETE на тот же path
        if (has_patch || has_edit) && (has_create || has_update) {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has PATCH_FILE/EDIT_FILE and CREATE/UPDATE",
                path
            ));
        }
        if (has_patch || has_edit) && (has_delete_file || has_delete_dir) {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has PATCH_FILE/EDIT_FILE and DELETE",
                path
            ));
        }
        if has_edit && has_patch {
            return Err(format!(
                "ERR_ACTION_CONFLICT: path '{}' has both EDIT_FILE and PATCH_FILE",
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
    // FILE[path]: или FILE[path] (sha256=...): — из fulfill_context_requests
    while let Some(start) = search.find("FILE[") {
        search = &search[start + 5..];
        if let Some(end) = search.find(']') {
            let path = search[..end].trim().replace('\\', "/");
            if !path.is_empty() {
                paths.insert(path);
            }
            search = &search[end + 1..];
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

/// v2: UPDATE_FILE запрещён для существующих файлов — используй PATCH_FILE.
fn validate_v2_update_existing_forbidden(
    project_root: &std::path::Path,
    actions: &[Action],
) -> Result<(), String> {
    if protocol_version(None) != 2 {
        return Ok(());
    }
    for (i, a) in actions.iter().enumerate() {
        if a.kind != ActionKind::UpdateFile {
            continue;
        }
        let p = match crate::tx::safe_join(project_root, &a.path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if p.is_file() {
            return Err(format!(
                "ERR_V2_UPDATE_EXISTING_FORBIDDEN: UPDATE_FILE path '{}' существует (actions[{}]). \
                В v2 используй PATCH_FILE для существующих файлов. Сгенерируй PATCH_FILE.",
                a.path, i
            ));
        }
    }
    Ok(())
}

/// APPLY-режим: UPDATE_FILE и PATCH_FILE должны ссылаться на файл, прочитанный в plan.
fn validate_update_without_base(
    actions: &[Action],
    plan_context: Option<&str>,
) -> Result<(), String> {
    let Some(ctx) = plan_context else {
        return Ok(());
    };
    let read_paths = extract_files_read_from_plan_context(ctx);
    for (i, a) in actions.iter().enumerate() {
        if a.kind == ActionKind::UpdateFile || a.kind == ActionKind::PatchFile {
            let path = a.path.replace('\\', "/").trim().to_string();
            if !read_paths.contains(&path) {
                let kind_str = if a.kind == ActionKind::PatchFile {
                    "PATCH_FILE"
                } else {
                    "UPDATE_FILE"
                };
                return Err(format!(
                    "ERR_UPDATE_WITHOUT_BASE: {} path '{}' not read in plan (actions[{}]). \
                    В PLAN-цикле должен быть context_requests.read_file для этого path.",
                    kind_str, path, i
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
                i,
                a.path.len(),
                MAX_PATH_LEN
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
            ActionKind::PatchFile => {
                let patch = a.patch.as_deref().unwrap_or("");
                let base = a.base_sha256.as_deref().unwrap_or("");
                if patch.trim().is_empty() {
                    return Err(format!(
                        "actions[{}].patch required for PATCH_FILE (ERR_PATCH_REQUIRED)",
                        i
                    ));
                }
                if !crate::patch::looks_like_unified_diff(patch) {
                    return Err(format!(
                        "actions[{}].patch is not unified diff (ERR_PATCH_NOT_UNIFIED)",
                        i
                    ));
                }
                if !crate::patch::is_valid_sha256_hex(base) {
                    return Err(format!(
                        "actions[{}].base_sha256 invalid (64 hex chars) (ERR_BASE_SHA256_INVALID)",
                        i
                    ));
                }
                total_bytes += a.patch.as_ref().map(|p| p.len()).unwrap_or(0);
            }
            ActionKind::EditFile => {
                const MAX_EDITS_PER_ACTION: usize = 50;
                const MAX_EDIT_BEFORE_AFTER_BYTES: usize = 200_000;
                let base = a.base_sha256.as_deref().unwrap_or("");
                let edits = a.edits.as_deref().unwrap_or(&[]);
                if !crate::patch::is_valid_sha256_hex(base) {
                    return Err(format!(
                        "actions[{}].base_sha256 invalid (64 hex chars) (ERR_BASE_SHA256_INVALID)",
                        i
                    ));
                }
                if edits.is_empty() {
                    return Err(format!(
                        "actions[{}].edits required and non-empty for EDIT_FILE (ERR_EDIT_APPLY_FAILED)",
                        i
                    ));
                }
                if edits.len() > MAX_EDITS_PER_ACTION {
                    return Err(format!(
                        "actions[{}].edits count {} > {} (ERR_EDIT_APPLY_FAILED)",
                        i,
                        edits.len(),
                        MAX_EDITS_PER_ACTION
                    ));
                }
                let mut edit_bytes = 0usize;
                for (j, e) in edits.iter().enumerate() {
                    if e.anchor.is_empty() || e.before.is_empty() {
                        return Err(format!(
                            "actions[{}].edits[{}].anchor and before required (after may be empty for delete) (ERR_EDIT_APPLY_FAILED)",
                            i, j
                        ));
                    }
                    if e.anchor.contains('\0') || e.before.contains('\0') || e.after.contains('\0')
                    {
                        return Err(format!(
                            "actions[{}].edits[{}] must not contain NUL (ERR_EDIT_APPLY_FAILED)",
                            i, j
                        ));
                    }
                    if e.occurrence < 1 {
                        return Err(format!(
                            "actions[{}].edits[{}].occurrence >= 1 (ERR_EDIT_APPLY_FAILED)",
                            i, j
                        ));
                    }
                    if e.context_lines > 3 {
                        return Err(format!(
                            "actions[{}].edits[{}].context_lines 0..=3 (ERR_EDIT_APPLY_FAILED)",
                            i, j
                        ));
                    }
                    edit_bytes += e.before.len() + e.after.len();
                }
                if edit_bytes > MAX_EDIT_BEFORE_AFTER_BYTES {
                    return Err(format!(
                        "actions[{}].edits total before+after {} > {} (ERR_EDIT_APPLY_FAILED)",
                        i, edit_bytes, MAX_EDIT_BEFORE_AFTER_BYTES
                    ));
                }
                total_bytes += edit_bytes;
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
            "PATCH_FILE" => ActionKind::PatchFile,
            "EDIT_FILE" => ActionKind::EditFile,
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
        let patch = obj
            .get("patch")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let base_sha256 = obj
            .get("base_sha256")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string());
        let edits: Option<Vec<crate::types::EditOp>> =
            obj.get("edits").and_then(|arr| arr.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let o = v.as_object()?;
                        Some(crate::types::EditOp {
                            op: o
                                .get("op")
                                .and_then(|x| x.as_str())
                                .unwrap_or("replace")
                                .to_string(),
                            anchor: o
                                .get("anchor")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            before: o
                                .get("before")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            after: o
                                .get("after")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            occurrence: o.get("occurrence").and_then(|x| x.as_u64()).unwrap_or(1)
                                as u32,
                            context_lines: o
                                .get("context_lines")
                                .and_then(|x| x.as_u64())
                                .unwrap_or(2) as u32,
                        })
                    })
                    .collect()
            });
        actions.push(Action {
            kind,
            path,
            content,
            patch,
            base_sha256,
            edits,
        });
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
        let memory_patch = obj
            .get("memory_patch")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<_, _>>()
            });
        let summary_override = obj
            .get("summary")
            .and_then(|v| v.as_str())
            .map(String::from);
        let context_requests = obj
            .get("context_requests")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().cloned().collect::<Vec<_>>());
        (
            actions_value,
            memory_patch,
            summary_override,
            context_requests,
        )
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

/// Один запрос к LLM без repair/retry. Для мульти-провайдера: сбор планов от нескольких ИИ.
pub async fn request_one_plan(
    api_url: &str,
    api_key: Option<&str>,
    model: &str,
    system_content: &str,
    user_message: &str,
    _path: &str,
) -> Result<AgentPlan, String> {
    let timeout_sec = std::env::var("PAPAYU_LLM_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(90);
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
    let schema_version = current_schema_version();
    let response_format = if use_strict_json {
        let raw = if schema_version == 3 {
            SCHEMA_V3_RAW
        } else if schema_version == 2 {
            SCHEMA_V2_RAW
        } else {
            SCHEMA_RAW
        };
        let schema_json: serde_json::Value =
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}));
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_sec))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let body = ChatRequest {
        model: model.trim().to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_content.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            },
        ],
        temperature: Some(temperature),
        max_tokens: Some(max_tokens),
        top_p: Some(1.0),
        presence_penalty: Some(0.0),
        frequency_penalty: Some(0.0),
        response_format,
    };
    let mut req = client.post(api_url).json(&body);
    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key.trim()));
        }
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Response body: {}", e))?;
    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }
    let chat: ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("Response JSON: {}", e))?;
    let content = chat
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.content.as_deref())
        .ok_or_else(|| "No choices in API response".to_string())?;
    let json_str = extract_json_from_content(content).map_err(|e| format!("ERR_JSON_EXTRACT: {}", e))?;
    let json_owned = json_str.to_string();
    let value: serde_json::Value =
        serde_json::from_str(&json_owned).map_err(|e| format!("ERR_JSON_PARSE: {}", e))?;
    validate_json_against_schema(&value).map_err(|e| format!("ERR_SCHEMA_VALIDATION: {}", e))?;
    let parsed = parse_plan_response(&json_owned)?;
    let summary = parsed
        .summary_override
        .unwrap_or_else(|| format!("План: {} действий.", parsed.actions.len()));
    Ok(AgentPlan {
        ok: true,
        summary,
        actions: parsed.actions,
        error: None,
        error_code: None,
        plan_json: Some(json_owned),
        plan_context: None,
        protocol_version_used: Some(schema_version),
        online_fallback_suggested: None,
        online_context_used: Some(false),
    })
}

/// Вызывает LLM API и возвращает план (AgentPlan).
/// Автосбор контекста: env + project prefs в начало user message; при context_requests — до MAX_CONTEXT_ROUNDS раундов.
/// output_format_override: "plan" | "apply" — для двухфазного Plan→Apply.
/// last_plan_for_apply, last_context_for_apply: при переходе из Plan в Apply (user сказал "ok").
/// apply_error_for_repair: (error_code, validated_json) при ретрае после ERR_BASE_MISMATCH/ERR_BASE_SHA256_INVALID.
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
    apply_error_for_repair: Option<(&str, &str)>,
    force_protocol_version: Option<u32>,
    apply_error_stage: Option<&str>,
    apply_repair_attempt: Option<u32>,
    online_context_md: Option<&str>,
    online_context_sources: Option<&[String]>,
    online_fallback_executed: Option<bool>,
    online_fallback_reason: Option<&str>,
) -> Result<AgentPlan, String> {
    let trace_id = Uuid::new_v4().to_string();
    let effective_protocol = force_protocol_version
        .filter(|v| *v == 1 || *v == 2 || *v == 3)
        .unwrap_or_else(|| crate::protocol::protocol_version(None));

    let _guard = crate::protocol::set_protocol_version(effective_protocol);
    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL not set")?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL is empty".into());
    }

    let model = std::env::var("PAPAYU_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();

    let mem = memory::load_memory(user_prefs_path, project_prefs_path);
    let mut memory_block = memory::build_memory_block(&mem);
    // Переопределение режима для Plan→Apply
    if let Some(of) = output_format_override {
        if of == "plan" || of == "apply" {
            memory_block.push_str(&format!(
                "\n\nРЕЖИМ_ДЛЯ_ЭТОГО_ЗАПРОСА: {} (соблюдай строго)",
                of
            ));
        }
    }
    let system_prompt = get_system_prompt_for_mode();
    let system_content = format!(
        "{}{}\n\nLLM_PLAN_SCHEMA_VERSION={}",
        system_prompt,
        memory_block,
        current_schema_version()
    );

    let project_root = Path::new(path);
    let base_context = context::gather_base_context(project_root, &mem);
    let prompt_body = build_prompt(
        path,
        report_json,
        user_goal,
        project_content,
        design_style,
        trends_context,
    );
    // Эвристики автосбора: Traceback, ImportError и т.д.
    let auto_from_message = context::gather_auto_context_from_message(
        project_root,
        &format!("{}\n{}", user_goal, report_json),
    );
    let rest_context = format!("{}{}{}", base_context, prompt_body, auto_from_message);
    let mut online_block_result: Option<crate::online_research::OnlineBlockResult> = None;
    let mut online_context_dropped = false;
    let mut notes_injected = false;
    let mut notes_count = 0usize;
    let mut notes_chars = 0usize;
    let mut notes_ids: Vec<String> = vec![];
    let mut user_message = rest_context.clone();
    if let Some((notes_block, ids, chars)) =
        crate::domain_notes::get_notes_block_for_prompt(project_root, user_goal)
    {
        user_message = format!("{}{}", notes_block, user_message);
        notes_injected = true;
        notes_count = ids.len();
        notes_chars = chars;
        notes_ids = ids;
    }
    if let Some(md) = online_context_md {
        if !md.trim().is_empty() {
            let max_chars = crate::online_research::online_context_max_chars();
            let max_sources = crate::online_research::online_context_max_sources();
            let rest_chars = rest_context.chars().count();
            let max_total = context::context_max_total_chars();
            let priority0_reserved = 4096usize;
            let effective_max = crate::online_research::effective_online_max_chars(
                rest_chars,
                max_total,
                priority0_reserved,
            );
            let effective_max = if effective_max > 0 {
                effective_max.min(max_chars)
            } else {
                0
            };
            let sources: Vec<String> = online_context_sources
                .map(|s| s.to_vec())
                .unwrap_or_default();
            if effective_max >= 512 {
                let result = crate::online_research::build_online_context_block(
                    md,
                    &sources,
                    effective_max,
                    max_sources,
                );
                if !result.dropped {
                    user_message = format!("{}{}", result.block, rest_context);
                    online_block_result = Some(result);
                } else {
                    online_context_dropped = true;
                }
            } else {
                online_context_dropped = true;
            }
        }
    }
    let mut repair_injected_paths: Vec<String> = Vec::new();

    // Переход Plan→Apply: инжектируем сохранённый план и контекст
    if output_format_override == Some("apply") {
        if let Some(plan_json) = last_plan_for_apply {
            let mut apply_prompt = String::new();
            // Repair после ERR_BASE_MISMATCH/ERR_BASE_SHA256_INVALID: подставляем sha256 из контекста
            if let Some((code, validated_json_str)) = apply_error_for_repair {
                let is_base_error = code == "ERR_BASE_MISMATCH"
                    || code == "ERR_BASE_SHA256_INVALID"
                    || code == "ERR_EDIT_BASE_MISMATCH";
                if is_base_error {
                    if let Some(ctx) = last_context_for_apply {
                        if let Ok(val) =
                            serde_json::from_str::<serde_json::Value>(validated_json_str)
                        {
                            if let Some((repair, paths)) =
                                build_v2_patch_repair_prompt_with_sha(ctx, &val)
                            {
                                repair_injected_paths = paths;
                                apply_prompt.push_str(
                                    "\n\n--- REPAIR (ERR_BASE_SHA256_NOT_FROM_CONTEXT) ---\n",
                                );
                                apply_prompt.push_str(&repair);
                                apply_prompt.push_str("\n\nRaw output предыдущего ответа:\n");
                                apply_prompt.push_str(validated_json_str);
                                apply_prompt.push_str("\n\n");
                            }
                        }
                    }
                }
                // Repair-first для ERR_PATCH_APPLY_FAILED, ERR_V2_UPDATE_EXISTING_FORBIDDEN, v3 EDIT_FILE
                if force_protocol_version != Some(1)
                    && (code == "ERR_PATCH_APPLY_FAILED"
                        || code == "ERR_V2_UPDATE_EXISTING_FORBIDDEN"
                        || code == "ERR_EDIT_ANCHOR_NOT_FOUND"
                        || code == "ERR_EDIT_BEFORE_NOT_FOUND"
                        || code == "ERR_EDIT_AMBIGUOUS")
                {
                    if code == "ERR_PATCH_APPLY_FAILED" {
                        apply_prompt.push_str("\n\n--- REPAIR (ERR_PATCH_APPLY_FAILED) ---\n");
                        apply_prompt.push_str("Увеличь контекст hunks до 3 строк, не меняй соседние блоки. Верни PATCH_FILE с исправленным patch.\n\n");
                    } else if code == "ERR_V2_UPDATE_EXISTING_FORBIDDEN" {
                        apply_prompt
                            .push_str("\n\n--- REPAIR (ERR_V2_UPDATE_EXISTING_FORBIDDEN) ---\n");
                        apply_prompt.push_str("Сгенерируй PATCH_FILE вместо UPDATE_FILE для существующих файлов. Используй base_sha256 из контекста.\n\n");
                    } else if code == "ERR_EDIT_ANCHOR_NOT_FOUND" {
                        apply_prompt.push_str("\n\n--- REPAIR (ERR_EDIT_ANCHOR_NOT_FOUND) ---\n");
                        apply_prompt.push_str("anchor не найден в файле. Выбери anchor как точную подстроку из FILE[...] в контексте (например def foo(, class X:, уникальная строка). Проверь регистр и пробелы.\n\n");
                    } else if code == "ERR_EDIT_BEFORE_NOT_FOUND" {
                        apply_prompt.push_str("\n\n--- REPAIR (ERR_EDIT_BEFORE_NOT_FOUND) ---\n");
                        apply_prompt.push_str("before должен быть точным фрагментом рядом с anchor. Скопируй before из FILE[...] в контексте без изменений.\n\n");
                    } else if code == "ERR_EDIT_AMBIGUOUS" {
                        apply_prompt.push_str("\n\n--- REPAIR (ERR_EDIT_AMBIGUOUS) ---\n");
                        apply_prompt.push_str("Сделай anchor более уникальным или сузь before; если нужно — укажи occurrence (номер вхождения).\n\n");
                    }
                    apply_prompt.push_str("Raw output предыдущего ответа:\n");
                    apply_prompt.push_str(validated_json_str);
                    apply_prompt.push_str("\n\n");
                }
            }
            apply_prompt.push_str("\n\n--- РЕЖИМ APPLY ---\nПользователь подтвердил план. Применяй изменения согласно плану ниже. Верни actions с конкретными правками файлов.\n\nПЛАН:\n");
            apply_prompt.push_str(plan_json);
            if let Some(ctx) = last_context_for_apply {
                apply_prompt.push_str("\n\nСОБРАННЫЙ_КОНТЕКСТ:\n");
                apply_prompt.push_str(ctx);
            }
            user_message.push_str(&apply_prompt);
        }
    }

    // Мульти-провайдер: сбор планов от нескольких ИИ и агрегация в один оптимальный
    if let Ok(providers) = crate::commands::multi_provider::parse_providers_from_env() {
        if !providers.is_empty() {
            return crate::commands::multi_provider::fetch_and_aggregate(
                &system_content,
                &user_message,
                path,
            )
            .await;
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

    let schema_version = current_schema_version();
    let response_format = if use_strict_json {
        let raw = if schema_version == 3 {
            SCHEMA_V3_RAW
        } else if schema_version == 2 {
            SCHEMA_V2_RAW
        } else {
            SCHEMA_RAW
        };
        let schema_json: serde_json::Value =
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}));
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
    let mut last_context_stats: Option<context::ContextStats> = None;

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
                ("schema_version", schema_version.to_string()),
                (
                    "strict_json",
                    (!skip_response_format && use_strict_json).to_string(),
                ),
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

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                let timeout = e.is_timeout();
                if timeout {
                    log_llm_event(
                        &trace_id,
                        "LLM_REQUEST_TIMEOUT",
                        &[("timeout_sec", timeout_sec.to_string())],
                    );
                }
                return Err(format!(
                    "{}: Request: {}",
                    if timeout {
                        "LLM_REQUEST_TIMEOUT"
                    } else {
                        "LLM_REQUEST"
                    },
                    e
                ));
            }
        };
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Response body: {}", e))?;

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
            if repair_done {
                "LLM_RESPONSE_REPAIR_RETRY"
            } else {
                "LLM_RESPONSE_OK"
            },
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
                log_llm_event(
                    &trace_id,
                    "VALIDATION_FAILED",
                    &[
                        ("code", "ERR_JSON_EXTRACT".to_string()),
                        ("reason", e.clone()),
                    ],
                );
                user_message.push_str(&format!(
                    "\n\n---\n{REPAIR_PROMPT}\n\nRaw output:\n{content}"
                ));
                repair_done = true;
                continue;
            }
            Err(e) => {
                let mut trace_val = serde_json::json!({ "trace_id": trace_id, "raw_content": content, "error": e, "event": "VALIDATION_FAILED" });
                write_trace(path, &trace_id, &mut trace_val);
                return Err(format!("ERR_JSON_EXTRACT: {}", e));
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
            Err(e) => return Err(format!("ERR_JSON_PARSE: JSON parse: {}", e)),
        };

        // Локальная валидация схемы (best-effort при strict выкл; обязательна при strict вкл)
        if let Err(e) = validate_json_against_schema(&value) {
            log_llm_event(
                &trace_id,
                "VALIDATION_FAILED",
                &[
                    ("code", "ERR_SCHEMA_VALIDATION".to_string()),
                    ("reason", e.clone()),
                ],
            );
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
            return Err(format!("ERR_SCHEMA_VALIDATION: {}", e));
        }

        let parsed = parse_plan_response(json_str)?;

        // Жёсткая валидация режимов: PLAN → actions=[], APPLY → actions непустой (если нужны изменения)
        let mode: &str = output_format_override.unwrap_or_else(|| {
            let s = mem.user.output_format.trim();
            if s.is_empty() {
                ""
            } else {
                mem.user.output_format.as_str()
            }
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
                return Err(
                    "В режиме APPLY при пустом actions summary обязан начинаться с NO_CHANGES:"
                        .to_string(),
                );
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
            last_context_stats = Some(fulfilled.context_stats);
            user_message.push_str(&fulfilled.content);
            round += 1;
            continue;
        }

        break (
            parsed.actions,
            parsed.summary_override,
            json_str.to_string(),
            user_message.clone(),
        );
    };

    // Строгая валидация: path, content, конфликты, UPDATE_WITHOUT_BASE, v2 UPDATE_EXISTING_FORBIDDEN
    if let Err(e) = validate_actions(&last_actions) {
        log_llm_event(
            &trace_id,
            "VALIDATION_FAILED",
            &[("code", "ERR_ACTIONS".to_string()), ("reason", e.clone())],
        );
        let mut trace_val = serde_json::json!({ "trace_id": trace_id, "validated_json": last_plan_json, "error": e, "event": "VALIDATION_FAILED" });
        write_trace(path, &trace_id, &mut trace_val);
        return Err(e);
    }
    let mode_for_update_base = output_format_override
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if mem.user.output_format.trim().is_empty() {
                None
            } else {
                Some(mem.user.output_format.as_str())
            }
        });
    if mode_for_update_base == Some("apply") {
        if let Err(e) = validate_update_without_base(&last_actions, last_context_for_apply) {
            log_llm_event(
                &trace_id,
                "VALIDATION_FAILED",
                &[
                    ("code", "ERR_UPDATE_WITHOUT_BASE".to_string()),
                    ("reason", e.clone()),
                ],
            );
            let mut trace_val = serde_json::json!({ "trace_id": trace_id, "validated_json": last_plan_json, "error": e, "event": "VALIDATION_FAILED" });
            write_trace(path, &trace_id, &mut trace_val);
            return Err(e);
        }
        if let Err(e) = validate_v2_update_existing_forbidden(project_root, &last_actions) {
            log_llm_event(
                &trace_id,
                "VALIDATION_FAILED",
                &[
                    ("code", "ERR_V2_UPDATE_EXISTING_FORBIDDEN".to_string()),
                    ("reason", e.clone()),
                ],
            );
            let mut trace_val = serde_json::json!({ "trace_id": trace_id, "validated_json": last_plan_json, "error": e, "event": "VALIDATION_FAILED" });
            write_trace(path, &trace_id, &mut trace_val);
            return Err(e);
        }
    }

    let mode_for_plan_json = output_format_override
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if mem.user.output_format.is_empty() {
                None
            } else {
                Some(mem.user.output_format.as_str())
            }
        });
    let is_plan_mode = mode_for_plan_json == Some("plan");
    let plan_json = is_plan_mode.then_some(last_plan_json.clone());
    let plan_context = is_plan_mode.then_some(last_context_for_return.clone());

    let mut trace_val = serde_json::json!({
        "trace_id": trace_id,
        "event": "LLM_PLAN_OK",
        "schema_version": current_schema_version(),
        "model": model.trim(),
        "provider": provider,
        "actions_count": last_actions.len(),
        "validated_json": last_plan_json,
        "protocol_default": crate::protocol::protocol_default(),
    });
    if let Some((_, _)) = apply_error_for_repair {
        trace_val["protocol_repair_attempt"] = serde_json::json!(apply_repair_attempt.unwrap_or(0));
    }
    if force_protocol_version == Some(1) {
        trace_val["protocol_attempts"] = serde_json::json!(["v2", "v1"]);
        trace_val["protocol_fallback_reason"] = serde_json::json!(apply_error_for_repair
            .as_ref()
            .map(|(c, _)| *c)
            .unwrap_or("unknown"));
        trace_val["protocol_fallback_attempted"] = serde_json::json!(true);
        trace_val["protocol_fallback_stage"] =
            serde_json::json!(apply_error_stage.unwrap_or("apply"));
    }
    if !repair_injected_paths.is_empty() {
        trace_val["repair_injected_sha256"] = serde_json::json!(true);
        trace_val["repair_injected_paths"] = serde_json::json!(repair_injected_paths);
    }
    if online_fallback_executed == Some(true) {
        trace_val["online_fallback_executed"] = serde_json::json!(true);
        if let Some(reason) = online_fallback_reason {
            trace_val["online_fallback_reason"] = serde_json::json!(reason);
        }
    }
    if let Some(ref r) = online_block_result {
        trace_val["online_context_injected"] = serde_json::json!(true);
        trace_val["online_context_chars"] = serde_json::json!(r.chars_used);
        trace_val["online_context_sources_count"] = serde_json::json!(r.sources_count);
        if r.was_truncated {
            trace_val["online_context_truncated"] = serde_json::json!(true);
        }
    }
    // S3: store origin+pathname only (no query/fragment) for trace privacy
    if let Some(sources) = online_context_sources {
        let stripped: Vec<String> = sources
            .iter()
            .map(|u| crate::online_research::url_for_trace(u))
            .collect();
        trace_val["online_sources"] = serde_json::json!(stripped);
    }
    if online_context_dropped {
        trace_val["online_context_dropped"] = serde_json::json!(true);
    }
    if notes_injected {
        trace_val["notes_injected"] = serde_json::json!(true);
        trace_val["notes_count"] = serde_json::json!(notes_count);
        trace_val["notes_chars"] = serde_json::json!(notes_chars);
        trace_val["notes_ids"] = serde_json::json!(notes_ids);
    }
    if let Some(ref cs) = last_context_stats {
        trace_val["context_stats"] = serde_json::json!({
            "context_files_count": cs.context_files_count,
            "context_files_dropped_count": cs.context_files_dropped_count,
            "context_total_chars": cs.context_total_chars,
            "context_logs_chars": cs.context_logs_chars,
            "context_truncated_files_count": cs.context_truncated_files_count,
        });
    }
    let cache_stats = context_cache.stats();
    trace_val["cache_stats"] = serde_json::json!({
        "env_hits": cache_stats.env_hits,
        "env_misses": cache_stats.env_misses,
        "logs_hits": cache_stats.logs_hits,
        "logs_misses": cache_stats.logs_misses,
        "read_hits": cache_stats.read_hits,
        "read_misses": cache_stats.read_misses,
        "search_hits": cache_stats.search_hits,
        "search_misses": cache_stats.search_misses,
        "hit_rate": cache_stats.hit_rate(),
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
        protocol_version_used: Some(effective_protocol),
        online_fallback_suggested: None,
        online_context_used: Some(online_block_result.is_some()),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_v2_patch_repair_prompt_with_sha, compiled_schema_for_version,
        extract_files_read_from_plan_context, is_protocol_fallback_applicable,
        parse_actions_from_json, schema_hash, schema_hash_for_version, validate_actions,
        validate_update_without_base, validate_v2_update_existing_forbidden,
        FIX_PLAN_SYSTEM_PROMPT, LLM_PLAN_SCHEMA_VERSION,
    };
    use crate::types::{Action, ActionKind};
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_protocol_fallback_applicable() {
        std::env::set_var("PAPAYU_PROTOCOL_DEFAULT", "2");
        std::env::set_var("PAPAYU_PROTOCOL_FALLBACK_TO_V1", "1");
        assert!(!is_protocol_fallback_applicable(
            "ERR_PATCH_APPLY_FAILED",
            0
        )); // repair-first
        assert!(is_protocol_fallback_applicable("ERR_PATCH_APPLY_FAILED", 1));
        assert!(is_protocol_fallback_applicable("ERR_NON_UTF8_FILE", 0)); // immediate fallback
        assert!(!is_protocol_fallback_applicable(
            "ERR_V2_UPDATE_EXISTING_FORBIDDEN",
            0
        )); // repair-first
        assert!(is_protocol_fallback_applicable(
            "ERR_V2_UPDATE_EXISTING_FORBIDDEN",
            1
        ));
        assert!(!is_protocol_fallback_applicable("ERR_BASE_MISMATCH", 0)); // sha repair, not fallback
        std::env::remove_var("PAPAYU_PROTOCOL_DEFAULT");
        std::env::remove_var("PAPAYU_PROTOCOL_FALLBACK_TO_V1");
    }

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
    fn test_schema_v2_compiles() {
        let schema: serde_json::Value =
            serde_json::from_str(super::SCHEMA_V2_RAW).expect("v2 schema valid JSON");
        let compiled = jsonschema::JSONSchema::options().compile(&schema);
        assert!(compiled.is_ok(), "v2 schema must compile");
    }

    #[test]
    fn test_schema_hash_non_empty_v2() {
        let h = schema_hash_for_version(2);
        assert!(!h.is_empty());
        assert_eq!(h.len(), 64);
    }

    /// Run with: cargo test golden_traces_v2_schema_hash -- --nocapture
    #[test]
    #[ignore]
    fn golden_traces_v2_schema_hash() {
        eprintln!("v2 schema_hash: {}", schema_hash_for_version(2));
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
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_ok());
    }

    #[test]
    fn test_validate_actions_rejects_parent_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "../etc/passwd".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_absolute_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "/etc/passwd".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_path_ending_with_dotdot() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "a/..".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_windows_drive() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "C:/foo/bar".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_unc_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "//server/share/file".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_dot_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateDir,
            path: ".".to_string(),
            content: None,
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_dot_segment() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "a/./b".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_allows_relative_prefix() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "./src/main.rs".to_string(),
            content: Some("fn main() {}".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
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
                patch: None,
                base_sha256: None,
                edits: None,
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("b".to_string()),
                patch: None,
                base_sha256: None,
                edits: None,
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
                patch: None,
                base_sha256: None,
                edits: None,
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("b".to_string()),
                patch: None,
                base_sha256: None,
                edits: None,
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
    fn test_extract_files_from_plan_context_v2_sha256() {
        let ctx = "FILE[src/parser.py] (sha256=7f3f2a0c9f8b1a0c9b4c0f9e3d8a4b2d8c9e7f1a0b3c4d5e6f7a8b9c0d1e2f3a):\n1|def parse";
        let paths = extract_files_read_from_plan_context(ctx);
        assert!(paths.contains("src/parser.py"));
    }

    #[test]
    fn test_validate_update_without_base_ok() {
        let ctx = "FILE[foo.txt]:\nold\n\n=== bar.txt ===\ncontent\n";
        let actions = vec![
            Action {
                kind: ActionKind::UpdateFile,
                path: "foo.txt".to_string(),
                content: Some("new".to_string()),
                patch: None,
                base_sha256: None,
                edits: None,
            },
            Action {
                kind: ActionKind::UpdateFile,
                path: "bar.txt".to_string(),
                content: Some("updated".to_string()),
                patch: None,
                base_sha256: None,
                edits: None,
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
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_update_without_base(&actions, Some(ctx)).is_err());
    }

    #[test]
    fn test_validate_actions_rejects_tilde_path() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "~/etc/passwd".to_string(),
            content: Some("x".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        assert!(validate_actions(&actions).is_err());
    }

    #[test]
    fn test_validate_actions_requires_content_for_create_file() {
        let actions = vec![Action {
            kind: ActionKind::CreateFile,
            path: "README.md".to_string(),
            content: None,
            patch: None,
            base_sha256: None,
            edits: None,
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

    #[test]
    fn test_v2_update_existing_forbidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::env::set_var("PAPAYU_PROTOCOL_VERSION", "2");

        let actions = vec![Action {
            kind: ActionKind::UpdateFile,
            path: "src/main.rs".to_string(),
            content: Some("fn main() { println!(\"x\"); }\n".to_string()),
            patch: None,
            base_sha256: None,
            edits: None,
        }];
        let r = validate_v2_update_existing_forbidden(root, &actions);
        std::env::remove_var("PAPAYU_PROTOCOL_VERSION");

        assert!(r.is_err());
        let e = r.unwrap_err();
        assert!(e.contains("ERR_V2_UPDATE_EXISTING_FORBIDDEN"));
        assert!(e.contains("PATCH_FILE"));
    }

    #[test]
    fn test_build_repair_prompt_injects_sha256() {
        let sha = "a".repeat(64);
        std::env::set_var("PAPAYU_PROTOCOL_VERSION", "2");
        let ctx = format!("FILE[src/main.rs] (sha256={}):\nfn main() {{}}\n", sha);
        let validated = serde_json::json!({
            "actions": [{
                "kind": "PATCH_FILE",
                "path": "src/main.rs",
                "base_sha256": "wrong",
                "patch": "--- a/foo\n+++ b/foo\n@@ -1,1 +1,2 @@\nold\n+new"
            }]
        });
        let result = build_v2_patch_repair_prompt_with_sha(&ctx, &validated);
        std::env::remove_var("PAPAYU_PROTOCOL_VERSION");
        assert!(result.is_some());
        let (p, paths) = result.unwrap();
        assert!(p.contains("base_sha256"));
        assert!(p.contains(&sha));
        assert!(p.contains("src/main.rs"));
        assert_eq!(paths, vec!["src/main.rs"]);
    }

    #[test]
    fn test_repair_prompt_fallback_when_sha_missing() {
        std::env::set_var("PAPAYU_PROTOCOL_VERSION", "2");
        let ctx = "FILE[src/main.rs]:\nfn main() {}\n";
        let validated = serde_json::json!({
            "actions": [{
                "kind": "PATCH_FILE",
                "path": "src/main.rs",
                "base_sha256": "wrong",
                "patch": "--- a/foo\n+++ b/foo\n@@ -1,1 +1,2 @@\nold\n+new"
            }]
        });
        let result = build_v2_patch_repair_prompt_with_sha(ctx, &validated);
        std::env::remove_var("PAPAYU_PROTOCOL_VERSION");
        assert!(result.is_none());
    }

    #[test]
    fn test_repair_prompt_not_generated_when_base_matches() {
        let sha = "b".repeat(64);
        std::env::set_var("PAPAYU_PROTOCOL_VERSION", "2");
        let ctx = format!("FILE[src/foo.rs] (sha256={}):\ncontent\n", sha);
        let validated = serde_json::json!({
            "actions": [{
                "kind": "PATCH_FILE",
                "path": "src/foo.rs",
                "base_sha256": sha,
                "patch": "--- a/foo\n+++ b/foo\n@@ -1,1 +1,2 @@\ncontent\n+more"
            }]
        });
        let result = build_v2_patch_repair_prompt_with_sha(&ctx, &validated);
        std::env::remove_var("PAPAYU_PROTOCOL_VERSION");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_actions_from_json_patch_file() {
        let sha = "a".repeat(64);
        let actions_str = format!(
            r#"[{{"kind":"PATCH_FILE","path":"src/main.rs","patch":"--- a/foo\n+++ b/foo\n@@ -1,1 +1,2 @@\nold\n+new","base_sha256":"{}"}}]"#,
            sha
        );
        let actions = parse_actions_from_json(&actions_str).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, ActionKind::PatchFile);
        assert_eq!(actions[0].path, "src/main.rs");
        assert!(actions[0].patch.is_some());
        assert_eq!(actions[0].base_sha256.as_deref(), Some(sha.as_str()));
    }

    #[test]
    fn golden_traces_v1_validate() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/golden_traces/v1");
        if !dir.exists() {
            return;
        }
        let expected_schema_hash = schema_hash();
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            let s = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", name));
            let v: serde_json::Value =
                serde_json::from_str(&s).unwrap_or_else(|e| panic!("{}: json {}", name, e));

            assert_eq!(
                v.get("protocol")
                    .and_then(|p| p.get("schema_version"))
                    .and_then(|x| x.as_u64()),
                Some(1),
                "{}: schema_version",
                name
            );
            let sh = v
                .get("protocol")
                .and_then(|p| p.get("schema_hash"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            assert_eq!(sh, expected_schema_hash, "{}: schema_hash", name);

            let validated = v
                .get("result")
                .and_then(|r| r.get("validated_json"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if validated.is_null() {
                continue;
            }
            super::validate_json_against_schema(&validated)
                .unwrap_or_else(|e| panic!("{}: schema validation: {}", name, e));

            let validated_str = serde_json::to_string(&validated).unwrap();
            let parsed = super::parse_plan_response(&validated_str)
                .unwrap_or_else(|e| panic!("{}: parse validated_json: {}", name, e));

            if v.get("result")
                .and_then(|r| r.get("validation_outcome"))
                .and_then(|x| x.as_str())
                == Some("ok")
            {
                assert!(
                    validate_actions(&parsed.actions).is_ok(),
                    "{}: validate_actions",
                    name
                );
            }

            let mode = v
                .get("request")
                .and_then(|r| r.get("mode"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if mode == "apply" && parsed.actions.is_empty() {
                let summary = validated
                    .get("summary")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                assert!(
                    summary.starts_with("NO_CHANGES:"),
                    "{}: apply with empty actions requires NO_CHANGES: prefix in summary",
                    name
                );
            }

            let ctx_stats = v.get("context").and_then(|c| c.get("context_stats"));
            let cache_stats = v.get("context").and_then(|c| c.get("cache_stats"));
            if let Some(stats) = ctx_stats {
                for key in ["context_files_count", "context_total_chars"] {
                    if let Some(n) = stats.get(key).and_then(|x| x.as_u64()) {
                        assert!(n <= 1_000_000, "{}: {} reasonable", name, key);
                    }
                }
            }
            if let Some(stats) = cache_stats {
                for key in ["env_hits", "env_misses", "read_hits", "read_misses"] {
                    if let Some(n) = stats.get(key).and_then(|x| x.as_u64()) {
                        assert!(n <= 1_000_000, "{}: cache {} reasonable", name, key);
                    }
                }
            }
        }
    }

    #[test]
    fn golden_traces_v2_validate() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/golden_traces/v2");
        if !dir.exists() {
            return;
        }
        let expected_schema_hash = schema_hash_for_version(2);
        let v2_schema = compiled_schema_for_version(2).expect("v2 schema must compile");
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            let s = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", name));
            let v: serde_json::Value =
                serde_json::from_str(&s).unwrap_or_else(|e| panic!("{}: json {}", name, e));

            assert_eq!(
                v.get("protocol")
                    .and_then(|p| p.get("schema_version"))
                    .and_then(|x| x.as_u64()),
                Some(2),
                "{}: schema_version must be 2",
                name
            );
            let sh = v
                .get("protocol")
                .and_then(|p| p.get("schema_hash"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            assert_eq!(sh, expected_schema_hash, "{}: schema_hash", name);

            let validated = v
                .get("result")
                .and_then(|r| r.get("validated_json"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if validated.is_null() {
                continue;
            }
            v2_schema
                .validate(&validated)
                .map_err(|errs| {
                    let msgs: Vec<String> = errs.map(|e| e.to_string()).collect();
                    format!("{}: v2 schema validation: {}", name, msgs.join("; "))
                })
                .unwrap();

            let validated_str = serde_json::to_string(&validated).unwrap();
            let parsed = super::parse_plan_response(&validated_str)
                .unwrap_or_else(|e| panic!("{}: parse validated_json: {}", name, e));

            if v.get("result")
                .and_then(|r| r.get("validation_outcome"))
                .and_then(|x| x.as_str())
                == Some("ok")
            {
                assert!(
                    validate_actions(&parsed.actions).is_ok(),
                    "{}: validate_actions",
                    name
                );
            }

            let mode = v
                .get("request")
                .and_then(|r| r.get("mode"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if mode == "apply" && parsed.actions.is_empty() {
                let summary = validated
                    .get("summary")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                assert!(
                    summary.starts_with("NO_CHANGES:"),
                    "{}: apply with empty actions requires NO_CHANGES: prefix in summary",
                    name
                );
            }
        }
    }

    #[test]
    fn golden_traces_v3_validate() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/golden_traces/v3");
        if !dir.exists() {
            return;
        }
        let expected_schema_hash = schema_hash_for_version(3);
        let v3_schema = compiled_schema_for_version(3).expect("v3 schema must compile");
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            let s = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", name));
            let v: serde_json::Value =
                serde_json::from_str(&s).unwrap_or_else(|e| panic!("{}: json {}", name, e));

            assert_eq!(
                v.get("protocol")
                    .and_then(|p| p.get("schema_version"))
                    .and_then(|x| x.as_u64()),
                Some(3),
                "{}: schema_version must be 3",
                name
            );
            let sh = v
                .get("protocol")
                .and_then(|p| p.get("schema_hash"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            assert_eq!(sh, expected_schema_hash, "{}: schema_hash", name);

            let validated = v
                .get("result")
                .and_then(|r| r.get("validated_json"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if validated.is_null() {
                continue;
            }
            v3_schema
                .validate(&validated)
                .map_err(|errs| {
                    let msgs: Vec<String> = errs.map(|e| e.to_string()).collect();
                    format!("{}: v3 schema validation: {}", name, msgs.join("; "))
                })
                .unwrap();

            let validated_str = serde_json::to_string(&validated).unwrap();
            let parsed = super::parse_plan_response(&validated_str)
                .unwrap_or_else(|e| panic!("{}: parse validated_json: {}", name, e));

            if v.get("result")
                .and_then(|r| r.get("validation_outcome"))
                .and_then(|x| x.as_str())
                == Some("ok")
            {
                assert!(
                    validate_actions(&parsed.actions).is_ok(),
                    "{}: validate_actions",
                    name
                );
            }

            let mode = v
                .get("request")
                .and_then(|r| r.get("mode"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if mode == "apply" && parsed.actions.is_empty() {
                let summary = validated
                    .get("summary")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                assert!(
                    summary.starts_with("NO_CHANGES:"),
                    "{}: apply with empty actions requires NO_CHANGES: prefix in summary",
                    name
                );
            }

            for a in &parsed.actions {
                if a.kind == ActionKind::EditFile {
                    assert!(
                        a.base_sha256
                            .as_ref()
                            .map(|s| s.len() == 64)
                            .unwrap_or(false),
                        "{}: EDIT_FILE must have base_sha256",
                        name
                    );
                    assert!(
                        a.edits.as_ref().map(|e| !e.is_empty()).unwrap_or(false),
                        "{}: EDIT_FILE must have non-empty edits",
                        name
                    );
                }
            }
        }
    }
}
