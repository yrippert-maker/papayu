import type { WeeklyProposal } from "@/lib/types";
import { canApplyProposal } from "./proposalMapping";

export type ProposalCardProps = {
  proposal: WeeklyProposal;
  projectPath: string;
  onApply?: (key: string, value: boolean | number | string) => Promise<void>;
  busy?: boolean;
  applied?: boolean;
};

export function ProposalCard({ proposal, projectPath, onApply, busy, applied }: ProposalCardProps) {
  const action = canApplyProposal(proposal);

  const copySteps = () => {
    const text = proposal.steps?.length
      ? proposal.steps.map((s, i) => `${i + 1}. ${s}`).join("\n")
      : proposal.expected_impact || proposal.title;
    void navigator.clipboard.writeText(text);
  };

  const copySnippet = () => {
    const text = proposal.steps?.length ? proposal.steps.join("\n") : proposal.expected_impact || proposal.title;
    void navigator.clipboard.writeText(text);
  };

  const handleApplySetting = async () => {
    if (!action || action.kind !== "setting" || !onApply) return;
    await onApply(action.key, action.value);
  };

  const riskColor =
    proposal.risk === "high"
      ? "#b91c1c"
      : proposal.risk === "medium"
        ? "#d97706"
        : "var(--color-text-muted)";

  return (
    <div
      style={{
        marginBottom: 14,
        padding: 14,
        background: "var(--color-surface)",
        borderRadius: "var(--radius-lg)",
        border: "1px solid var(--color-border)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", marginBottom: 8 }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: "#1e3a5f" }}>{proposal.title}</span>
        {applied && (
          <span style={{ fontSize: 11, color: "#059669", fontWeight: 600 }}>Applied ✓</span>
        )}
        <span
          style={{
            padding: "2px 8px",
            borderRadius: "var(--radius-sm)",
            background: "var(--color-bg)",
            fontSize: 11,
            color: "var(--color-text-muted)",
            fontWeight: 600,
          }}
        >
          {proposal.kind}
        </span>
        <span
          style={{
            padding: "2px 8px",
            borderRadius: "var(--radius-sm)",
            fontSize: 11,
            color: riskColor,
            fontWeight: 600,
            border: `1px solid ${riskColor}`,
          }}
        >
          risk: {proposal.risk}
        </span>
      </div>
      <p style={{ margin: "0 0 8px 0", fontSize: 13, color: "var(--color-text)", lineHeight: 1.5 }}>{proposal.why}</p>
      <p style={{ margin: "0 0 8px 0", fontSize: 12, color: "var(--color-text-muted)", lineHeight: 1.5 }}>
        <strong>Expected impact:</strong> {proposal.expected_impact}
      </p>
      {proposal.evidence && (
        <pre
          style={{
            margin: "0 0 10px 0",
            padding: 8,
            background: "var(--color-bg)",
            borderRadius: "var(--radius-md)",
            fontSize: 11,
            fontFamily: "var(--font-mono, monospace)",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            color: "var(--color-text-muted)",
          }}
        >
          {proposal.evidence}
        </pre>
      )}
      {proposal.steps?.length > 0 && (
        <pre
          style={{
            margin: "0 0 12px 0",
            padding: 8,
            background: "#f8fafc",
            borderRadius: "var(--radius-md)",
            fontSize: 11,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {proposal.steps.map((s, i) => `${i + 1}. ${s}`).join("\n")}
        </pre>
      )}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <button
          type="button"
          onClick={proposal.kind === "prompt_change" ? copySnippet : copySteps}
          style={{
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            background: "var(--color-surface)",
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          {proposal.kind === "prompt_change" ? "Copy suggested snippet" : "Copy steps"}
        </button>
        {proposal.kind === "golden_trace_add" && (
          <a
            href={`file://${projectPath}/docs/golden_traces/README.md`}
            target="_blank"
            rel="noopener noreferrer"
            style={{
              padding: "5px 10px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--color-border)",
              background: "var(--color-surface)",
              fontSize: 12,
              fontWeight: 600,
              textDecoration: "none",
              color: "var(--color-text)",
            }}
          >
            Open Golden Traces README
          </a>
        )}
        {proposal.kind === "setting_change" && action && (
          <button
            type="button"
            onClick={handleApplySetting}
            disabled={busy}
            style={{
              padding: "5px 10px",
              borderRadius: "var(--radius-md)",
              border: "none",
              background: "#059669",
              color: "#fff",
              fontSize: 12,
              fontWeight: 600,
              cursor: busy ? "not-allowed" : "pointer",
            }}
          >
            {busy ? "…" : `Apply setting (${action.key}=${String(action.value)})`}
          </button>
        )}
      </div>
    </div>
  );
}
