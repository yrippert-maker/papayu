# Иконка приложения PAPA YU

- **icon.svg** — исходная иконка (код/скобки + галочка «исправлено», синий фон, оранжевый акцент).
- **icon.png** — используется в сборке Tauri (1024×1024).

Чтобы пересобрать PNG из SVG (после изменения иконки):

```bash
# из корня проекта papa-yu
npm run icons:export
```

Требуется один из вариантов:
- **ImageMagick:** `brew install imagemagick` (на macOS)
- **sharp:** `npm install --save-dev sharp`

Либо откройте `icon.svg` в браузере или редакторе (Figma, Inkscape) и экспортируйте как PNG 1024×1024 в `icon.png`.
