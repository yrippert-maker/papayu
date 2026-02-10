#!/bin/bash
# Запуск PAPA YU с подключением к OpenAI.
# Ключ API храните только в .env.openai на своём компьютере (не передавайте в чат и не коммитьте).

cd "$(dirname "$0")"

if [ ! -f .env.openai ]; then
  echo "Файл .env.openai не найден."
  echo ""
  echo "1. Скопируйте шаблон:"
  echo "   cp env.openai.example .env.openai"
  echo ""
  echo "2. Откройте .env.openai и замените your-openai-key-here на ваш ключ OpenAI (sk-...)."
  echo ""
  exit 1
fi

# Загружаем переменные, убирая \r (Windows-переносы), чтобы не было "command not found"
export $(grep -v '^#' .env.openai | grep -v '^$' | sed 's/\r$//' | xargs)


npm run tauri dev
