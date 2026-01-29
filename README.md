# PAPA YU

[![CI](https://github.com/yrippert-maker/papayu/actions/workflows/ci.yml/badge.svg)](https://github.com/yrippert-maker/papayu/actions/workflows/ci.yml)

Десктопное приложение (Tauri 2 + React).

---

## Сборка и запуск

- **Разработка:** из корня репозитория: `cd desktop/src-tauri && cargo tauri dev` (или из `desktop/ui` — `npm run dev`, отдельно backend по необходимости).
- **Сборка:** `cd desktop/src-tauri && cargo tauri build`.
- Подробнее: [docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md](docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md).

---

## Release process

- Релизы собираются в GitHub Actions по тегам `v*` (workflow **Release**).
- Чеклист выпуска релиза и проверки обновлений: [docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md](docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md).
- Если сборка падает: [docs/CI_ОТЛАДКА_РЕЛИЗА.md](docs/CI_ОТЛАДКА_РЕЛИЗА.md) — классификация ошибок и точечные патчи.

---

## Документация

| Документ | Описание |
|----------|----------|
| [docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md](docs/РЕЛИЗ_И_ОБНОВЛЕНИЯ.md) | Релиз, теги, секреты, проверка обновлений |
| [docs/CI_ОТЛАДКА_РЕЛИЗА.md](docs/CI_ОТЛАДКА_РЕЛИЗА.md) | Отладка падений `tauri build` в CI |
| [docs/SSH_НАСТРОЙКА.md](docs/SSH_НАСТРОЙКА.md) | Настройка SSH для GitHub |
| [docs/CONTRACTS.md](docs/CONTRACTS.md) | Контракты и архитектура |
