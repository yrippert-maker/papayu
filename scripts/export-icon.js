#!/usr/bin/env node
/**
 * Экспорт иконки: SVG → PNG 1024x1024 для сборки Tauri.
 * Варианты: ImageMagick (convert/magick), иначе npm install sharp && node scripts/export-icon.js
 */
const path = require('path');
const fs = require('fs');
const { execSync } = require('child_process');

const src = path.join(__dirname, '../src-tauri/icons/icon.svg');
const out = path.join(__dirname, '../src-tauri/icons/icon.png');

if (!fs.existsSync(src)) {
  console.error('Не найден файл:', src);
  process.exit(1);
}

function tryImageMagick() {
  try {
    execSync('convert -version', { stdio: 'ignore' });
    execSync(`convert -background none -resize 1024x1024 "${src}" "${out}"`, { stdio: 'inherit' });
    return true;
  } catch (_) {}
  try {
    execSync('magick -version', { stdio: 'ignore' });
    execSync(`magick convert -background none -resize 1024x1024 "${src}" "${out}"`, { stdio: 'inherit' });
    return true;
  } catch (_) {}
  return false;
}

async function run() {
  if (tryImageMagick()) {
    console.log('Иконка экспортирована (ImageMagick):', out);
    return;
  }
  try {
    const sharp = require('sharp');
    await sharp(src)
      .resize(1024, 1024)
      .png()
      .toFile(out);
    console.log('Иконка экспортирована (sharp):', out);
  } catch (e) {
    if (e.code === 'MODULE_NOT_FOUND') {
      console.error('Установите sharp: npm install --save-dev sharp');
      console.error('Или экспортируйте вручную: откройте src-tauri/icons/icon.svg в браузере/редакторе и сохраните как PNG 1024×1024 в icon.png');
    } else {
      console.error(e.message);
    }
    process.exit(1);
  }
}

run();
