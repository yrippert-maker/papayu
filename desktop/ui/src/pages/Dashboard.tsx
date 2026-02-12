import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { ROUTES } from '../config/routes';
import { eventBus, Events } from '../lib/event-bus';
import { useAppStore } from '../store/app-store';
import { animateCardsStagger, animateFadeInUp } from '../lib/anime-utils';
import {
  Shield, FileText, Lock, CheckCircle2, AlertTriangle, ArrowRight,
  Sparkles, Info, Activity, Code2, FolderOpen, Bug, Brain,
} from 'lucide-react';

function HealthRing({ score, size = 120 }: { score: number; size?: number }) {
  const r = (size - 12) / 2;
  const circ = 2 * Math.PI * r;
  const offset = circ * (1 - score / 100);
  const color = score >= 80 ? '#22c55e' : score >= 50 ? '#eab308' : '#ef4444';
  return (
    <div className="relative" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="currentColor" strokeWidth="8" className="text-muted/20" />
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth="8"
          strokeDasharray={circ} strokeDashoffset={offset} strokeLinecap="round"
          className="transition-all duration-1000 ease-out" />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-3xl font-bold" style={{ color }}>{score}</span>
        <span className="text-xs text-muted-foreground">из 100</span>
      </div>
    </div>
  );
}

function MiniBar({ label, value, max, color }: { label: string; value: number; max: number; color: string }) {
  const pct = max > 0 ? Math.min((value / max) * 100, 100) : 0;
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs">
        <span className="text-muted-foreground">{label}</span>
        <span className="font-medium">{value}</span>
      </div>
      <div className="h-1.5 bg-muted/30 rounded-full overflow-hidden">
        <div className="h-full rounded-full transition-all duration-700" style={{ width: `${pct}%`, backgroundColor: color }} />
      </div>
    </div>
  );
}

function StatCard({ icon: Icon, label, value, sub, color }: {
  icon: typeof Activity; label: string; value: string | number; sub?: string; color: string;
}) {
  return (
    <div className="bg-card/60 backdrop-blur-sm border rounded-xl p-4 space-y-1">
      <div className="flex items-center gap-2 mb-2">
        <Icon className="w-4 h-4" style={{ color }} />
        <span className="text-xs text-muted-foreground">{label}</span>
      </div>
      <div className="text-2xl font-bold">{value}</div>
      {sub && <div className="text-xs text-muted-foreground">{sub}</div>}
    </div>
  );
}

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
  const stats = lastReport?.stats;
  const highFindings = findings.filter((f) => f.severity === 'high');
  const warnFindings = findings.filter((f) => f.severity === 'warn');
  const securitySignals = signals.filter((s) => s.category === 'security');
  const secretFindings = findings.filter(
    (f) => f.title.includes('\u{1F510}') || f.title.toLowerCase().includes('secret') || f.title.toLowerCase().includes('.env')
  );
  const qualityFindings = findings.filter((f) => f.title.includes('\u{1F4DD}') || f.title.includes('\u{1F4CF}'));

  const calcHealth = () => {
    if (!hasData) return 0;
    let score = 100;
    score -= highFindings.length * 15;
    score -= warnFindings.length * 5;
    score -= securitySignals.filter((s) => s.level === 'high').length * 10;
    return Math.max(0, Math.min(100, score));
  };
  const healthScore = calcHealth();

  const policyStatus = hasData && highFindings.length === 0 && securitySignals.filter((s) => s.level === 'high').length === 0;
  const secretsStatus = hasData && secretFindings.length === 0;

  const handleCardClick = (path: string) => {
    try {
      eventBus.emit(Events.NAVIGATE, { path });
      addAuditEvent({ id: `nav-${Date.now()}`, event: 'navigation', timestamp: new Date().toISOString(), actor: 'user' });
    } catch { /* ignored */ }
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
      title: 'Политики',
      description: hasData ? (policyStatus ? 'Критичных проблем нет' : `Проблем: ${highFindings.length}`) : 'Запустите анализ',
      icon: Shield, isOk: policyStatus,
      gradient: 'from-blue-500/10 to-blue-600/5', iconColor: 'text-blue-600', borderColor: 'border-blue-200/50',
    },
    {
      path: ROUTES.AUDIT_LOGGER.path,
      title: 'Журнал аудита',
      description: auditEvents.length > 0 ? `Записей: ${auditEvents.length}` : 'Журнал пуст',
      icon: FileText, isOk: true,
      gradient: 'from-purple-500/10 to-purple-600/5', iconColor: 'text-purple-600', borderColor: 'border-purple-200/50',
    },
    {
      path: ROUTES.SECRETS_GUARD.path,
      title: 'Секреты',
      description: hasData ? (secretsStatus ? 'Утечек нет' : `Проблем: ${secretFindings.length}`) : 'Запустите анализ',
      icon: Lock, isOk: secretsStatus,
      gradient: 'from-emerald-500/10 to-emerald-600/5', iconColor: 'text-emerald-600', borderColor: 'border-emerald-200/50',
    },
    {
      path: ROUTES.LLM_SETTINGS.path,
      title: 'AI Настройки',
      description: 'OpenAI / Anthropic / Ollama',
      icon: Brain, isOk: true,
      gradient: 'from-orange-500/10 to-orange-600/5', iconColor: 'text-orange-600', borderColor: 'border-orange-200/50',
    },
  ];

  return (
    <div className="min-h-screen p-6 md:p-10 lg:p-14 bg-gradient-to-br from-background via-background to-muted/20">
      <div ref={headerRef} className="mb-8 md:mb-12">
        <div className="flex items-center gap-3 mb-3">
          <div className="p-2 rounded-lg bg-primary/10"><Sparkles className="w-5 h-5 text-primary" /></div>
          <h1 className="text-3xl md:text-4xl font-bold tracking-tight">PAPA YU</h1>
        </div>
        <p className="text-base text-muted-foreground font-light max-w-2xl">
          {hasData ? `Проект: ${lastReport.path}` : 'AI-аудитор проектов. Начните с анализа на главной.'}
        </p>
      </div>

      {!hasData && (
        <div className="bg-card/50 backdrop-blur-sm border rounded-xl p-6 mb-8 text-center">
          <Info className="w-10 h-10 mx-auto mb-4 text-muted-foreground" />
          <p className="text-muted-foreground mb-4">Данные появятся после анализа проекта</p>
          <button onClick={() => navigate('/')} className="px-4 py-2 bg-primary text-primary-foreground rounded-lg font-medium hover:bg-primary/90">
            Перейти к анализу
          </button>
        </div>
      )}

      {hasData && (
        <div className="grid grid-cols-1 md:grid-cols-5 gap-6 mb-8">
          <div className="md:col-span-1 bg-card/60 backdrop-blur-sm border rounded-xl p-6 flex flex-col items-center justify-center">
            <HealthRing score={healthScore} />
            <span className="text-sm font-medium mt-2">Здоровье</span>
          </div>
          <div className="md:col-span-4 grid grid-cols-2 md:grid-cols-4 gap-3">
            <StatCard icon={FolderOpen} label="Файлов" value={stats?.file_count ?? 0} sub={`${stats?.dir_count ?? 0} папок`} color="#6366f1" />
            <StatCard icon={Code2} label="Тип" value={lastReport.structure?.project_type ?? '\u2014'} sub={lastReport.structure?.framework ?? ''} color="#8b5cf6" />
            <StatCard icon={Bug} label="Проблемы" value={findings.length} sub={`${highFindings.length} критичных`} color="#ef4444" />
            <StatCard icon={Activity} label="Сигналы" value={signals.length} sub={`${securitySignals.length} security`} color="#f59e0b" />
          </div>
        </div>
      )}

      {hasData && findings.length > 0 && (
        <div className="bg-card/60 backdrop-blur-sm border rounded-xl p-6 mb-8">
          <h3 className="text-sm font-semibold mb-4 text-muted-foreground">Распределение проблем</h3>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <MiniBar label="Безопасность" value={secretFindings.length} max={findings.length} color="#ef4444" />
            <MiniBar label="Уязвимости" value={Math.max(0, highFindings.length - secretFindings.length)} max={findings.length} color="#f59e0b" />
            <MiniBar label="Качество" value={qualityFindings.length} max={findings.length} color="#3b82f6" />
            <MiniBar label="Зависимости" value={findings.filter((f) => f.title.includes('\u{1F4E6}')).length} max={findings.length} color="#8b5cf6" />
          </div>
        </div>
      )}

      <div ref={cardsRef} className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 md:gap-6">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div key={card.path} role="button" tabIndex={0}
              onClick={() => handleCardClick(card.path)}
              onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && handleCardClick(card.path)}
              className={`card-item-anime group relative bg-card/80 backdrop-blur-sm p-6 rounded-2xl border-2 cursor-pointer hover-lift transition-all-smooth ${card.borderColor} hover:border-primary/50 hover:shadow-primary-lg focus:outline-none focus:ring-2 focus:ring-primary`}
            >
              <div className="relative z-10">
                <div className="flex items-start justify-between mb-4">
                  <div className={`p-2.5 rounded-xl bg-gradient-to-br ${card.gradient}`}>
                    <Icon className={`w-5 h-5 ${card.iconColor}`} />
                  </div>
                  {hasData && card.path !== ROUTES.LLM_SETTINGS.path && (
                    <div className={`status-badge ${card.isOk ? 'status-active' : 'status-inactive'}`}>
                      {card.isOk ? <CheckCircle2 className="w-3.5 h-3.5" /> : <AlertTriangle className="w-3.5 h-3.5" />}
                      <span className="text-xs">{card.isOk ? 'OK' : '!'}</span>
                    </div>
                  )}
                </div>
                <h2 className="text-lg font-bold mb-1.5 tracking-tight group-hover:text-primary transition-colors">{card.title}</h2>
                <p className="text-sm text-muted-foreground mb-4">{card.description}</p>
                <div className="flex items-center gap-2 text-primary text-sm font-semibold group-hover:gap-3 transition-all">
                  <span>Открыть</span>
                  <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {hasData && lastReport.llm_context && (
        <div className="mt-8 bg-card/50 backdrop-blur-sm border rounded-xl p-6">
          <div className="flex items-center gap-3 mb-3">
            <FileText className="w-5 h-5 text-muted-foreground" />
            <h3 className="text-sm font-semibold">Сводка анализа</h3>
          </div>
          <p className="text-sm text-muted-foreground leading-relaxed">{lastReport.llm_context.concise_summary}</p>
          {lastReport.llm_context.key_risks.length > 0 && (
            <div className="mt-3 space-y-1">
              {lastReport.llm_context.key_risks.map((r, i) => (
                <div key={i} className="flex items-start gap-2 text-sm text-muted-foreground">
                  <AlertTriangle className="w-3.5 h-3.5 text-red-500 mt-0.5 flex-shrink-0" />
                  <span>{r}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
