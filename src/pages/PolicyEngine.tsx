import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface PolicyRule {
  id: string;
  name: string;
  description: string;
  check: string;
}

interface PolicyCheckResult {
  rule_id: string;
  passed: boolean;
  message: string;
}

export default function PolicyEngine() {
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [rules, setRules] = useState<PolicyRule[]>([]);
  const [results, setResults] = useState<PolicyCheckResult[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadPolicies = async () => {
    try {
      const list = await invoke<PolicyRule[]>("get_policies_cmd");
      setRules(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const selectFolder = async () => {
    const selected = await open({ directory: true });
    if (selected) setProjectPath(selected);
  };

  const runCheck = async () => {
    if (!projectPath) {
      setError("Сначала выберите папку проекта");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<PolicyCheckResult[]>("run_policy_check_cmd", {
        projectPath,
      });
      setResults(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadPolicies();
  }, []);

  return (
    <div style={{ maxWidth: 700 }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>Движок политик</h1>
      <p style={{ color: "var(--color-text-muted)", marginBottom: 24 }}>
        Проверка проекта по правилам: README, .gitignore, .env не в репо, наличие tests/.
      </p>
      <div style={{ display: "flex", gap: 12, alignItems: "center", marginBottom: 24, flexWrap: "wrap" }}>
        <button
          type="button"
          onClick={selectFolder}
          style={{
            padding: "10px 18px",
            background: "var(--color-border)",
            border: "none",
            borderRadius: 8,
            cursor: "pointer",
          }}
        >
          Выбрать папку
        </button>
        {projectPath && (
          <span style={{ fontSize: 14, color: "var(--color-text-muted)" }}>{projectPath}</span>
        )}
        <button
          type="button"
          onClick={runCheck}
          disabled={loading || !projectPath}
          style={{
            padding: "10px 18px",
            background: "#2563eb",
            color: "#fff",
            border: "none",
            borderRadius: 8,
            fontWeight: 600,
            cursor: loading || !projectPath ? "not-allowed" : "pointer",
          }}
        >
          {loading ? "Проверка…" : "Проверить"}
        </button>
      </div>
      {error && <p style={{ color: "#dc2626", marginBottom: 16 }}>{error}</p>}
      {results && (
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {results.map((r, i) => (
            <li
              key={i}
              style={{
                padding: "12px 16px",
                marginBottom: 8,
                background: "var(--color-card)",
                borderRadius: 8,
                border: "1px solid var(--color-border)",
                display: "flex",
                alignItems: "center",
                gap: 12,
              }}
            >
              <span
                style={{
                  width: 24,
                  height: 24,
                  borderRadius: "50%",
                  background: r.passed ? "#16a34a" : "#dc2626",
                  flexShrink: 0,
                }}
                title={r.passed ? "Выполнено" : "Не выполнено"}
              />
              <div>
                <span style={{ fontWeight: 600 }}>{r.rule_id}</span>
                <div style={{ fontSize: 14, color: "var(--color-text-muted)" }}>{r.message}</div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
