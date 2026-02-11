import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { ROUTES } from '../config/routes';
import { eventBus, Events } from '../lib/event-bus';
import { useAppStore } from '../store/app-store';
import { animateCardsStagger, animateFadeInUp } from '../lib/anime-utils';
import { Shield, FileText, Lock, CheckCircle2, AlertTriangle, ArrowRight, Sparkles, Info } from 'lucide-react';

export function Dashboard() {
  const headerRef = useRef<HTMLDivElement>(null);
  const cardsRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const lastReport = useAppStore((s) => s.lastReport);
  const auditEvents = useAppStore((s) => s.auditEvents);
  const addAuditEvent = useAppStore((s) => s.addAuditEvent);

  const hasData = !!lastReport;
  const findings = lastReport?.findings ?? [];
  const signals = lastReport?.signals ?? [];
  const highFindings = findings.filter((f) => f.severity === 'high');
  const securitySignals = signals.filter((s) => s.category === 'security');
  const secretFindings = findings.filter(
    (f) => f.title.toLowerCase().includes('.env') || f.title.toLowerCase().includes('secret') || f.title.toLowerCase().includes('gitignore')
  );

  const policyStatus = hasData && highFindings.length === 0 && securitySignals.filter((s) => s.level === 'high').length === 0;
  const secretsStatus = hasData && secretFindings.length === 0;

  const handleCardClick = (path: string) => {
    try {
      eventBus.emit(Events.NAVIGATE, { path });
      addAuditEvent({ id: `nav-${Date.now()}`, event: 'navigation', timestamp: new Date().toISOString(), actor: 'user' });
    } catch (_) {}
    navigate(path);
  };

  useEffect(() => {
    if (headerRef.current) animateFadeInUp(headerRef.current, { duration: 600 });
  }, []);
  useEffect(() => {
    if (cardsRef.current) animateCardsStagger(cardsRef.current.querySelectorAll('.card-item-anime'));
  }, []);

  const cards = [
    {
      path: ROUTES.POLICY_ENGINE.path,
      title: 'Политики безопасности',
      description: hasData
        ? (policyStatus ? 'Критичных проблем не обнаружено' : `Обнаружено проблем: ${highFindings.length}`)
        : 'Запустите анализ проекта',
      icon: Shield,
      isOk: policyStatus,
      gradient: 'from-blue-500/10 to-blue-600/5',
      iconColor: 'text-blue-600',
      borderColor: 'border-blue-200/50',
    },
    {
      path: ROUTES.AUDIT_LOGGER.path,
      title: 'Журнал аудита',
      description: auditEvents.length > 0 ? `Записей: ${auditEvents.length}` : 'Журнал пуст',
      icon: FileText,
      isOk: true,
      gradient: 'from-purple-500/10 to-purple-600/5',
      iconColor: 'text-purple-600',
      borderColor: 'border-purple-200/50',
    },
    {
      path: ROUTES.SECRETS_GUARD.path,
      title: 'Защита секретов',
      description: hasData
        ? (secretsStatus ? 'Утечек не обнаружено' : `Потенциальных проблем: ${secretFindings.length}`)
        : 'Запустите анализ проекта',
      icon: Lock,
      isOk: secretsStatus,
      gradient: 'from-emerald-500/10 to-emerald-600/5',
      iconColor: 'text-emerald-600',
      borderColor: 'border-emerald-200/50',
    },
  ];

  return (
    <div className="min-h-screen p-8 md:p-12 lg:p-16 bg-gradient-to-br from-background via-background to-muted/20">
      <div ref={headerRef} className="mb-12 md:mb-16">
        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 rounded-lg bg-primary/10">
            <Sparkles className="w-5 h-5 text-primary" />
          </div>
          <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight text-balance">Безопасность</h1>
        </div>
        <p className="text-lg md:text-xl text-muted-foreground font-light max-w-2xl">
          {hasData ? `Проект: ${lastReport.path}` : 'Сначала запустите анализ на главной странице'}
        </p>
      </div>

      {!hasData && (
        <div className="bg-card/50 backdrop-blur-sm border rounded-xl p-6 md:p-8 mb-8 text-center animate-fade-in">
          <Info className="w-10 h-10 mx-auto mb-4 text-muted-foreground" />
          <p className="text-muted-foreground mb-4">Данные безопасности появятся после анализа проекта</p>
          <button
            onClick={() => navigate('/')}
            className="px-4 py-2 bg-primary text-primary-foreground rounded-lg font-medium hover:bg-primary/90"
          >
            Перейти к анализу
          </button>
        </div>
      )}

      <div ref={cardsRef} className="grid grid-cols-1 md:grid-cols-3 gap-6 md:gap-8">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div
              key={card.path}
              role="button"
              tabIndex={0}
              onClick={() => handleCardClick(card.path)}
              onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && handleCardClick(card.path)}
              className={`card-item-anime group relative bg-card/80 backdrop-blur-sm p-8 rounded-2xl border-2 cursor-pointer hover-lift transition-all-smooth ${card.borderColor} hover:border-primary/50 hover:shadow-primary-lg focus:outline-none focus:ring-2 focus:ring-primary`}
            >
              <div className="relative z-10">
                <div className="flex items-start justify-between mb-6">
                  <div className={`p-3 rounded-xl bg-gradient-to-br ${card.gradient}`}>
                    <Icon className={`w-6 h-6 ${card.iconColor}`} />
                  </div>
                  {hasData && (
                    <div className={`status-badge ${card.isOk ? 'status-active' : 'status-inactive'}`}>
                      {card.isOk ? <CheckCircle2 className="w-4 h-4" /> : <AlertTriangle className="w-4 h-4" />}
                      <span>{card.isOk ? 'OK' : 'Внимание'}</span>
                    </div>
                  )}
                </div>
                <h2 className="text-2xl md:text-3xl font-bold mb-3 tracking-tight group-hover:text-primary transition-colors">
                  {card.title}
                </h2>
                <p className="text-base text-muted-foreground mb-6 min-h-[3rem]">{card.description}</p>
                <div className="flex items-center gap-2 text-primary font-semibold group-hover:gap-3 transition-all">
                  <span>Открыть</span>
                  <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {hasData && lastReport.llm_context && (
        <div className="mt-12 md:mt-16 animate-fade-in-up" style={{ animationDelay: '0.4s', animationFillMode: 'both' }}>
          <div className="bg-card/50 backdrop-blur-sm border rounded-xl p-6 md:p-8">
            <div className="flex items-center gap-3 mb-4">
              <FileText className="w-5 h-5 text-muted-foreground" />
              <h3 className="text-lg font-semibold">Сводка анализа</h3>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed mb-3">
              {lastReport.llm_context.concise_summary}
            </p>
            {lastReport.llm_context.key_risks.length > 0 && (
              <div className="mt-3">
                <p className="text-xs font-semibold text-muted-foreground mb-1">Ключевые риски:</p>
                <ul className="text-sm text-muted-foreground space-y-1">
                  {lastReport.llm_context.key_risks.map((r, i) => (
                    <li key={i} className="flex items-start gap-2">
                      <AlertTriangle className="w-3.5 h-3.5 text-red-500 mt-0.5 flex-shrink-0" />
                      <span>{r}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
