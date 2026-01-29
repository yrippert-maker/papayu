import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Lock, ArrowLeft, CheckCircle2, XCircle, AlertTriangle, Shield, Key, Eye, EyeOff, TrendingUp, Clock } from 'lucide-react';

interface SecretViolation {
  id: string;
  type: string;
  timestamp: string;
  severity: 'low' | 'medium' | 'high' | 'critical';
  original: string;
  redacted: string;
}

export function SecretsGuard() {
  const navigate = useNavigate();
  const [status] = useState<'active' | 'inactive'>('active');
  const [violations, setViolations] = useState<SecretViolation[]>([]);
  const [loading, setLoading] = useState(true);
  const [stats, setStats] = useState({ total: 0, critical: 0, high: 0, medium: 0, low: 0 });

  useEffect(() => {
    const mock: SecretViolation[] = [
      { id: '1', type: 'api_key', timestamp: new Date().toISOString(), severity: 'high', original: 'api_key=sk_live_***', redacted: 'api_key=***REDACTED***' },
      { id: '2', type: 'aws_key', timestamp: new Date(Date.now() - 3600000).toISOString(), severity: 'critical', original: 'AWS_ACCESS_KEY_ID=AKIA***', redacted: 'AWS_ACCESS_KEY_ID=***REDACTED***' },
      { id: '3', type: 'password', timestamp: new Date(Date.now() - 7200000).toISOString(), severity: 'high', original: 'password=***', redacted: 'password=***REDACTED***' },
    ];
    const t = setTimeout(() => {
      setViolations(mock);
      setStats({
        total: mock.length,
        critical: mock.filter((v) => v.severity === 'critical').length,
        high: mock.filter((v) => v.severity === 'high').length,
        medium: mock.filter((v) => v.severity === 'medium').length,
        low: mock.filter((v) => v.severity === 'low').length,
      });
      setLoading(false);
    }, 500);
    return () => clearTimeout(t);
  }, []);

  const getSeverityConfig = (severity: string) => {
    const map: Record<string, { label: string; bg: string; text: string; border: string; icon: typeof Shield }> = {
      critical: { label: 'Критично', bg: 'bg-red-50 dark:bg-red-900/20', text: 'text-red-700 dark:text-red-400', border: 'border-red-200', icon: AlertTriangle },
      high: { label: 'Высокий', bg: 'bg-orange-50 dark:bg-orange-900/20', text: 'text-orange-700 dark:text-orange-400', border: 'border-orange-200', icon: AlertTriangle },
      medium: { label: 'Средний', bg: 'bg-yellow-50 dark:bg-yellow-900/20', text: 'text-yellow-700 dark:text-yellow-400', border: 'border-yellow-200', icon: Shield },
      low: { label: 'Низкий', bg: 'bg-blue-50 dark:bg-blue-900/20', text: 'text-blue-700 dark:text-blue-400', border: 'border-blue-200', icon: Shield },
    };
    return map[severity] || map.low;
  };

  const statCards = [
    { label: 'Всего обнаружено', value: stats.total, icon: TrendingUp, color: 'from-emerald-500/10 to-emerald-600/5 text-emerald-700' },
    { label: 'Критичных', value: stats.critical, icon: AlertTriangle, color: 'from-red-500/10 to-red-600/5 text-red-700' },
    { label: 'Высокий уровень', value: stats.high, icon: AlertTriangle, color: 'from-orange-500/10 to-orange-600/5 text-orange-700' },
    { label: 'Средний уровень', value: stats.medium, icon: Shield, color: 'from-yellow-500/10 to-yellow-600/5 text-yellow-700' },
    { label: 'Низкий уровень', value: stats.low, icon: Shield, color: 'from-blue-500/10 to-blue-600/5 text-blue-700' },
  ];

  if (loading) {
    return (
      <div className="p-8 md:p-12">
        <div className="animate-pulse space-y-6">
          <div className="h-10 bg-muted rounded w-1/3 mb-6" />
          <div className="h-32 bg-muted rounded" />
          <div className="grid grid-cols-5 gap-4">
            {[1, 2, 3, 4, 5].map((i) => <div key={i} className="h-24 bg-muted rounded" />)}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-emerald-50/30 dark:to-emerald-950/10">
      <button
        onClick={() => navigate('/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад к панели управления
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-emerald-500/20 to-emerald-600/10 ring-2 ring-emerald-500/20">
            <Lock className="w-8 h-8 text-emerald-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Защита секретов</h1>
            <p className="text-lg md:text-xl text-muted-foreground font-light mt-2">Мониторинг и защита от утечек конфиденциальных данных</p>
          </div>
        </div>
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border-2 border-emerald-200/50 mb-8 animate-fade-in-up">
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div className="flex items-center gap-4">
            <div className={status === 'active' ? 'p-3 rounded-xl bg-green-100 dark:bg-green-900/20' : 'p-3 rounded-xl bg-red-100 dark:bg-red-900/20'}>
              {status === 'active' ? <CheckCircle2 className="w-6 h-6 text-green-600" /> : <XCircle className="w-6 h-6 text-red-600" />}
            </div>
            <div>
              <h2 className="text-xl font-semibold mb-1">Статус мониторинга</h2>
              <p className="text-muted-foreground">{status === 'active' ? 'Мониторинг активен' : 'Мониторинг неактивен'}</p>
            </div>
          </div>
          <div className={`status-badge ${status === 'active' ? 'status-active' : 'status-inactive'}`}>
            {status === 'active' ? <><CheckCircle2 className="w-4 h-4" /><span>Активен</span></> : <><XCircle className="w-4 h-4" /><span>Неактивен</span></>}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-5 gap-4 md:gap-6 mb-8 animate-fade-in-up" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
        {statCards.map((stat, i) => {
          const Icon = stat.icon;
          return (
            <div key={i} className={`bg-card/80 backdrop-blur-sm p-6 rounded-xl border-2 bg-gradient-to-br ${stat.color} transition-all-smooth hover:shadow-lg`}>
              <div className="flex items-center justify-between mb-3">
                <div className="p-2 rounded-lg bg-white/20 dark:bg-black/20">
                  <Icon className="w-5 h-5" />
                </div>
              </div>
              <div className="text-3xl md:text-4xl font-bold mb-2">{stat.value}</div>
              <div className="text-sm text-muted-foreground">{stat.label}</div>
            </div>
          );
        })}
      </div>

      <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border animate-fade-in-up" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
        <div className="flex items-center gap-3 mb-6">
          <Key className="w-6 h-6 text-primary" />
          <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Примеры редактирования</h2>
        </div>
        <div className="space-y-4">
          {violations.map((v) => {
            const cfg = getSeverityConfig(v.severity);
            const SeverityIcon = cfg.icon;
            return (
              <div key={v.id} className="p-5 rounded-xl border-2 bg-card/50 hover:shadow-lg transition-all-smooth">
                <div className="flex items-center justify-between mb-4 flex-wrap gap-3">
                  <div className="flex items-center gap-3">
                    <div className={`p-2 rounded-lg ${cfg.bg}`}>
                      <SeverityIcon className={`w-5 h-5 ${cfg.text}`} />
                    </div>
                    <div>
                      <div className="font-semibold capitalize">{v.type.replace('_', ' ')}</div>
                      <span className={`status-badge ${cfg.bg} ${cfg.text} border ${cfg.border}`}>
                        <SeverityIcon className="w-4 h-4" />
                        <span>{cfg.label}</span>
                      </span>
                    </div>
                  </div>
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Clock className="w-4 h-4" />
                    <span className="font-mono">{new Date(v.timestamp).toLocaleString('ru-RU')}</span>
                  </div>
                </div>
                <div className="space-y-3">
                  <div>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2">
                      <Eye className="w-4 h-4" />
                      <span>Оригинал</span>
                    </div>
                    <div className="font-mono text-sm bg-red-50 dark:bg-red-900/20 p-3 rounded-lg border-2 border-red-200 text-red-900 dark:text-red-100">
                      {v.original}
                    </div>
                  </div>
                  <div>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2">
                      <EyeOff className="w-4 h-4" />
                      <span>После редактирования</span>
                    </div>
                    <div className="font-mono text-sm bg-green-50 dark:bg-green-900/20 p-3 rounded-lg border-2 border-green-200 text-green-900 dark:text-green-100">
                      {v.redacted}
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
