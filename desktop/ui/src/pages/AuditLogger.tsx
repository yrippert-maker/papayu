import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { FileText, ArrowLeft, CheckCircle2, XCircle, Clock, Filter, Search, Activity, Lock } from 'lucide-react';

interface AuditEvent {
  id: string;
  event: string;
  actor: string;
  timestamp: string;
  result?: 'success' | 'failure';
  metadata?: Record<string, unknown>;
}

export function AuditLogger() {
  const navigate = useNavigate();
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<{ type?: string; actor?: string }>({});

  useEffect(() => {
    const t = setTimeout(() => {
      setEvents([
        { id: '1', event: 'command_executed', actor: 'command_router', timestamp: new Date().toISOString(), result: 'success', metadata: { tool: 'fs.read', path: './src/App.tsx' } },
        { id: '2', event: 'policy_denial', actor: 'policy_engine', timestamp: new Date(Date.now() - 3600000).toISOString(), result: 'failure', metadata: { reason: 'tool_not_in_allowlist', tool: 'shell.exec' } },
        { id: '3', event: 'secret_detected', actor: 'secrets_guard', timestamp: new Date(Date.now() - 7200000).toISOString(), result: 'success', metadata: { violationCount: 1, type: 'api_key' } },
      ]);
      setLoading(false);
    }, 500);
    return () => clearTimeout(t);
  }, []);

  const getEventIcon = (event: string) => {
    if (event.includes('command')) return Activity;
    if (event.includes('policy')) return XCircle;
    if (event.includes('secret')) return Lock;
    return FileText;
  };

  const filtered = events.filter((e) => {
    if (filter.type && e.event !== filter.type) return false;
    if (filter.actor && e.actor !== filter.actor) return false;
    return true;
  });

  if (loading) {
    return (
      <div className="p-8 md:p-12">
        <div className="animate-pulse space-y-6">
          <div className="h-10 bg-muted rounded w-1/3 mb-6" />
          <div className="h-48 bg-muted rounded" />
          <div className="space-y-3">
            {[1, 2, 3].map((i) => <div key={i} className="h-24 bg-muted rounded" />)}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-purple-50/30 dark:to-purple-950/10">
      <button
        onClick={() => navigate('/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад к панели управления
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-purple-500/20 to-purple-600/10 ring-2 ring-purple-500/20">
            <FileText className="w-8 h-8 text-purple-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Журнал аудита</h1>
            <p className="text-lg md:text-xl text-muted-foreground font-light mt-2">Просмотр и анализ всех действий в системе</p>
          </div>
        </div>
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border mb-8 animate-fade-in-up">
        <div className="flex items-center gap-3 mb-6">
          <Filter className="w-6 h-6 text-primary" />
          <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Фильтры</h2>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="block text-sm font-medium mb-2">Тип события</label>
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <select
                className="w-full pl-10 pr-4 py-2.5 border-2 rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary"
                value={filter.type || ''}
                onChange={(e) => setFilter({ ...filter, type: e.target.value })}
              >
                <option value="">Все события</option>
                <option value="command_executed">Выполнение команды</option>
                <option value="policy_denial">Отклонение политики</option>
                <option value="secret_detected">Обнаружение секрета</option>
              </select>
            </div>
          </div>
          <div>
            <label className="block text-sm font-medium mb-2">Агент</label>
            <select
              className="w-full px-4 py-2.5 border-2 rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary"
              value={filter.actor || ''}
              onChange={(e) => setFilter({ ...filter, actor: e.target.value })}
            >
              <option value="">Все агенты</option>
              <option value="command_router">Command Router</option>
              <option value="policy_engine">Policy Engine</option>
              <option value="secrets_guard">Secrets Guard</option>
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
          <span className="text-sm text-muted-foreground">Всего: {filtered.length}</span>
        </div>
        {filtered.length === 0 ? (
          <div className="text-center py-16">
            <FileText className="w-8 h-8 text-muted-foreground mx-auto mb-4" />
            <p className="text-muted-foreground text-lg">Нет событий для отображения</p>
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
    </div>
  );
}
