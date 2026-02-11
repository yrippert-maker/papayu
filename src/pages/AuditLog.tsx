import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AuditEvent {
  ts: string;
  event_type: string;
  project_path: string | null;
  result: string | null;
  details: string | null;
}

export default function AuditLog() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<AuditEvent[]>("audit_log_list_cmd", { limit: 100 });
      setEvents(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  return (
    <div style={{ maxWidth: 900 }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>Журнал аудита</h1>
      <p style={{ color: "var(--color-text-muted)", marginBottom: 24 }}>
        События анализа и применения изменений. Лог хранится локально.
      </p>
      <button
        type="button"
        onClick={load}
        disabled={loading}
        style={{
          padding: "8px 16px",
          background: "var(--color-border)",
          border: "none",
          borderRadius: 8,
          cursor: loading ? "not-allowed" : "pointer",
          marginBottom: 16,
        }}
      >
        Обновить
      </button>
      {error && <p style={{ color: "#dc2626", marginBottom: 16 }}>{error}</p>}
      {events.length === 0 && !loading && (
        <p style={{ color: "var(--color-text-muted)" }}>Событий пока нет.</p>
      )}
      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
        {events.map((ev, i) => (
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
            <span style={{ fontWeight: 600 }}>{ev.event_type}</span>
            {ev.project_path && (
              <span style={{ color: "var(--color-text-muted)", marginLeft: 8 }}>
                {ev.project_path}
              </span>
            )}
            {ev.result && (
              <span style={{ marginLeft: 8, color: ev.result === "ok" ? "#16a34a" : "#dc2626" }}>
                {ev.result}
              </span>
            )}
            <div style={{ marginTop: 4, color: "var(--color-text-muted)", fontSize: 12 }}>
              {ev.ts}
              {ev.details && ` · ${ev.details}`}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
