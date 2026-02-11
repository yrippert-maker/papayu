import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { FileText, ArrowLeft, CheckCircle2, XCircle, Clock, Filter, Search, Activity, Info, Trash2 } from 'lucide-react';
import { useAppStore } from '../store/app-store';

export function AuditLogger() {
  const navigate = useNavigate();
  const auditEvents = useAppStore((s) => s.auditEvents);
  const clearAuditEvents = useAppStore((s) => s.clearAuditEvents);
  const lastReport = useAppStore((s) => s.lastReport);
  const [filter, setFilter] = useState<{ type?: string; actor?: string }>({});

  const hasData = auditEvents.length > 0;

  const getEventIcon = (event: string) => {
    if (event.includes('analyz')) return Search;
    if (event.includes('appl')) return CheckCircle2;
    if (event.includes('fail') || event.includes('error')) return XCircle;
    return Activity;
  };

  const eventTypes = [...new Set(auditEvents.map((e) => e.event))];
  const actors = [...new Set(auditEvents.map((e) => e.actor))];

  const filtered = auditEvents.filter((e) => {
    if (filter.type && e.event !== filter.type) return false;
    if (filter.actor && e.actor !== filter.actor) return false;
    return true;
  });

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-purple-50/30 dark:to-purple-950/10">
      <button
        onClick={() => navigate(lastReport ? '/control-panel' : '/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-purple-500/20 to-purple-600/10 ring-2 ring-purple-500/20">
            <FileText className="w-8 h-8 text-purple-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Журнал аудита</h1>
            <p className="text-lg md:text-xl text-muted-foreground font-light mt-2">
              Реальные действия в текущей сессии
            </p>
          </div>
        </div>
      </div>

      {!hasData ? (
        <div className="bg-card/80 backdrop-blur-sm p-8 rounded-2xl border text-center">
          <Info className="w-12 h-12 mx-auto mb-4 text-muted-foreground" />
          <p className="text-lg text-muted-foreground mb-4">Журнал пуст — действия появятся после анализа проекта</p>
          <button
            onClick={() => navigate('/')}
            className="px-4 py-2 bg-primary text-primary-foreground rounded-lg font-medium hover:bg-primary/90"
          >
            Перейти к анализу
          </button>
        </div>
      ) : (
        <>
          <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border mb-8 animate-fade-in-up">
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-3">
                <Filter className="w-6 h-6 text-primary" />
                <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Фильтры</h2>
              </div>
              <button
                onClick={clearAuditEvents}
                className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium hover:bg-muted text-muted-foreground"
              >
                <Trash2 className="w-4 h-4" />
                Очистить журнал
              </button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div>
                <label className="block text-sm font-medium mb-2">Тип события</label>
                <select
                  className="w-full px-4 py-2.5 border-2 rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary"
                  value={filter.type || ''}
                  onChange={(e) => setFilter({ ...filter, type: e.target.value || undefined })}
                >
                  <option value="">Все события</option>
                  {eventTypes.map((t) => (
                    <option key={t} value={t}>{t}</option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium mb-2">Агент</label>
                <select
                  className="w-full px-4 py-2.5 border-2 rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary"
                  value={filter.actor || ''}
                  onChange={(e) => setFilter({ ...filter, actor: e.target.value || undefined })}
                >
                  <option value="">Все агенты</option>
                  {actors.map((a) => (
                    <option key={a} value={a}>{a}</option>
                  ))}
                </select>
              </div>
            </div>
          </div>

          <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border animate-fade-in-up" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-3">
                <Activity className="w-6 h-6 text-primary" />
                <h2 className="text-2xl md:text-3xl font-bold tracking-tight">События</h2>
              </div>
              <span className="text-sm text-muted-foreground">Показано: {filtered.length} из {auditEvents.length}</span>
            </div>
            {filtered.length === 0 ? (
              <div className="text-center py-16">
                <FileText className="w-8 h-8 text-muted-foreground mx-auto mb-4" />
                <p className="text-muted-foreground text-lg">Нет событий для текущего фильтра</p>
              </div>
            ) : (
              <div className="space-y-3">
                {filtered.map((event) => {
                  const Icon = getEventIcon(event.event);
                  const isSuccess = event.result === 'success';
                  return (
                    <div key={event.id} className="p-5 rounded-xl border-2 bg-card/50 hover:shadow-lg transition-all-smooth">
                      <div className="flex items-start gap-4">
                        <div className={`p-3 rounded-xl ${isSuccess ? 'bg-green-100 dark:bg-green-900/20' : 'bg-red-100 dark:bg-red-900/20'}`}>
                          <Icon className={`w-5 h-5 ${isSuccess ? 'text-green-600' : 'text-red-600'}`} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-3 mb-2 flex-wrap">
                            <span className="font-semibold">{event.event}</span>
                            <span className={`status-badge ${isSuccess ? 'status-active' : 'status-inactive'}`}>
                              {isSuccess ? <><CheckCircle2 className="w-4 h-4" /><span>Успех</span></> : <><XCircle className="w-4 h-4" /><span>Ошибка</span></>}
                            </span>
                            <span className="px-3 py-1 rounded-full text-xs font-medium bg-muted">{event.actor}</span>
                          </div>
                          <div className="flex items-center gap-2 mb-3 text-sm text-muted-foreground">
                            <Clock className="w-4 h-4" />
                            <span className="font-mono">{new Date(event.timestamp).toLocaleString('ru-RU')}</span>
                          </div>
                          {event.metadata && Object.keys(event.metadata).length > 0 && (
                            <div className="mt-3 p-3 rounded-lg bg-muted/50 border">
                              <pre className="text-xs font-mono text-muted-foreground break-all">{JSON.stringify(event.metadata, null, 2)}</pre>
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
