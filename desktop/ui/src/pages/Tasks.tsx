import { useState, useRef, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import {
  MessageSquare,
  RotateCcw,
  Trash2,
  FolderOpen,
  FolderPlus,
  File,
  Download,
  FileDown,
  User,
  Bot,
  Info,
  RefreshCw,
  GitCompare,
  History,
  X,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { analyzeProject, type AnalyzeReport, type Action, type ApplyResult, type UndoResult, type PreviewResult, type DiffItem } from '../lib/analyze';
import { animateFadeInUp } from '../lib/anime-utils';
import { useAppStore } from '../store/app-store';

type Message =
  | { role: 'user'; text: string }
  | { role: 'system'; text: string }
  | { role: 'assistant'; text: string }
  | { role: 'assistant'; report: AnalyzeReport; error?: string };

type HistoryItem = {
  path: string;
  ts: number;
  projectType?: string;
  risk?: string;
  issueCount?: number;
  summary?: string;
  report: AnalyzeReport;
};

const UNDO_SYSTEM_MESSAGE = 'Последнее действие отменено.';
const HISTORY_MAX = 20;

export function Tasks() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [lastReport, setLastReport] = useState<AnalyzeReport | null>(null);
  const [lastPath, setLastPath] = useState<string | null>(null);
  const [previousReport, setPreviousReport] = useState<AnalyzeReport | null>(null);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [selectedActions, setSelectedActions] = useState<Record<string, boolean>>({});
  const [undoAvailable, setUndoAvailable] = useState(false);
  const [pendingPreview, setPendingPreview] = useState<{
    path: string;
    actions: Action[];
    diffs: DiffItem[];
  } | null>(null);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const messagesListRef = useRef<HTMLDivElement>(null);
  const storeSetLastReport = useAppStore((s) => s.setLastReport);
  const addAuditEvent = useAppStore((s) => s.addAuditEvent);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    if (messages.length === 0) return;
    const t = setTimeout(() => {
      const last = messagesListRef.current?.querySelector('.message-item-anime:last-child');
      if (last) animateFadeInUp(last, { duration: 500 });
    }, 50);
    return () => clearTimeout(t);
  }, [messages.length]);

  useEffect(() => {
    const unlisten = listen<string>('analyze_progress', (e) => {
      if (e.payload) {
        setMessages((prev) => [...prev, { role: 'system', text: e.payload }]);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleClearChat = () => {
    setMessages([]);
  };

  const handleUndo = () => {
    if (messages.length === 0) return;
    setMessages((prev) => {
      const next = [...prev];
      while (next.length > 0) {
        const last = next[next.length - 1];
        next.pop();
        if (last.role === 'user') break;
      }
      next.push({ role: 'system', text: UNDO_SYSTEM_MESSAGE });
      return next;
    });
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

  const runAnalysis = async (pathStr: string) => {
    setIsAnalyzing(true);
    setMessages((prev) => [
      ...prev,
      { role: 'user', text: `Проанализируй проект: ${pathStr}` },
      { role: 'assistant', text: 'Индексирую файлы…' },
    ]);

    try {
      const report = await analyzeProject(pathStr);
      setPreviousReport(lastReport);
      setLastReport(report);
      setLastPath(pathStr);
      storeSetLastReport(report, pathStr);
      addAuditEvent({
        id: `analyze-${Date.now()}`,
        event: 'project_analyzed',
        timestamp: new Date().toISOString(),
        actor: 'analyzer',
        result: 'success',
        metadata: { path: pathStr, projectType: report.structure?.project_type, findings: report.findings?.length ?? 0 },
      });
      const init: Record<string, boolean> = {};
      (report.actions ?? []).forEach((a) => { init[a.id] = true; });
      setSelectedActions(init);
      setUndoAvailable(false);
      setPendingPreview(null);
      setHistory((prev) => {
        const item: HistoryItem = {
          path: report.path ?? pathStr,
          ts: Date.now(),
          projectType: report.structure?.project_type,
          risk: report.project_context?.risk_level,
          issueCount: report.findings?.length ?? 0,
          summary: report.narrative?.slice(0, 80) + (report.narrative?.length > 80 ? '…' : ''),
          report,
        };
        const next = [item, ...prev].slice(0, HISTORY_MAX);
        return next;
      });
      setMessages((prev) => {
        const next = [...prev];
        for (let i = next.length - 1; i >= 0; i--) {
          if (next[i].role === 'assistant' && 'text' in next[i]) {
            next[i] = { role: 'assistant', report };
            break;
          }
        }
        return next;
      });
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setMessages((prev) => {
        const next = [...prev];
        for (let i = next.length - 1; i >= 0; i--) {
          if (next[i].role === 'assistant' && 'text' in next[i]) {
            next[i] = { role: 'assistant', report: {} as AnalyzeReport, error: errMsg };
            break;
          }
        }
        return next;
      });
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handlePickFolderAndAnalyze = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    await runAnalysis(selected);
  };

  const handlePickFileAndAnalyze = async () => {
    const selected = await open({ directory: false, multiple: false });
    if (!selected) return;
    const pathStr = typeof selected === 'string' ? selected : selected[0] ?? '';
    if (!pathStr) return;
    const parentDir = pathStr.replace(/[/\\][^/\\]+$/, '') || pathStr;
    await runAnalysis(parentDir);
  };

  const handlePickFoldersAndAnalyze = async () => {
    const selected = await open({ directory: true, multiple: true });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;
    if (paths.length > 1) {
      setMessages((prev) => [...prev, { role: 'system', text: `Выбрано папок: ${paths.length}. Анализирую первую.` }]);
    }
    await runAnalysis(paths[0]);
  };

  const handleRepeatAnalysis = () => {
    if (lastPath) runAnalysis(lastPath);
  };

  const handleCompareWithPrevious = () => {
    if (!lastReport || !previousReport) return;
    const curr = lastReport.stats;
    const prev = previousReport.stats;
    const diffFiles = curr.file_count - prev.file_count;
    const diffDirs = curr.dir_count - prev.dir_count;
    const text =
      diffFiles === 0 && diffDirs === 0
        ? 'Предыдущий и текущий отчёт совпадают по числу файлов и папок.'
        : `Сравнение с предыдущим анализом:\n\nФайлов: ${prev.file_count} → ${curr.file_count} (${diffFiles >= 0 ? '+' : ''}${diffFiles})\nПапок: ${prev.dir_count} → ${curr.dir_count} (${diffDirs >= 0 ? '+' : ''}${diffDirs})\n\nТип тогда: ${previousReport.structure?.project_type ?? '—'}\nТип сейчас: ${lastReport.structure?.project_type ?? '—'}`;
    setMessages((p) => [...p, { role: 'assistant', text }]);
  };

  const handleCompareWithHistory = (item: HistoryItem) => {
    if (!lastReport) return;
    const curr = lastReport.stats;
    const prev = item.report.stats;
    const diffFiles = curr.file_count - prev.file_count;
    const diffDirs = curr.dir_count - prev.dir_count;
    const text = `Сравнение с историей (${new Date(item.ts).toLocaleString('ru-RU')}):\n\nФайлов: ${prev.file_count} → ${curr.file_count} (${diffFiles >= 0 ? '+' : ''}${diffFiles})\nПапок: ${prev.dir_count} → ${curr.dir_count} (${diffDirs >= 0 ? '+' : ''}${diffDirs})\nПроблем: ${item.issueCount ?? 0} → ${lastReport.findings?.length ?? 0}\n\nТип тогда: ${item.projectType ?? '—'}\nТип сейчас: ${lastReport.structure?.project_type ?? '—'}\nРиск тогда: ${item.risk ?? '—'}\nРиск сейчас: ${lastReport.project_context?.risk_level ?? '—'}`;
    setMessages((p) => [...p, { role: 'assistant', text }]);
    setHistoryOpen(false);
  };

  const handleDownloadReport = (report: AnalyzeReport) => {
    const blob = new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'papa-yu-report.json';
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleDownloadMD = (report: AnalyzeReport) => {
    const md = report.report_md ?? report.narrative ?? '';
    const blob = new Blob([md], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'papa-yu-report.md';
    a.click();
    URL.revokeObjectURL(url);
  };

  const pushSystem = (text: string) => {
    setMessages((p) => [...p, { role: 'system', text }]);
  };

  const pushAssistant = (text: string) => {
    setMessages((p) => [...p, { role: 'assistant', text }]);
  };

  function clip(s: string, n = 1200) {
    if (!s) return '';
    return s.length > n ? s.slice(0, n) + '\n…(обрезано)…' : s;
  }

  function renderPreviewText(diffs: DiffItem[]) {
    const lines: string[] = [];
    lines.push('Вот что изменится:\n\n');
    diffs.forEach((d, i) => {
      lines.push(`${i + 1}. ${d.summary}`);
      if (d.kind === 'create' || d.kind === 'update') {
        if (d.before != null) {
          lines.push(`— До:\n\`\`\`\n${clip(d.before)}\n\`\`\``);
        }
        if (d.after != null) {
          lines.push(`— После:\n\`\`\`\n${clip(d.after)}\n\`\`\``);
        }
      }
      if (d.kind === 'delete' && d.before != null) {
        lines.push(`— Будет удалено содержимое:\n\`\`\`\n${clip(d.before)}\n\`\`\``);
      }
      lines.push('');
    });
    lines.push('Если всё выглядит правильно — нажмите «Применить». Иначе — «Отмена».');
    return lines.join('\n');
  }

  const handlePreview = async (projectPath: string, actions: Action[]) => {
    const selected = actions.filter((a) => selectedActions[a.id]);
    if (!selected.length) return;
    setIsPreviewing(true);
    try {
      const res = await invoke<PreviewResult>('preview_actions', {
        payload: { path: projectPath, actions: selected },
      });
      setIsPreviewing(false);
      if (!res.ok) {
        pushSystem('Не удалось сформировать предпросмотр изменений.');
        return;
      }
      setPendingPreview({ path: projectPath, actions: selected, diffs: res.diffs });
      pushSystem('Подготовил предпросмотр изменений.');
      pushAssistant(renderPreviewText(res.diffs));
    } catch (e) {
      setIsPreviewing(false);
      pushSystem(String(e ?? 'Ошибка предпросмотра.'));
    }
  };

  const handleApplyPending = async () => {
    if (!pendingPreview) return;
    const { path, actions } = pendingPreview;
    try {
      const res = await invoke<ApplyResult>('apply_actions', {
        payload: { path, actions },
      });
      if (res.ok) {
        pushSystem('Изменения применены.');
        setUndoAvailable(true);
        addAuditEvent({
          id: `apply-${Date.now()}`,
          event: 'actions_applied',
          timestamp: new Date().toISOString(),
          actor: 'apply_engine',
          result: 'success',
          metadata: { applied: res.applied, path },
        });
      } else {
        pushSystem(res.error ?? 'Изменения не применены. Откат выполнен.');
        setUndoAvailable(false);
        addAuditEvent({
          id: `apply-fail-${Date.now()}`,
          event: 'actions_apply_failed',
          timestamp: new Date().toISOString(),
          actor: 'apply_engine',
          result: 'failure',
          metadata: { error: res.error, path },
        });
      }
    } catch (e) {
      pushSystem(String(e ?? 'Ошибка применения.'));
      setUndoAvailable(false);
    }
    setPendingPreview(null);
  };

  const handleCancelPending = () => {
    if (!pendingPreview) return;
    setPendingPreview(null);
    pushSystem('Предпросмотр отменён. Ничего не изменено.');
  };

  const handleUndoLast = async (projectPath: string) => {
    try {
      const res = await invoke<UndoResult>('undo_last', { path: projectPath });
      if (res.ok) {
        pushSystem('Откат выполнен.');
        setUndoAvailable(false);
      } else {
        pushSystem(res.error ?? 'Откат недоступен.');
      }
    } catch (e) {
      pushSystem(String(e ?? 'Ошибка отката.'));
    }
  };

  // handleApplyActions removed: Apply goes through Preview → handleApplyPending

  return (
    <div className="min-h-screen flex flex-col bg-background">
      <div className="p-4 border-b flex items-center justify-between flex-wrap gap-2 shrink-0 bg-card/30">
        <img src={`${import.meta.env.BASE_URL}logo-papa-yu.png`} alt="PAPA YU" className="h-8 md:h-9 w-auto object-contain" />
        <div className="flex items-center gap-2 flex-wrap">
          <button
            onClick={handlePickFolderAndAnalyze}
            disabled={isAnalyzing}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border border-primary/50 text-primary text-sm font-medium hover:bg-primary/10 disabled:opacity-50"
            title="Выбрать одну папку проекта"
          >
            <FolderOpen className="w-4 h-4" />
            Выбрать папку
          </button>
          <button
            onClick={handlePickFileAndAnalyze}
            disabled={isAnalyzing}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border border-primary/50 text-primary text-sm font-medium hover:bg-primary/10 disabled:opacity-50"
            title="Выбрать файл — будет проанализирована родительская папка"
          >
            <File className="w-4 h-4" />
            Выбрать файл
          </button>
          <button
            onClick={handlePickFoldersAndAnalyze}
            disabled={isAnalyzing}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border border-primary/50 text-primary text-sm font-medium hover:bg-primary/10 disabled:opacity-50"
            title="Выбрать несколько папок (анализ первой)"
          >
            <FolderPlus className="w-4 h-4" />
            Выбрать папки
          </button>
          {lastPath && (
            <button
              onClick={handleRepeatAnalysis}
              disabled={isAnalyzing}
              className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted disabled:opacity-50"
              title="Повторить анализ последней папки"
            >
              <RefreshCw className="w-4 h-4" />
              Повтори анализ
            </button>
          )}
          {lastReport && previousReport && (
            <button
              onClick={handleCompareWithPrevious}
              className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted"
              title="Сравнить с предыдущим отчётом"
            >
              <GitCompare className="w-4 h-4" />
              Сравнить с предыдущим
            </button>
          )}
          <button
            onClick={() => setHistoryOpen((o) => !o)}
            className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted"
            title="История анализов"
          >
            <History className="w-4 h-4" />
            История
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

      <div ref={containerRef} className="flex-1 overflow-auto">
        <div className="max-w-[900px] mx-auto px-4 py-6">
          <h2 className="text-lg font-semibold text-foreground/90 mb-4">Анализ проекта</h2>
          {messages.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground animate-fade-in">
              <MessageSquare className="w-12 h-12 mx-auto mb-4 opacity-60" />
              <p className="text-base mb-6">Выберите папку проекта для анализа.</p>
              <div className="flex flex-wrap justify-center gap-3 mb-8">
                <button
                  onClick={handlePickFolderAndAnalyze}
                  disabled={isAnalyzing}
                  className="inline-flex items-center gap-2 px-4 py-3 rounded-xl border-2 border-primary/50 text-primary font-medium hover:bg-primary/10 disabled:opacity-50 transition-colors"
                  title="Выбрать одну папку"
                >
                  <FolderOpen className="w-5 h-5" />
                  Выбрать папку
                </button>
                <button
                  onClick={handlePickFileAndAnalyze}
                  disabled={isAnalyzing}
                  className="inline-flex items-center gap-2 px-4 py-3 rounded-xl border-2 border-primary/50 text-primary font-medium hover:bg-primary/10 disabled:opacity-50 transition-colors"
                  title="Анализ по родительской папке выбранного файла"
                >
                  <File className="w-5 h-5" />
                  Выбрать файл
                </button>
                <button
                  onClick={handlePickFoldersAndAnalyze}
                  disabled={isAnalyzing}
                  className="inline-flex items-center gap-2 px-4 py-3 rounded-xl border-2 border-primary/50 text-primary font-medium hover:bg-primary/10 disabled:opacity-50 transition-colors"
                  title="Выбрать несколько папок"
                >
                  <FolderPlus className="w-5 h-5" />
                  Выбрать папки
                </button>
              </div>
              <p className="text-sm">Или введите путь или сообщение ниже.</p>
            </div>
          ) : (
            <div ref={messagesListRef} className="space-y-4">
              {messages.map((m, i) => (
                <div
                  key={i}
                  className={`message-item-anime flex gap-2 ${
                    m.role === 'user' ? 'justify-end' : m.role === 'system' ? 'justify-center' : 'justify-start'
                  }`}
                >
                  {m.role !== 'system' && (
                    <div className="flex-shrink-0 mt-1 w-8 h-8 rounded-full flex items-center justify-center bg-muted/60">
                      {m.role === 'user' ? (
                        <User className="w-4 h-4 text-muted-foreground" />
                      ) : (
                        <Bot className="w-4 h-4 text-muted-foreground" />
                      )}
                    </div>
                  )}
                  {m.role === 'system' && (
                    <div className="flex-shrink-0 mt-1 w-6 h-6 rounded-full flex items-center justify-center bg-muted/50">
                      <Info className="w-3 h-3 text-muted-foreground" />
                    </div>
                  )}
                  <div
                    className={`max-w-[85%] md:max-w-[75%] rounded-2xl px-4 py-3 transition-all-smooth ${
                      m.role === 'user'
                        ? 'bg-primary/90 text-primary-foreground'
                        : m.role === 'system'
                          ? 'bg-muted/60 text-muted-foreground text-sm'
                          : 'bg-card border border-border/60'
                    }`}
                  >
                    {m.role === 'system' && <div className="text-sm">{m.text}</div>}
                    {m.role === 'user' && <div className="font-medium">{m.text}</div>}
                    {m.role === 'assistant' && 'text' in m && (
                      <div className="font-medium whitespace-pre-wrap text-foreground/90">{m.text}</div>
                    )}
                    {m.role === 'assistant' && 'report' in m && m.report && (
                      <ReportBlock
                        report={m.report}
                        error={(m as Message & { error?: string }).error}
                        onDownload={handleDownloadReport}
                        onDownloadMD={handleDownloadMD}
                        isCurrentReport={lastReport === m.report}
                        selectedActions={selectedActions}
                        setSelectedActions={setSelectedActions}
                        undoAvailable={undoAvailable}
                        hasPendingPreview={!!pendingPreview}
                        isPreviewing={isPreviewing}
                        onPreview={handlePreview}
                        onApplyPending={handleApplyPending}
                        onCancelPending={handleCancelPending}
                        onUndo={handleUndoLast}
                      />
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
          <div ref={messagesEndRef} />
          {historyOpen && (
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" onClick={() => setHistoryOpen(false)}>
              <div className="bg-card border rounded-xl shadow-lg max-w-lg w-full max-h-[80vh] overflow-hidden" onClick={(e) => e.stopPropagation()}>
                <div className="p-4 border-b flex items-center justify-between">
                  <h3 className="font-semibold">История анализов</h3>
                  <button onClick={() => setHistoryOpen(false)} className="p-1 rounded hover:bg-muted"><X className="w-5 h-5" /></button>
                </div>
                <div className="p-4 overflow-auto max-h-[60vh] space-y-2">
                  {history.length === 0 ? (
                    <p className="text-sm text-muted-foreground">Пока нет записей. Запустите анализ папки.</p>
                  ) : (
                    history.map((item, i) => (
                      <div key={i} className="p-3 rounded-lg border bg-background/50 text-sm space-y-1">
                        <p className="font-mono text-xs truncate" title={item.path}>{item.path}</p>
                        <p className="text-muted-foreground">{item.projectType ?? '—'} · риск {item.risk ?? '—'} · проблем {item.issueCount ?? 0}</p>
                        <div className="flex gap-2 mt-2">
                          <button onClick={() => { runAnalysis(item.path); setHistoryOpen(false); }} disabled={isAnalyzing} className="text-xs px-2 py-1 rounded border hover:bg-muted disabled:opacity-50">Повтори</button>
                          <button onClick={() => handleCompareWithHistory(item)} className="text-xs px-2 py-1 rounded border hover:bg-muted">Сравнить</button>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {pendingPreview && (
            <PreviewDialog
              diffs={pendingPreview.diffs}
              onApply={handleApplyPending}
              onCancel={handleCancelPending}
            />
          )}
        </div>
      </div>

      <div className="p-4 border-t shrink-0 bg-card/20">
        <div className="max-w-[900px] mx-auto flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder="Сообщение или путь к папке..."
            className="flex-1 px-4 py-2.5 border rounded-xl bg-background focus:outline-none focus:ring-2 focus:ring-primary/50"
          />
          <button
            onClick={handleSend}
            className="px-4 py-2.5 bg-primary text-primary-foreground rounded-xl font-medium hover:bg-primary/90"
          >
            Отправить
          </button>
        </div>
      </div>
    </div>
  );
}

function PreviewDialog({
  diffs,
  onApply,
  onCancel,
}: {
  diffs: DiffItem[];
  onApply: () => void;
  onCancel: () => void;
}) {
  const [expanded, setExpanded] = useState<Record<number, boolean>>({});
  const [tab, setTab] = useState<'preview' | 'verify' | 'write'>('preview');
  const toggle = (i: number) => setExpanded((p) => ({ ...p, [i]: !p[i] }));
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="bg-card border rounded-xl shadow-lg max-w-3xl w-full max-h-[90vh] overflow-hidden flex flex-col">
        <div className="p-4 border-b flex items-center justify-between shrink-0">
          <h3 className="font-semibold">Предпросмотр изменений</h3>
          <button onClick={onCancel} className="p-1 rounded hover:bg-muted" aria-label="Закрыть">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="flex border-b shrink-0">
          {(['preview', 'verify', 'write'] as const).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${tab === t ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'}`}
            >
              {t === 'preview' && 'Превью'}
              {t === 'verify' && 'Проверка'}
              {t === 'write' && 'Написание программы'}
            </button>
          ))}
        </div>
        <div className="p-4 overflow-auto flex-1 min-h-0">
          {tab === 'preview' && (
            <ul className="space-y-3">
              {diffs.map((d, i) => (
                <li key={i} className="rounded-lg border bg-background/50 overflow-hidden">
                  <button
                    type="button"
                    onClick={() => toggle(i)}
                    className="w-full px-3 py-2 text-left text-sm font-medium flex items-center justify-between hover:bg-muted/50"
                  >
                    <span className="truncate">{d.summary}</span>
                    <span className="text-xs text-muted-foreground ml-2">{d.kind}</span>
                  </button>
                  {expanded[i] && (
                    <div className="px-3 pb-3 space-y-2 text-xs font-mono bg-muted/30 border-t">
                      {d.before != null && (
                        <div>
                          <p className="text-muted-foreground mb-1">До:</p>
                          <pre className="whitespace-pre-wrap break-words max-h-40 overflow-auto rounded p-2 bg-background">{d.before}</pre>
                        </div>
                      )}
                      {d.after != null && (
                        <div>
                          <p className="text-muted-foreground mb-1">После:</p>
                          <pre className="whitespace-pre-wrap break-words max-h-40 overflow-auto rounded p-2 bg-background">{d.after}</pre>
                        </div>
                      )}
                      {d.kind === 'delete' && d.before == null && d.after == null && (
                        <p className="text-muted-foreground">Файл или каталог будет удалён.</p>
                      )}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
          {tab === 'verify' && (
            <p className="text-sm text-muted-foreground">Проверка типов и сборки после применения будет доступна в следующей версии.</p>
          )}
          {tab === 'write' && (
            <p className="text-sm text-muted-foreground">Написание и генерация кода по результатам проверки — в разработке.</p>
          )}
        </div>
        <div className="p-4 border-t flex gap-2 justify-end shrink-0">
          <button onClick={onCancel} className="px-4 py-2 rounded-lg border font-medium hover:bg-muted">
            Отмена
          </button>
          <button onClick={onApply} className="px-4 py-2 rounded-lg bg-primary text-primary-foreground font-medium hover:bg-primary/90">
            Применить
          </button>
        </div>
      </div>
    </div>
  );
}

function PriorityBadge({ priority }: { priority: string }) {
  const p = (priority || '').toLowerCase();
  const style = p === 'high' ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400' : p === 'medium' ? 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400' : 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400';
  const label = p === 'high' ? 'high' : p === 'medium' ? 'medium' : 'low';
  return <span className={`text-xs px-1.5 py-0.5 rounded font-medium ${style}`}>{label}</span>;
}

function ReportBlock({
  report,
  error,
  onDownload,
  onDownloadMD,
  isCurrentReport,
  selectedActions,
  setSelectedActions,
  undoAvailable,
  hasPendingPreview,
  isPreviewing,
  onPreview,
  onApplyPending,
  onCancelPending,
  onUndo,
}: {
  report: AnalyzeReport;
  error?: string;
  onDownload: (r: AnalyzeReport) => void;
  onDownloadMD: (r: AnalyzeReport) => void;
  isCurrentReport: boolean;
  selectedActions: Record<string, boolean>;
  setSelectedActions: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
  undoAvailable: boolean;
  hasPendingPreview: boolean;
  isPreviewing: boolean;
  onPreview: (projectPath: string, actions: Action[]) => void;
  onApplyPending: () => void;
  onCancelPending: () => void;
  onUndo: (projectPath: string) => void;
}) {
  if (error) {
    return <div className="text-sm text-destructive">Ошибка: {error}</div>;
  }
  const r = report as AnalyzeReport;
  const hasReport = r && (r.path || r.narrative || r.findings?.length || r.recommendations?.length);
  const ctx = r.project_context;
  const recs = r.recommendations ?? [];
  const actions = r.actions ?? [];
  return (
    <div className="text-sm space-y-4">
      {hasReport && (
        <>
          {r.narrative && (
            <div className="whitespace-pre-wrap text-foreground/90 leading-relaxed">{r.narrative}</div>
          )}
          {ctx && (ctx.stack?.length || ctx.maturity || ctx.risk_level) && (
            <div className="rounded-lg bg-muted/40 px-3 py-2">
              <p className="text-xs font-semibold text-muted-foreground mb-1">Контекст проекта</p>
              <p className="text-foreground/90">
                {[ctx.stack?.join(', '), ctx.maturity, ctx.risk_level && `риск ${ctx.risk_level}`].filter(Boolean).join(' · ')}
              </p>
            </div>
          )}
          {r.structure && (r.structure.project_type || r.structure.architecture) && (
            <div className="rounded-lg bg-muted/40 px-3 py-2 space-y-1">
              {r.structure.project_type && (
                <p>
                  <span className="font-medium text-muted-foreground">Тип проекта:</span>{' '}
                  {r.structure.project_type}
                </p>
              )}
              {r.structure.architecture && (
                <p>
                  <span className="font-medium text-muted-foreground">Архитектура:</span>{' '}
                  {r.structure.architecture}
                </p>
              )}
            </div>
          )}
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
          {recs.length > 0 && (
            <div>
              <p className="text-xs font-semibold text-muted-foreground mb-1">Топ-рекомендации</p>
              <ul className="space-y-1.5">
                {recs.slice(0, 5).map((rec, i) => (
                  <li key={i} className="flex items-start gap-2">
                    <PriorityBadge priority={rec.priority ?? 'medium'} />
                    <span>
                      <span className="font-medium">{rec.title}</span>
                      {(rec.effort || rec.impact) && (
                        <span className="text-muted-foreground text-xs ml-1">
                          (effort: {rec.effort ?? '—'}, impact: {rec.impact ?? '—'})
                        </span>
                      )}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {isCurrentReport && actions.length > 0 && (
            <div className="rounded-lg bg-muted/40 px-3 py-2 space-y-2">
              <p className="text-xs font-semibold text-muted-foreground">Исправления</p>
              <ul className="space-y-1.5">
                {actions.map((a) => (
                  <li key={a.id} className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      id={`action-${a.id}`}
                      checked={selectedActions[a.id] !== false}
                      onChange={() => setSelectedActions((prev) => ({ ...prev, [a.id]: !prev[a.id] }))}
                      className="rounded border-border"
                    />
                    <label htmlFor={`action-${a.id}`} className="cursor-pointer">
                      <span className="font-medium">{a.title}</span>
                      <span className="text-muted-foreground text-xs ml-1">— {a.path}</span>
                    </label>
                  </li>
                ))}
              </ul>
              <div className="flex gap-2 flex-wrap">
                {!hasPendingPreview ? (
                  <button
                    type="button"
                    onClick={() => onPreview(r.path, actions)}
                    disabled={isPreviewing}
                    className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-primary/10 text-primary text-sm font-medium hover:bg-primary/20 disabled:opacity-50"
                  >
                    {isPreviewing ? 'Готовлю предпросмотр…' : 'Предпросмотр изменений'}
                  </button>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={onApplyPending}
                      className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-primary/10 text-primary text-sm font-medium hover:bg-primary/20"
                    >
                      Применить
                    </button>
                    <button
                      type="button"
                      onClick={onCancelPending}
                      className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-background/80 text-sm font-medium hover:bg-muted"
                    >
                      Отмена
                    </button>
                  </>
                )}
                {undoAvailable && (
                  <button
                    type="button"
                    onClick={() => onUndo(r.path)}
                    className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-background/80 text-sm font-medium hover:bg-muted"
                  >
                    Откатить изменения
                  </button>
                )}
              </div>
            </div>
          )}
          <div className="flex gap-2 mt-2">
            <button
              type="button"
              onClick={() => onDownload(r)}
              className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-background/80 text-sm font-medium hover:bg-muted"
            >
              <Download className="w-4 h-4" />
              Скачать JSON
            </button>
            {(r.report_md ?? r.narrative) && (
              <button
                type="button"
                onClick={() => onDownloadMD(r)}
                className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border bg-background/80 text-sm font-medium hover:bg-muted"
              >
                <FileDown className="w-4 h-4" />
                Скачать MD
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
