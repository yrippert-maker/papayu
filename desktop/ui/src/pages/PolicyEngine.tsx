import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Shield, ArrowLeft, CheckCircle2, XCircle, AlertTriangle, Clock, FileText } from 'lucide-react';

export function PolicyEngine() {
  const navigate = useNavigate();
  const [status] = useState<'active' | 'inactive'>('active');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const t = setTimeout(() => setLoading(false), 500);
    return () => clearTimeout(t);
  }, []);

  const rules = [
    { title: 'Security over convenience', description: 'Безопасность важнее удобства', icon: Shield, color: 'blue' },
    { title: 'Policy Engine supremacy', description: 'Движок политик имеет приоритет над всеми модулями', icon: Shield, color: 'purple' },
    { title: 'Desktop Core authority', description: 'Desktop Core имеет финальную власть над Web слоем', icon: Shield, color: 'emerald' },
    { title: 'Mandatory audit logging', description: 'Все действия должны логироваться', icon: FileText, color: 'orange' },
  ];

  const denials = [
    { timestamp: new Date().toISOString(), reason: "Запрос отклонён: инструмент 'shell.exec' не в allowlist", tool: 'shell.exec' },
    { timestamp: new Date(Date.now() - 3600000).toISOString(), reason: "Запрос отклонён: путь '/etc/passwd' не разрешён", tool: 'fs.read' },
  ];

  const colorClasses: Record<string, string> = {
    blue: 'from-blue-500/10 to-blue-600/5 border-blue-200/50 text-blue-700 dark:text-blue-400',
    purple: 'from-purple-500/10 to-purple-600/5 border-purple-200/50 text-purple-700 dark:text-purple-400',
    emerald: 'from-emerald-500/10 to-emerald-600/5 border-emerald-200/50 text-emerald-700 dark:text-emerald-400',
    orange: 'from-orange-500/10 to-orange-600/5 border-orange-200/50 text-orange-700 dark:text-orange-400',
  };

  if (loading) {
    return (
      <div className="p-8 md:p-12">
        <div className="animate-pulse space-y-6">
          <div className="h-10 bg-muted rounded w-1/3 mb-6" />
          <div className="h-32 bg-muted rounded" />
          <div className="h-64 bg-muted rounded" />
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-blue-50/30 dark:to-blue-950/10">
      <button
        onClick={() => navigate('/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад к панели управления
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-blue-500/20 to-blue-600/10 ring-2 ring-blue-500/20">
            <Shield className="w-8 h-8 text-blue-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Движок политик</h1>
            <p className="text-lg md:text-xl text-muted-foreground font-light mt-2">Управление правилами безопасности</p>
          </div>
        </div>
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border-2 border-blue-200/50 mb-8 animate-fade-in-up">
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div className="flex items-center gap-4">
            <div className={status === 'active' ? 'p-3 rounded-xl bg-green-100 dark:bg-green-900/20' : 'p-3 rounded-xl bg-red-100 dark:bg-red-900/20'}>
              {status === 'active' ? <CheckCircle2 className="w-6 h-6 text-green-600" /> : <XCircle className="w-6 h-6 text-red-600" />}
            </div>
            <div>
              <h2 className="text-xl font-semibold mb-1">Статус системы</h2>
              <p className="text-muted-foreground">{status === 'active' ? 'Движок политик активен' : 'Движок политик неактивен'}</p>
            </div>
          </div>
          <div className={`status-badge ${status === 'active' ? 'status-active' : 'status-inactive'}`}>
            {status === 'active' ? <><CheckCircle2 className="w-4 h-4" /><span>Активен</span></> : <><XCircle className="w-4 h-4" /><span>Неактивен</span></>}
          </div>
        </div>
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border mb-8 animate-fade-in-up" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
        <div className="flex items-center gap-3 mb-6">
          <Shield className="w-6 h-6 text-primary" />
          <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Правила безопасности</h2>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {rules.map((rule, index) => {
            const Icon = rule.icon;
            const cls = colorClasses[rule.color] || colorClasses.blue;
            return (
              <div key={index} className={`p-5 rounded-xl border-2 bg-gradient-to-br ${cls} transition-all-smooth hover:shadow-md`}>
                <div className="flex items-start gap-3">
                  <div className="p-2 rounded-lg bg-white/20 dark:bg-black/20">
                    <Icon className="w-5 h-5" />
                  </div>
                  <div className="flex-1">
                    <div className="font-semibold mb-1">{rule.title}</div>
                    <div className="text-sm opacity-80">{rule.description}</div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border animate-fade-in-up" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
        <div className="flex items-center gap-3 mb-6">
          <AlertTriangle className="w-6 h-6 text-destructive" />
          <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Журнал блокировок</h2>
        </div>
        <div className="space-y-3">
          {denials.map((d, i) => (
            <div key={i} className="p-4 rounded-xl border-2 border-destructive/20 bg-destructive/5">
              <div className="flex items-start gap-3">
                <XCircle className="w-5 h-5 text-destructive mt-0.5 flex-shrink-0" />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-2">
                    <Clock className="w-4 h-4 text-muted-foreground" />
                    <span className="font-mono text-xs text-muted-foreground">{new Date(d.timestamp).toLocaleString('ru-RU')}</span>
                  </div>
                  <div className="text-sm font-medium">{d.reason}</div>
                  {d.tool && <div className="mt-2 inline-block px-2 py-1 rounded-md bg-muted text-xs font-mono">{d.tool}</div>}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
