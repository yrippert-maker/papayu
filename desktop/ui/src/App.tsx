import { HashRouter, Routes, Route, useLocation } from 'react-router-dom';
import { useEffect } from 'react';
import { Dashboard } from './pages/Dashboard';
import { Tasks } from './pages/Tasks';
import { PolicyEngine } from './pages/PolicyEngine';
import { AuditLogger } from './pages/AuditLogger';
import { SecretsGuard } from './pages/SecretsGuard';
import { Updates } from './pages/Updates';
import { Diagnostics } from './pages/Diagnostics';
import { Layout } from './components/layout/Layout';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ErrorDisplay } from './components/ErrorDisplay';
import { NotFound } from './pages/NotFound';
import { ROUTES } from './config/routes';
import { useAppStore } from './store/app-store';

function RouteTracker() {
  const location = useLocation();
  useEffect(() => {
    try {
      useAppStore.getState().setCurrentRoute(location.pathname);
    } catch { /* ignored */ }
  }, [location.pathname]);
  return null;
}

function App() {
  return (
    <ErrorBoundary>
      <HashRouter>
        <RouteTracker />
        <ErrorDisplay />
        <Layout>
          <Routes>
            <Route path={ROUTES.TASKS.path} element={<Tasks />} />
            <Route path={ROUTES.CONTROL_PANEL.path} element={<Dashboard />} />
            <Route path={ROUTES.POLICY_ENGINE.path} element={<PolicyEngine />} />
            <Route path={ROUTES.AUDIT_LOGGER.path} element={<AuditLogger />} />
            <Route path={ROUTES.SECRETS_GUARD.path} element={<SecretsGuard />} />
            <Route path={ROUTES.UPDATES.path} element={<Updates />} />
            <Route path={ROUTES.DIAGNOSTICS.path} element={<Diagnostics />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </Layout>
      </HashRouter>
    </ErrorBoundary>
  );
}

export default App;
