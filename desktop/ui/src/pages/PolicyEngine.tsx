import { useNavigate } from 'react-router-dom';
import { Shield, ArrowLeft, CheckCircle2, AlertTriangle, Info } from 'lucide-react';
import { useAppStore } from '../store/app-store';

export function PolicyEngine() {
  const navigate = useNavigate();
  const lastReport = useAppStore((s) => s.lastReport);

  const signals = lastReport?.signals ?? [];
  const findings = lastReport?.findings ?? [];

  const securitySignals = signals.filter((s) => s.category === 'security');
  const highFindings = findings.filter((f) => f.severity === 'high');
  const warnFindings = findings.filter((f) => f.severity === 'warn');

  const hasData = !!lastReport;
  const isSecure = highFindings.length === 0 && securitySignals.filter((s) => s.level === 'high').length === 0;

  const policyRules = [
    {
      title: '.env без .gitignore',
      description: 'Файлы .env должны быть исключены из git',
      check: !findings.some((f) => f.title.toLowerCase().includes('.env') || f.title.toLowerCase().includes('gitignore')),
      color: 'blue',
    },
    {
      title: 'Наличие README',
      description: 'Проект должен содержать README',
      check: !findings.some((f) => f.title.toLowerCase().includes('readme')),
      color: 'purple',
    },
    {
      title: 'Наличие тестов',
      description: 'Проект должен содержать директорию tests/',
      check: !findings.some((f) => f.title.toLowerCase().includes('тест') || f.title.toLowerCase().includes('test')),
      color: 'emerald',
    },
    {
      title: 'Глубина вложенности',
      description: 'Не должна превышать 6 уровней',
      check: !findings.some((f) => f.title.toLowerCase().includes('глубина') || f.title.toLowerCase().includes('вложен')),
      color: 'orange',
    },
  ];

  const colorClasses: Record<string, string> = {
    blue: 'from-blue-500/10 to-blue-600/5 border-blue-200/50 text-blue-700 dark:text-blue-400',
    purple: 'from-purple-500/10 to-purple-600/5 border-purple-200/50 text-purple-700 dark:text-purple-400',
    emerald: 'from-emerald-500/10 to-emerald-600/5 border-emerald-200/50 text-emerald-700 dark:text-emerald-400',
    orange: 'from-orange-500/10 to-orange-600/5 border-orange-200/50 text-orange-700 dark:text-orange-400',
  };

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-blue-50/30 dark:to-blue-950/10">
      <button
        onClick={() => navigate(hasData ? '/control-panel' : '/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад
      </button>

      <div className="mb-10 md:mb-12 animate-fade-in">
        <div className="flex items-center gap-4 mb-4">
          <div className="p-3 rounded-xl bg-gradient-to-br from-blue-500/20 to-blue-600/10 ring-2 ring-blue-500/20">
            <Shield className="w-8 h-8 text-blue-600" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight">Политики безопасности</h1>
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
          <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border-2 border-blue-200/50 mb-8 animate-fade-in-up">
            <div className="flex items-center justify-between flex-wrap gap-4">
              <div className="flex items-center gap-4">
                <div className={isSecure ? 'p-3 rounded-xl bg-green-100 dark:bg-green-900/20' : 'p-3 rounded-xl bg-red-100 dark:bg-red-900/20'}>
                  {isSecure ? <CheckCircle2 className="w-6 h-6 text-green-600" /> : <AlertTriangle className="w-6 h-6 text-red-600" />}
                </div>
                <div>
                  <h2 className="text-xl font-semibold mb-1">Статус</h2>
                  <p className="text-muted-foreground">
                    {isSecure
                      ? 'Критичных проблем безопасности не обнаружено'
                      : `Обнаружено проблем: ${highFindings.length} критичных, ${warnFindings.length} предупреждений`}
                  </p>
                </div>
              </div>
              <div className={`status-badge ${isSecure ? 'status-active' : 'status-inactive'}`}>
                {isSecure ? (
                  <><CheckCircle2 className="w-4 h-4" /><span>Безопасно</span></>
                ) : (
                  <><AlertTriangle className="w-4 h-4" /><span>Есть проблемы</span></>
                )}
              </div>
            </div>
          </div>

          <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border mb-8 animate-fade-in-up" style={{ animationDelay: '0.1s', animationFillMode: 'both' }}>
            <div className="flex items-center gap-3 mb-6">
              <Shield className="w-6 h-6 text-primary" />
              <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Проверки</h2>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {policyRules.map((rule, index) => {
                const cls = colorClasses[rule.color] || colorClasses.blue;
                return (
                  <div key={index} className={`p-5 rounded-xl border-2 bg-gradient-to-br ${cls} transition-all-smooth hover:shadow-md`}>
                    <div className="flex items-start gap-3">
                      <div className="p-2 rounded-lg bg-white/20 dark:bg-black/20">
                        {rule.check ? <CheckCircle2 className="w-5 h-5 text-green-600" /> : <AlertTriangle className="w-5 h-5 text-red-600" />}
                      </div>
                      <div className="flex-1">
                        <div className="font-semibold mb-1">{rule.title}</div>
                        <div className="text-sm opacity-80">{rule.description}</div>
                        <div className={`text-xs font-medium mt-2 ${rule.check ? 'text-green-600' : 'text-red-600'}`}>
                          {rule.check ? '✓ Пройдено' : '✗ Нарушение'}
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {highFindings.length > 0 && (
            <div className="bg-card/80 backdrop-blur-sm p-6 md:p-8 rounded-2xl border animate-fade-in-up" style={{ animationDelay: '0.2s', animationFillMode: 'both' }}>
              <div className="flex items-center gap-3 mb-6">
                <AlertTriangle className="w-6 h-6 text-destructive" />
                <h2 className="text-2xl md:text-3xl font-bold tracking-tight">Критичные проблемы</h2>
              </div>
              <div className="space-y-3">
                {highFindings.map((f, i) => (
                  <div key={i} className="p-4 rounded-xl border-2 border-destructive/20 bg-destructive/5">
                    <div className="font-medium text-sm">{f.title}</div>
                    {f.details && <div className="text-sm text-muted-foreground mt-1">{f.details}</div>}
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
