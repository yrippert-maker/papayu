import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { open } from '@tauri-apps/plugin-dialog';
import { MessageSquare, ArrowLeft, RotateCcw, Trash2, FolderOpen } from 'lucide-react';
import { analyzeProject, type AnalyzeReport } from '../lib/analyze';

type Message =
  | { role: 'user'; text: string }
  | { role: 'assistant'; text: string }
  | { role: 'assistant'; report: AnalyzeReport; error?: string };

export function ChatAgent() {
  const navigate = useNavigate();
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');

  const handleClearChat = () => {
    setMessages([]);
  };

  const handleUndo = () => {
    if (messages.length > 0) {
      setMessages((prev) => prev.slice(0, -1));
    }
  };

  const handleSend = () => {
    if (!input.trim()) return;
    setMessages((prev) => [...prev, { role: 'user', text: input.trim() }]);
    setInput('');
    setTimeout(() => {
      setMessages((prev) => [
        ...prev,
        { role: 'assistant', text: 'Ответ ИИ агента будет отображаться здесь. Результаты действий агента подключаются к backend.' },
      ]);
    }, 500);
  };

  const handlePickFolderAndAnalyze = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
    });
    if (!selected) return;

    const pathStr = selected;

    setMessages((prev) => [
      ...prev,
      { role: 'user', text: `Проанализируй проект: ${pathStr}` },
      { role: 'assistant', text: 'Индексирую файлы…' },
    ]);

    try {
      const report = await analyzeProject(pathStr);
      setMessages((prev) => {
        const next = [...prev];
        next[next.length - 1] = { role: 'assistant', report };
        return next;
      });
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setMessages((prev) => {
        const next = [...prev];
        next[next.length - 1] = { role: 'assistant', report: {} as AnalyzeReport, error: errMsg };
        return next;
      });
    }
  };

  return (
    <div className="min-h-screen flex flex-col bg-background">
      <div className="p-4 border-b flex items-center justify-between flex-wrap gap-2">
        <button
          onClick={() => navigate('/')}
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="w-4 h-4" />
          Назад
        </button>
        <div className="flex items-center gap-2">
          <button
            onClick={handlePickFolderAndAnalyze}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border border-primary/50 text-primary text-sm font-medium hover:bg-primary/10"
          >
            <FolderOpen className="w-4 h-4" />
            Выбрать папку
          </button>
          <button
            onClick={handleClearChat}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted"
          >
            <Trash2 className="w-4 h-4" />
            Очистка чата
          </button>
          <button
            onClick={handleUndo}
            disabled={messages.length === 0}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <RotateCcw className="w-4 h-4" />
            Откат
          </button>
        </div>
      </div>

      <div className="flex-1 p-6 overflow-auto">
        <div className="max-w-2xl mx-auto">
          {messages.length === 0 ? (
            <div className="text-center py-16 text-muted-foreground">
              <MessageSquare className="w-12 h-12 mx-auto mb-4 opacity-50" />
              <p>Диалог с ИИ агентом. Результаты действий отображаются здесь.</p>
              <p className="text-sm mt-2">Нажмите «Выбрать папку» для анализа проекта или введите сообщение ниже.</p>
            </div>
          ) : (
            <div className="space-y-4">
              {messages.map((m, i) => (
                <div
                  key={i}
                  className={`p-4 rounded-xl ${m.role === 'user' ? 'bg-primary/10 ml-8' : 'bg-muted/50 mr-8'}`}
                >
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    {m.role === 'user' ? 'Вы' : 'Агент'}
                  </div>
                  {'text' in m && <div className="text-sm">{m.text}</div>}
                  {'report' in m && m.report && (
                    <ReportBlock report={m.report} error={(m as Message & { error?: string }).error} />
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="p-4 border-t">
        <div className="max-w-2xl mx-auto flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder="Сообщение или путь к папке для анализа..."
            className="flex-1 px-4 py-2.5 border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary"
          />
          <button
            onClick={handleSend}
            className="px-4 py-2.5 bg-primary text-primary-foreground rounded-lg font-medium hover:bg-primary/90"
          >
            Отправить
          </button>
        </div>
      </div>
    </div>
  );
}

function ReportBlock({ report, error }: { report: AnalyzeReport; error?: string }) {
  if (error) {
    return <div className="text-sm text-destructive">Ошибка: {error}</div>;
  }
  const r = report as AnalyzeReport;
  return (
    <div className="text-sm space-y-3">
      <p className="font-medium">{r.narrative || r.path}</p>
      {r.findings?.length > 0 && (
        <div>
          <p className="text-xs font-semibold text-muted-foreground mb-1">Находки</p>
          <ul className="list-disc list-inside space-y-0.5">
            {r.findings.slice(0, 10).map((f, i) => (
              <li key={i}>
                <span className={f.severity === 'high' ? 'text-destructive' : ''}>{f.title}</span>
                {f.details && ` — ${f.details}`}
              </li>
            ))}
          </ul>
        </div>
      )}
      {r.recommendations?.length > 0 && (
        <div>
          <p className="text-xs font-semibold text-muted-foreground mb-1">Рекомендации</p>
          <ul className="list-disc list-inside space-y-0.5">
            {r.recommendations.slice(0, 10).map((rec, i) => (
              <li key={i}>
                <span className="font-medium">{rec.title}</span>
                {rec.details && ` — ${rec.details}`}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
