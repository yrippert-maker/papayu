import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { ROUTES } from '../../config/routes';
import { eventBus, Events } from '../../lib/event-bus';
import { animateLogo, animateStaggerIn } from '../../lib/anime-utils';
import { LayoutDashboard, ListTodo, FileText, Package, Wallet, Users, MessageSquare, Download } from 'lucide-react';

interface LayoutProps {
  children: ReactNode;
}

const NAV_ICONS: Record<string, typeof LayoutDashboard> = {
  [ROUTES.DASHBOARD.path]: LayoutDashboard,
  [ROUTES.TASKS.path]: ListTodo,
  [ROUTES.CONTROL_PANEL.path]: LayoutDashboard,
  [ROUTES.UPDATES.path]: Download,
  [ROUTES.DIAGNOSTICS.path]: LayoutDashboard,
  [ROUTES.REGLAMENTY.path]: FileText,
  [ROUTES.TMC_ZAKUPKI.path]: Package,
  [ROUTES.FINANCES.path]: Wallet,
  [ROUTES.PERSONNEL.path]: Users,
  [ROUTES.CHAT_AGENT.path]: MessageSquare,
};

async function checkAndInstallUpdate(): Promise<{ ok: boolean; message: string }> {
  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const { relaunch } = await import('@tauri-apps/plugin-process');
    const update = await check();
    if (!update) return { ok: true, message: 'Обновлений нет. У вас актуальная версия.' };
    await update.downloadAndInstall();
    await relaunch();
    return { ok: true, message: 'Установка обновления…' };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    const friendly =
      msg && (msg.includes('fetch') || msg.includes('valid') || msg.includes('signature') || msg.includes('network'))
        ? 'Обновления пока недоступны (сервер или подпись не настроены).'
        : msg || 'Ошибка проверки обновлений.';
    return { ok: false, message: friendly };
  }
}

export function Layout({ children }: LayoutProps) {
  const location = useLocation();
  const logoRef = useRef<HTMLImageElement>(null);
  const navRef = useRef<HTMLDivElement>(null);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);

  const handleCheckUpdate = async () => {
    setIsCheckingUpdate(true);
    setUpdateStatus(null);
    const result = await checkAndInstallUpdate();
    setUpdateStatus(result.message);
    setIsCheckingUpdate(false);
  };

  useEffect(() => {
    if (logoRef.current) animateLogo(logoRef.current);
  }, []);

  useEffect(() => {
    if (!navRef.current) return;
    const links = navRef.current.querySelectorAll('.nav-item-anime');
    if (links.length) animateStaggerIn(links, { staggerDelay: 70, duration: 450 });
  }, [location.pathname]);

  const handleNav = (path: string) => {
    try {
      eventBus.emit(Events.NAVIGATE, { path });
      eventBus.emit(Events.ROUTE_CHANGED, { path });
    } catch (_) {}
  };

  const navItems = [
    ROUTES.TASKS,
    ROUTES.CONTROL_PANEL,
    ROUTES.UPDATES,
    ROUTES.DIAGNOSTICS,
  ].map((r) => ({ path: r.path, name: r.name, icon: NAV_ICONS[r.path] ?? FileText }));

  return (
    <div className="min-h-screen bg-background">
      <nav className="glass-effect border-b sticky top-0 z-50 shadow-sm">
        <div className="container mx-auto px-6 md:px-8 py-4 md:py-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Link
                to={ROUTES.TASKS.path}
                className="group flex items-center gap-2 transition-all-smooth hover:opacity-90"
                aria-label="PAPA YU"
              >
                <img
                  ref={logoRef}
                  src={`${import.meta.env.BASE_URL}logo-papa-yu.png`}
                  alt="PAPA YU"
                  className="h-10 md:h-12 w-auto object-contain"
                />
              </Link>
              <button
                type="button"
                onClick={handleCheckUpdate}
                disabled={isCheckingUpdate}
                className="flex items-center gap-1.5 px-2 py-1.5 rounded-lg border border-primary/50 text-primary text-xs font-medium hover:bg-primary/10 disabled:opacity-50 transition-colors"
                title="Проверить обновления"
              >
                <img src={`${import.meta.env.BASE_URL}logo-papa-yu.png`} alt="" className="h-5 w-5 object-contain" />
                <Download className="w-3.5 h-3.5" />
              </button>
              {updateStatus && (
                <span className="text-xs text-muted-foreground max-w-[140px] truncate" title={updateStatus}>
                  {updateStatus}
                </span>
              )}
            </div>
            <div ref={navRef} className="flex flex-wrap items-center gap-1 md:gap-2">
              {navItems.map((item) => {
                const isActive = location.pathname === item.path;
                const Icon = item.icon;
                return (
                  <Link
                    key={item.path}
                    to={item.path}
                    onClick={() => handleNav(item.path)}
                    className={`nav-item-anime group flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs md:text-sm font-medium transition-all-smooth ${
                      isActive ? 'text-primary bg-primary/10' : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'
                    }`}
                  >
                    <Icon className="w-3.5 h-3.5 flex-shrink-0" />
                    <span>{item.name}</span>
                  </Link>
                );
              })}
            </div>
          </div>
        </div>
      </nav>
      <main>{children}</main>
    </div>
  );
}
