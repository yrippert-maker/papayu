import { useState } from "react";
import type { WeeklyProposal } from "@/lib/types";
import { ProposalCard } from "./ProposalCard";

export type WeeklyReportProposalsPanelProps = {
  projectPath: string;
  proposals: WeeklyProposal[];
  onApply?: (key: string, value: boolean | number | string) => Promise<void>;
  onApplied?: () => void;
};

type KindFilter = WeeklyProposal["kind"] | "all";
type RiskFilter = WeeklyProposal["risk"] | "all";

export function WeeklyReportProposalsPanel({
  projectPath,
  proposals,
  onApply,
  onApplied,
}: WeeklyReportProposalsPanelProps) {
  const [busyIndex, setBusyIndex] = useState<number | null>(null);
  const [error, setError] = useState<string | undefined>(undefined);
  const [appliedIds, setAppliedIds] = useState<Set<string>>(new Set());
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");
  const [riskFilter, setRiskFilter] = useState<RiskFilter>("all");

  const proposalId = (p: WeeklyProposal, i: number) => `${p.kind}-${p.title}-${i}`;

  const filtered = proposals.filter((p) => {
    if (kindFilter !== "all" && p.kind !== kindFilter) return false;
    if (riskFilter !== "all" && p.risk !== riskFilter) return false;
    return true;
  });

  const kinds: KindFilter[] = ["all", "setting_change", "golden_trace_add", "prompt_change", "limit_tuning", "safety_rule"];
  const risks: RiskFilter[] = ["all", "low", "medium", "high"];

  const handleApply = async (index: number, key: string, value: boolean | number | string) => {
    if (!onApply) return;
    setBusyIndex(index);
    setError(undefined);
    try {
      await onApply(key, value);
      const p = filtered[index];
      if (p) setAppliedIds((s) => new Set(s).add(proposalId(p, index)));
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyIndex(null);
    }
  };

  if (proposals.length === 0) {
    return (
      <p style={{ margin: 0, padding: 16, color: "var(--color-text-muted)", fontSize: 14 }}>
        В отчёте нет предложений (proposals). Они появляются, когда LLM обосновывает их полями bundle и deltas.
      </p>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <span style={{ fontSize: 12, color: "var(--color-text-muted)", marginRight: 4 }}>Kind:</span>
        {kinds.map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => setKindFilter(k)}
            style={{
              padding: "4px 8px",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--color-border)",
              background: kindFilter === k ? "var(--color-primary)" : "var(--color-surface)",
              color: kindFilter === k ? "#fff" : "var(--color-text)",
              fontSize: 11,
              cursor: "pointer",
            }}
          >
            {k}
          </button>
        ))}
        <span style={{ fontSize: 12, color: "var(--color-text-muted)", marginLeft: 8, marginRight: 4 }}>Risk:</span>
        {risks.map((r) => (
          <button
            key={r}
            type="button"
            onClick={() => setRiskFilter(r)}
            style={{
              padding: "4px 8px",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--color-border)",
              background: riskFilter === r ? "var(--color-primary)" : "var(--color-surface)",
              color: riskFilter === r ? "#fff" : "var(--color-text)",
              fontSize: 11,
              cursor: "pointer",
            }}
          >
            {r}
          </button>
        ))}
      </div>
      {error && (
        <div style={{ padding: 10, background: "#fef2f2", borderRadius: "var(--radius-md)", color: "#b91c1c", fontSize: 13 }}>
          {error}
        </div>
      )}
      <p style={{ margin: 0, fontSize: 12, color: "var(--color-text-muted)" }}>
        Показано: {filtered.length} из {proposals.length}. Apply только для whitelist (onlineAutoUseAsContext).
      </p>
      <div style={{ overflowY: "auto", flex: 1, minHeight: 0 }}>
        {filtered.map((p, i) => (
          <ProposalCard
            key={proposalId(p, i)}
            proposal={p}
            projectPath={projectPath}
            onApply={onApply ? (key, value) => handleApply(i, key, value) : undefined}
            busy={busyIndex === i}
            applied={appliedIds.has(proposalId(p, i))}
          />
        ))}
      </div>
    </div>
  );
}
