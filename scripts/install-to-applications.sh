#!/usr/bin/env bash
# Устанавливает PAPA YU в папку «Программы» (/Applications).
# После этого приложение можно запускать из Launchpad или Finder без терминала.

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUNDLE_DIR="$ROOT_DIR/src-tauri/target/release/bundle/macos"
APP_NAME="PAPA YU.app"
APPLICATIONS="/Applications"

cd "$ROOT_DIR"

if [ ! -d "$BUNDLE_DIR/$APP_NAME" ]; then
  echo "  Сборка приложения..."
  export CI=false
  npm run tauri build
fi

if [ ! -d "$BUNDLE_DIR/$APP_NAME" ]; then
  echo "  Ошибка: после сборки не найден $BUNDLE_DIR/$APP_NAME"
  exit 1
fi

echo "  Копирование в $APPLICATIONS..."
rm -rf "$APPLICATIONS/$APP_NAME"
cp -R "$BUNDLE_DIR/$APP_NAME" "$APPLICATIONS/"

echo "  Обновление Launchpad (чтобы иконка появилась)..."
defaults write com.apple.dock ResetLaunchPad -bool true 2>/dev/null || true
killall Dock 2>/dev/null || true

echo ""
echo "  Готово. PAPA YU установлен в «Программы»."
echo "  Иконка должна появиться в Launchpad через несколько секунд."
echo "  Также: Spotlight (Cmd+Пробел) → «PAPA YU» или Finder → Программы."
echo ""
