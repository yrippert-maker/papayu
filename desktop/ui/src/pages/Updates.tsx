import { useState } from 'react';
import { RefreshCw, Copy, Check } from 'lucide-react';

const UPDATER_ENDPOINT = 'https://github.com/yrippert-maker/papayu/releases/latest/download/latest.json';
const CHANNEL = 'stable';

export function Updates() {
  const [checkResult, setCheckResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [logLines, setLogLines] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);

  const addLog = (line: string) => {
    setLogLines((prev) => [...prev, `${new Date().toISOString()} ${line}`]);
  };

  const handleCheck = async () => {
    setIsChecking(true);
    setCheckResult(null);
    setLogLines([]);
    addLog('Запрос проверки обновлений…');
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const { getVersion } = await import('@tauri-apps/api/app');
      const currentVersion = await getVersion();
      addLog(`Текущая версия: ${currentVersion}`);
      addLog(`Endpoint: ${UPDATER_ENDPOINT}`);
      addLog(`Канал: ${CHANNEL}`);
      const update = await check();
      if (!update) {
        addLog('Обновлений нет.');
        setCheckResult({ ok: true, message: 'Обновлений нет. У вас актуальная версия.' });
        return;
      }
      addLog(`Доступна версия: ${update.version}`);
      setCheckResult({ ok: true, message: `Доступна версия ${update.version}. Нажмите «Установить» в шапке приложения.` });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      addLog(`Ошибка: ${msg}`);
      const friendly =
        msg && (msg.includes('fetch') || msg.includes('valid') || msg.includes('signature'))
          ? 'Обновления пока недоступны (сервер или подпись не настроены).'
          : msg || 'Ошибка проверки обновлений.';
      setCheckResult({ ok: false, message: friendly });
    } finally {
      setIsChecking(false);
    }
  };

  const copyLog = async () => {
    const text = logLines.join('\n') || 'Лог пуст.';
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="max-w-2xl mx-auto px-4 py-8">
      <h1 className="text-xl font-semibold text-foreground mb-6">Обновления</h1>
      <div className="space-y-4 rounded-lg border bg-card p-4">
        <p className="text-sm text-muted-foreground">
          Endpoint: <code className="bg-muted px-1 rounded text-xs break-all">{UPDATER_ENDPOINT}</code>
        </p>
        <p className="text-sm text-muted-foreground">Канал: {CHANNEL}</p>
        <button
          type="button"
          onClick={handleCheck}
          disabled={isChecking}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground font-medium hover:bg-primary/90 disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${isChecking ? 'animate-spin' : ''}`} />
          {isChecking ? 'Проверка…' : 'Проверить обновления'}
        </button>
        {checkResult && (
          <div
            className={`text-sm p-3 rounded-lg ${
              checkResult.ok ? 'bg-green-500/10 text-green-800 dark:text-green-400' : 'bg-amber-500/10 text-amber-800 dark:text-amber-400'
            }`}
          >
            {checkResult.message}
          </div>
        )}
        {logLines.length > 0 && (
          <div className="mt-4">
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs font-medium text-muted-foreground">Лог</span>
              <button
                type="button"
                onClick={copyLog}
                className="inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline"
              >
                {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                {copied ? 'Скопировано' : 'Скопировать лог'}
              </button>
            </div>
            <pre className="text-xs bg-muted/50 rounded p-3 max-h-40 overflow-auto font-mono whitespace-pre-wrap break-words">
              {logLines.join('\n')}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
