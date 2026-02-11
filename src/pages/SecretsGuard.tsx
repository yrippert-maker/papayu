import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface SecretSuspicion {
  path: string;
  line: number | null;
  kind: string;
  snippet: string;
}

export default function SecretsGuard() {
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [list, setList] = useState<SecretSuspicion[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectFolder = async () => {
    const selected = await open({ directory: true });
    if (selected) setProjectPath(selected);
  };

  const runScan = async () => {
    if (!projectPath) {
      setError("Сначала выберите папку проекта");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<SecretSuspicion[]>("scan_secrets_cmd", {
        projectPath,
      });
      setList(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ maxWidth: 800 }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>Защита секретов</h1>
      <p style={{ color: "var(--color-text-muted)", marginBottom: 24 }}>
        Сканирование проекта на типичные утечки: ключи в коде, .env в репо, подозрительные паттерны.
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
          onClick={runScan}
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
          {loading ? "Сканирование…" : "Проверить"}
        </button>
      </div>
      {error && <p style={{ color: "#dc2626", marginBottom: 16 }}>{error}</p>}
      {list.length > 0 && (
        <p style={{ marginBottom: 12, fontSize: 14 }}>
          Найдено подозрений: <strong>{list.length}</strong>
        </p>
      )}
      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
        {list.map((s, i) => (
          <li
            key={i}
            style={{
              padding: "12px 16px",
              marginBottom: 8,
              background: "var(--color-card)",
              borderRadius: 8,
              border: "1px solid var(--color-border)",
              fontSize: 14,
            }}
          >
            <span style={{ fontWeight: 600 }}>{s.path}</span>
            {s.line != null && (
              <span style={{ color: "var(--color-text-muted)", marginLeft: 8 }}>стр. {s.line}</span>
            )}
            <span style={{ marginLeft: 8, color: "#b45309" }}>{s.kind}</span>
            <div style={{ marginTop: 6, fontSize: 13, fontFamily: "monospace" }}>{s.snippet}</div>
          </li>
        ))}
      </ul>
    </div>
  );
}
