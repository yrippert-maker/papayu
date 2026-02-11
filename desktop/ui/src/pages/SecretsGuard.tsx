import { useNavigate } from 'react-router-dom';
import { Lock, ArrowLeft, CheckCircle2, AlertTriangle, Shield, Key, Info } from 'lucide-react';
import { useAppStore } from '../store/app-store';

export function SecretsGuard() {
  const navigate = useNavigate();
  const lastReport = useAppStore((s) => s.lastReport);

  const hasData = !!lastReport;
  const signals = lastReport?.signals ?? [];
  const findings = lastReport?.findings ?? [];

  // Extract security-related findings
  const secretFindings = findings.filter(
    (f) =>
      f.title.toLowerCase().includes('.env') ||
      f.title.toLowerCase().includes('secret') ||
      f.title.toLowerCase().includes('gitignore') ||
      f.title.toLowerCase().includes('key') ||
      f.title.toLowerCase().includes('token') ||
      f.title.toLowerCase().includes('password')
  );

  const securitySignals = signals.filter((s) => s.category === 'security');
  const allSecurityIssues = [...secretFindings, ...securitySignals.map((s) => ({ severity: s.level, title: s.message, details: '' }))];

  const criticalCount = allSecurityIssues.filter((i) => i.severity === 'high').length;
  const warnCount = allSecurityIssues.filter((i) => i.severity === 'warn').length;
  const infoCount = allSecurityIssues.filter((i) => i.severity === 'info').length;

  const isClean = allSecurityIssues.length === 0;

  const statCards = [
    { label: 'Всего проблем', value: allSecurityIssues.length, color: 'from-emerald-500/10 to-emerald-600/5 text-emerald-700' },
    { label: 'Критичных', value: criticalCount, color: 'from-red-500/10 to-red-600/5 text-red-700' },
    { label: 'Предупреждений', value: warnCount, color: 'from-orange-500/10 to-orange-600/5 text-orange-700' },
    { label: 'Информация', value: infoCount, color: 'from-blue-500/10 to-blue-600/5 text-blue-700' },
  ];

  const getSeverityConfig = (severity: string) => {
    const map: Record<string, { label: string; bg: string; text: string }> = {
      high: { label: 'Критично', bg: 'bg-red-50 dark:bg-red-900/20', text: 'text-red-700 dark:text-red-400' },
      warn: { label: 'Предупреждение', bg: 'bg-orange-50 dark:bg-orange-900/20', text: 'text-orange-700 dark:text-orange-400' },
      info: { label: 'Информация', bg: 'bg-blue-50 dark:bg-blue-900/20', text: 'text-blue-700 dark:text-blue-400' },
    };
    return map[severity] || map.info;
  };

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-emerald-50/30 dark:to-emerald-950/10">
      <button
        onClick={() => navigate(hasData ? '/control-panel' : '/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-emerald-500/20 to-emerald-600/10 ring-2 ring-emerald-500/20">
            <Lock className="w-8 h-8 text-emerald-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Защита секретов</h1>
            <p className="text-lg md:text-xl text-muted-foreground font-light mt-2">
              {hasData ? `Проект: ${lastReport.path}` : 'Сначала запустите анализ проекта'}
            </p>
          </div>
        </div>
      </div>

      {!hasData ? (
        <div className="bg-card/80 backdrop-blur-sm p-8 rounded-2xl border text-center">
          <Info className="w-12 h-12 mx-auto mb-4 text-muted-foreground" />
          <p className="text-lg text-muted-foreground mb-4">Нет данных для отображения</p>
          <button
            onClick={() => navigate('/')}
            className="px-4 py-2 bg-primary text-primary-foreground rounded-lg font-medium hover:bg-primary/90"
          >
            Перейти к анализу
          </button>
        </div>
      ) : (
        <>
          <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border-2 border-emerald-200/50 mb-8 animate-fade-in-up">
            <div className="flex items-center justify-between flex-wrap gap-4">
              <div className="flex items-center gap-4">
                <div className={isClean ? 'p-3 rounded-xl bg-green-100 dark:bg-green-900/20' : 'p-3 rounded-xl bg-red-100 dark:bg-red-900/20'}>
                  {isClean ? <CheckCircle2 className="w-6 h-6 text-green-600" /> : <AlertTriangle className="w-6 h-6 text-red-600" />}
                </div>
                <div>
                  <h2 className="text-xl font-semibold mb-1">Статус</h2>
                  <p className="text-muted-foreground">
                    {isClean ? 'Утечек секретов не обнаружено' : `Обнаружено ${allSecurityIssues.length} потенциальных проблем`}
                  </p>
                </div>
              </div>
              <div className={`status-badge ${isClean ? 'status-active' : 'status-inactive'}`}>
                {isClean ? (
                  <><CheckCircle2 className="w-4 h-4" /><span>Чисто</span></>
                ) : (
                  <><AlertTriangle className="w-4 h-4" /><span>Есть проблемы</span></>
                )}
              </div>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-4 md:gap-6 mb-8 animate-fade-in-up" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
            {statCards.map((stat, i) => (
              <div key={i} className={`bg-card/80 backdrop-blur-sm p-6 rounded-xl border-2 bg-gradient-to-br ${stat.color} transition-all-smooth hover:shadow-lg`}>
                <div className="text-3xl md:text-4xl font-bold mb-2">{stat.value}</div>
                <div className="text-sm text-muted-foreground">{stat.label}</div>
              </div>
            ))}
          </div>

          {allSecurityIssues.length > 0 && (
            <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border animate-fade-in-up" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
              <div className="flex items-center gap-3 mb-6">
                <Key className="w-6 h-6 text-primary" />
                <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Обнаруженные проблемы</h2>
              </div>
              <div className="space-y-4">
                {allSecurityIssues.map((issue, i) => {
                  const cfg = getSeverityConfig(issue.severity);
                  return (
                    <div key={i} className="p-5 rounded-xl border-2 bg-card/50 hover:shadow-lg transition-all-smooth">
                      <div className="flex items-center gap-3 mb-2">
                        <div className={`p-2 rounded-lg ${cfg.bg}`}>
                          <Shield className={`w-5 h-5 ${cfg.text}`} />
                        </div>
                        <div>
                          <div className="font-semibold">{issue.title}</div>
                          <span className={`text-xs font-medium px-2 py-0.5 rounded ${cfg.bg} ${cfg.text}`}>{cfg.label}</span>
                        </div>
                      </div>
                      {issue.details && <div className="text-sm text-muted-foreground mt-2">{issue.details}</div>}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {isClean && (
            <div className="bg-card/80 backdrop-blur-sm p-8 rounded-2xl border text-center animate-fade-in-up" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
              <CheckCircle2 className="w-12 h-12 mx-auto mb-4 text-green-600" />
              <p className="text-lg font-medium text-green-700 dark:text-green-400">Проект чист — утечек секретов не обнаружено</p>
              <p className="text-sm text-muted-foreground mt-2">
                Рекомендуем регулярно повторять анализ при изменениях в проекте
              </p>
            </div>
          )}
        </>
      )}
    </div>
  );
}
