# PAPA YU Tests

## Структура

```
tests/
├── README.md           # Этот файл
└── fixtures/           # Тестовые фикстуры (минимальные проекты)
    ├── minimal-node/   # Node.js проект без README
    └── minimal-rust/   # Rust проект без README

docs/golden_traces/     # Эталонные трассы (регрессия, без raw_content)
├── README.md
└── v1/                 # Protocol v1 fixtures
    001_fix_bug_plan.json
    002_fix_bug_apply.json
    ...
```

## Юнит-тесты (Rust)

Запуск всех юнит-тестов:

```bash
cd src-tauri
cargo test
```

Текущие тесты покрывают:
- `golden_traces_v1_validate` — валидация fixtures в `docs/golden_traces/v1/` (schema_version, schema_hash, parse, validate_actions, NO_CHANGES)
- `detect_project_type` — определение типа проекта
- `get_project_limits` — лимиты по типу проекта
- `is_protected_file` — защита служебных файлов
- `is_text_allowed` — фильтр текстовых файлов
- `settings_export` — экспорт/импорт настроек

## E2E сценарий (ручной)

См. `docs/E2E_SCENARIO.md` для пошагового сценария:

1. Запустить приложение: `npm run tauri dev`
2. Выбрать одну из фикстур (например, `tests/fixtures/minimal-node`)
3. Запустить анализ
4. Применить рекомендованные исправления
5. Проверить, что README.md создан
6. Откатить изменения (Undo)
7. Проверить, что README.md удалён

## Тестовые фикстуры

### minimal-node

Минимальный Node.js проект:
- `package.json` — манифест пакета
- `index.js` — точка входа
- **Нет README** — должен быть предложен при анализе

### minimal-rust

Минимальный Rust проект:
- `Cargo.toml` — манифест пакета
- `src/main.rs` — точка входа
- **Нет README** — должен быть предложен при анализе

## Автоматизация E2E (будущее)

Планируется использовать:
- **Tauri test** — для тестирования команд
- **Playwright** — для тестирования UI
