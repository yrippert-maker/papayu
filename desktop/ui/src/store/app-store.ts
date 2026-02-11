import { create } from 'zustand';
import type { AnalyzeReport } from '../lib/analyze';

export interface AuditEvent {
  id: string;
  event: string;
  timestamp: string;
  actor: string;
  result?: 'success' | 'failure';
  metadata?: Record<string, unknown>;
}

export interface AppState {
  currentRoute: string;
  setCurrentRoute: (route: string) => void;

  /** Last analysis report — shared across pages */
  lastReport: AnalyzeReport | null;
  lastPath: string | null;
  setLastReport: (report: AnalyzeReport, path: string) => void;

  /** Audit events collected from real analysis actions */
  auditEvents: AuditEvent[];
  addAuditEvent: (event: AuditEvent) => void;
  clearAuditEvents: () => void;

  error: string | null;
  setError: (error: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentRoute: '/',
  setCurrentRoute: (route) => set({ currentRoute: route }),

  lastReport: null,
  lastPath: null,
  setLastReport: (report, path) => set({ lastReport: report, lastPath: path }),

  auditEvents: [],
  addAuditEvent: (event) =>
    set((s) => ({
      auditEvents: [event, ...s.auditEvents].slice(0, 200),
    })),
  clearAuditEvents: () => set({ auditEvents: [] }),

  error: null,
  setError: (error) => set({ error }),
}));
