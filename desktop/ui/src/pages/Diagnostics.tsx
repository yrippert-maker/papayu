import { useState, useEffect } from 'react';
import { Copy, Check, Download } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';

interface AppInfo {
  version: string;
  app_data_dir: string | null;
  app_config_dir: string | null;
}

export function Diagnostics() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [tauriVersion, setTauriVersion] = useState<string>('—');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const info = await invoke<AppInfo>('get_app_info');
        setAppInfo(info);
      } catch (_) {
        setAppInfo(null);
      }
      try {
        const v = await getVersion();
        setTauriVersion(v);
      } catch (_) {}
    })();
  }, []);

  const buildDiagnosticsText = () => {
    const lines = [
      `PAPA YU Diagnostics — ${new Date().toISOString()}`,
      '',
      'Версии:',
      `  App (package): ${appInfo?.version ?? '—'}`,
      `  Tauri (getVersion): ${tauriVersion}`,
      '',
      'Пути (системные директории Tauri/OS):',
      `  app_data_dir: ${appInfo?.app_data_dir ?? '—'}`,
      `  app_config_dir: ${appInfo?.app_config_dir ?? '—'}`,
      '',
      'Updater:',
      '  endpoint: https://github.com/yrippert-maker/papayu/releases/latest/download/latest.json',
      '  подпись: требуется (pubkey в tauri.conf.json)',
      '',
    ];
    return lines.join('\n');
  };

  const handleCopy = async () => {
    const text = buildDiagnosticsText();
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleExport = () => {
    const text = buildDiagnosticsText();
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `papayu-diagnostics-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="max-w-2xl mx-auto px-4 py-8">
      <h1 className="text-xl font-semibold text-foreground mb-6">Диагностика</h1>
      <div className="space-y-6 rounded-lg border bg-card p-4">
        <section>
          <h2 className="text-sm font-semibold text-muted-foreground mb-2">Версии</h2>
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted-foreground">Приложение:</dt>
            <dd className="font-mono">{appInfo?.version ?? tauriVersion ?? '—'}</dd>
            <dt className="text-muted-foreground">Tauri:</dt>
            <dd className="font-mono">{tauriVersion}</dd>
          </dl>
        </section>
        <section>
          <h2 className="text-sm font-semibold text-muted-foreground mb-2">Пути данных</h2>
          <p className="text-xs text-muted-foreground mb-1">Используются системные директории (не зависят от $HOME):</p>
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm font-mono break-all">
            <dt className="text-muted-foreground">app_data_dir:</dt>
            <dd>{appInfo?.app_data_dir ?? '—'}</dd>
            <dt className="text-muted-foreground">app_config_dir:</dt>
            <dd>{appInfo?.app_config_dir ?? '—'}</dd>
          </dl>
        </section>
        <section>
          <h2 className="text-sm font-semibold text-muted-foreground mb-2">Состояние обновлений</h2>
          <p className="text-sm text-muted-foreground">
            Endpoint: <code className="bg-muted px-1 rounded text-xs">…/releases/latest/download/latest.json</code>
          </p>
          <p className="text-xs text-muted-foreground mt-1">
            Подпись обязательна; pubkey задаётся в tauri.conf.json. Если ключ не настроен — проверка обновлений вернёт ошибку.
          </p>
        </section>
        <div className="flex gap-2 pt-2">
          <button
            type="button"
            onClick={handleCopy}
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border font-medium text-sm hover:bg-muted"
          >
            {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
            {copied ? 'Скопировано' : 'Скопировать отчёт'}
          </button>
          <button
            type="button"
            onClick={handleExport}
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border font-medium text-sm hover:bg-muted"
          >
            <Download className="w-4 h-4" />
            Экспортировать логи
          </button>
        </div>
      </div>
    </div>
  );
}
