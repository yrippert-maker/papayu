import type { AgenticRunResult, AnalyzeReport } from "@/lib/types";

export interface AgenticResultProps {
  agenticResult: AgenticRunResult | null;
  lastReport: AnalyzeReport | null;
  undoAvailable: boolean;
  onUndo: () => void;
  onDownloadReport: () => void;
  onDownloadDiff: () => void;
}

const sectionStyle: React.CSSProperties = {
  marginTop: "16px",
  padding: "12px",
  background: "#eff6ff",
  borderRadius: "8px",
  border: "1px solid #bfdbfe",
};

export function AgenticResult({
  agenticResult,
  lastReport,
  undoAvailable,
  onUndo,
  onDownloadReport,
  onDownloadDiff,
}: AgenticResultProps) {
  if (!agenticResult) return null;

  const lastAttempt = agenticResult.attempts[agenticResult.attempts.length - 1];
  const canDownloadDiff = !!lastAttempt?.preview?.diffs?.length;

  return (
    <section style={sectionStyle}>
      <h3 style={{ fontSize: "14px", marginBottom: "8px", fontWeight: 600 }}>Результат исправления</h3>
      <p style={{ fontSize: "13px", marginBottom: "10px" }}>{agenticResult.final_summary}</p>
      {agenticResult.attempts.length > 0 && (
        <div style={{ marginBottom: "10px", overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
            <thead>
              <tr style={{ borderBottom: "1px solid #e2e8f0" }}>
                <th style={{ textAlign: "left", padding: "6px 8px" }}>Попытка</th>
                <th style={{ textAlign: "left", padding: "6px 8px" }}>Verify</th>
                <th style={{ textAlign: "left", padding: "6px 8px" }}>Статус</th>
              </tr>
            </thead>
            <tbody>
              {agenticResult.attempts.map((a, i) => (
                <tr key={i} style={{ borderBottom: "1px solid #f1f5f9" }}>
                  <td style={{ padding: "6px 8px" }}>{a.attempt}</td>
                  <td style={{ padding: "6px 8px" }}>{a.verify.ok ? "✓" : "✗"}</td>
                  <td style={{ padding: "6px 8px" }}>{!a.verify.ok ? "откачено" : a.apply.ok ? "применено" : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
        <button
          type="button"
          onClick={onDownloadReport}
          disabled={!lastReport}
          style={{ padding: "6px 12px", border: "1px solid #3b82f6", borderRadius: "6px", background: "#fff", fontSize: "12px" }}
        >
          Скачать отчёт (JSON)
        </button>
        <button
          type="button"
          onClick={onDownloadDiff}
          disabled={!canDownloadDiff}
          style={{ padding: "6px 12px", border: "1px solid #3b82f6", borderRadius: "6px", background: "#fff", fontSize: "12px" }}
        >
          Скачать изменения (diff)
        </button>
        {undoAvailable && (
          <button
            type="button"
            onClick={onUndo}
            style={{ padding: "6px 12px", border: "1px solid #64748b", borderRadius: "6px", background: "#64748b", color: "#fff", fontSize: "12px" }}
          >
            Откатить изменения (Undo)
          </button>
        )}
      </div>
    </section>
  );
}
