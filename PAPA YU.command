#!/bin/bash
# PAPA YU — запуск приложения (двойной клик). Сборка не выполняется.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUNDLE_DIR="$SCRIPT_DIR/src-tauri/target/release/bundle/macos"

find_app() {
  [ -d "$BUNDLE_DIR/PAPA YU.app" ] && echo "$BUNDLE_DIR/PAPA YU.app" && return
  for f in "$BUNDLE_DIR"/*.app; do
    [ -d "$f" ] && echo "$f" && return
  done
  echo ""
}

APP=$(find_app)
if [ -n "$APP" ]; then
  open "$APP"
  exit 0
fi

echo ""
echo "  PAPA YU не найден."
echo "  Для первой сборки запустите: «PAPA YU — Сборка и запуск.command»"
echo ""
read -n 1 -s -r -p "  Нажмите любую клавишу..."
exit 1
