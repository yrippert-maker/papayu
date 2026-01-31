# Подключение PAPA-YU к OpenAI

Инструкция по настройке кнопки **«Предложить исправления»** для работы через API OpenAI.

---

## 1. Получение API-ключа OpenAI

1. Зайдите на [platform.openai.com](https://platform.openai.com).
2. Войдите в аккаунт или зарегистрируйтесь.
3. Откройте **API keys** (раздел **Settings** → **API keys** или [прямая ссылка](https://platform.openai.com/api-keys)).
4. Нажмите **Create new secret key**, задайте имя (например, `PAPA-YU`) и скопируйте ключ.
5. Сохраните ключ в надёжном месте — повторно его показать нельзя.

---

## 2. Переменные окружения

Перед запуском приложения задайте три переменные.

### Обязательные

| Переменная | Значение | Описание |
|------------|----------|----------|
| `PAPAYU_LLM_API_URL` | `https://api.openai.com/v1/chat/completions` | URL эндпоинта Chat Completions OpenAI. |
| `PAPAYU_LLM_API_KEY` | Ваш API-ключ OpenAI | Ключ передаётся в заголовке `Authorization: Bearer <ключ>`. |

### Опциональные

| Переменная | Значение по умолчанию | Описание |
|------------|------------------------|----------|
| `PAPAYU_LLM_MODEL` | `gpt-4o-mini` | Модель для генерации плана (например, `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`). |
| `PAPAYU_LLM_MODE` | `chat` | Режим агента: `chat` (инженер-коллега) или `fixit` (обязан вернуть патч + проверку). См. `docs/AGENT_CONTRACT.md`. |

---

## 3. Запуск с OpenAI

### Вариант A: В текущей сессии терминала (macOS / Linux)

```bash
cd /Users/yrippertgmail.com/Desktop/papa-yu

export PAPAYU_LLM_API_URL="https://api.openai.com/v1/chat/completions"
export PAPAYU_LLM_API_KEY="sk-ваш-ключ-openai"
export PAPAYU_LLM_MODEL="gpt-4o-mini"

npm run tauri dev
```

Подставьте вместо `sk-ваш-ключ-openai` свой ключ.

### Вариант B: Одна строкой (без сохранения ключа в истории)

```bash
cd /Users/yrippertgmail.com/Desktop/papa-yu
PAPAYU_LLM_API_URL="https://api.openai.com/v1/chat/completions" \
PAPAYU_LLM_API_KEY="sk-ваш-ключ" \
PAPAYU_LLM_MODEL="gpt-4o-mini" \
npm run tauri dev
```

### Вариант C: Файл `.env` в корне проекта (если приложение его подхватывает)

В PAPA-YU переменные читаются из окружения процесса. Tauri сам по себе не загружает `.env`. Чтобы использовать `.env`, можно запускать через `env` или скрипт:

```bash
# В papa-yu создайте файл .env (добавьте .env в .gitignore, чтобы не коммитить ключ):
# PAPAYU_LLM_API_URL=https://api.openai.com/v1/chat/completions
# PAPAYU_LLM_API_KEY=sk-ваш-ключ
# PAPAYU_LLM_MODEL=gpt-4o-mini

# Запуск с подгрузкой .env (macOS/Linux, если установлен dotenv-cli):
# npm install -g dotenv-cli
# dotenv -e .env -- npm run tauri dev
```

Или простой скрипт `start-with-openai.sh`:

```bash
#!/bin/bash
cd "$(dirname "$0")"
set -a
source .env   # или export переменные здесь
set +a
npm run tauri dev
```

---

## 4. Проверка

1. Запустите приложение с заданными переменными.
2. Выберите проект (папку или путь).
3. Запустите **Анализ**.
4. Введите цель (например: «Добавить README и .gitignore»).
5. Нажмите **«Предложить исправления»**.

Если всё настроено верно, план будет сформирован через OpenAI. В случае ошибки в интерфейсе или в логах будет указание на API (например, 401 — неверный ключ, 429 — лимиты).

---

## 5. Безопасность

- Не коммитьте API-ключ в репозиторий и не вставляйте его в скрипты, которые попадают в историю.
- Добавьте `.env` в `.gitignore`, если храните ключ в `.env`.
- При утечке ключа отзовите его в [OpenAI API keys](https://platform.openai.com/api-keys) и создайте новый.

---

## 6. Другие модели OpenAI

Можно указать другую модель через `PAPAYU_LLM_MODEL`, например:

- `gpt-4o` — более способная модель.
- `gpt-4o-mini` — быстрее и дешевле (по умолчанию в коде).
- `gpt-4-turbo` — баланс качества и скорости.

Актуальный список и цены: [OpenAI Pricing](https://openai.com/pricing).

---

## 7. Если переменные не заданы

Если `PAPAYU_LLM_API_URL` не задана или пустая, кнопка **«Предложить исправления»** работает без API: используется встроенная эвристика (правила для README, .gitignore, LICENSE, .env.example и т.п.).
