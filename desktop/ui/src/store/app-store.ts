import { create } from 'zustand';

export interface AppState {
  currentRoute: string;
  setCurrentRoute: (route: string) => void;
  systemStatus: {
    policyEngine: 'active' | 'inactive';
    auditLogger: 'active' | 'inactive';
    secretsGuard: 'active' | 'inactive';
  };
  recentAuditEvents: Array<{ id: string; event: string; timestamp: string; actor: string }>;
  addAuditEvent: (event: AppState['recentAuditEvents'][0]) => void;
  error: string | null;
  setError: (error: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentRoute: '/',
  setCurrentRoute: (route) => set({ currentRoute: route }),
  systemStatus: {
    policyEngine: 'active',
    auditLogger: 'active',
    secretsGuard: 'active',
  },
  recentAuditEvents: [],
  addAuditEvent: (event) =>
    set((s) => ({
      recentAuditEvents: [event, ...s.recentAuditEvents].slice(0, 50),
    })),
  error: null,
  setError: (error) => set({ error }),
}));
