#!/bin/bash
# PAPA YU — сборка приложения и запуск (первая установка или после обновления кода).
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "  Сборка PAPA YU..."
export CI=false
if ! npm run tauri build; then
  echo ""
  echo "  Ошибка сборки. Проверьте: npm install, Rust и Xcode Command Line Tools."
  read -n 1 -s -r -p "  Нажмите любую клавишу..."
  exit 1
fi

BUNDLE="$SCRIPT_DIR/src-tauri/target/release/bundle/macos/PAPA YU.app"
[ -d "$BUNDLE" ] && open "$BUNDLE" || open "$SCRIPT_DIR/src-tauri/target/release/bundle/macos"
echo "  Готово."
