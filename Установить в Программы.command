#!/bin/bash
# Установка PAPA YU в «Программы». Запустите двойным щелчком один раз.
# После установки открывайте приложение из Launchpad или Finder — терминал не нужен.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"
bash scripts/install-to-applications.sh
echo ""
read -n 1 -s -r -p "Нажмите любую клавишу для выхода..."
