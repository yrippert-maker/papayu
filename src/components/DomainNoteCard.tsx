import { useState, useEffect } from "react";
import type { DomainNote } from "@/lib/types";

export type DomainNoteCardProps = {
  note: DomainNote;
  onPinToggle: (id: string, pinned: boolean) => void;
  onDelete: (id: string) => void;
  busy?: boolean;
};

function formatDate(ts: number | null | undefined): string {
  if (ts == null) return "—";
  try {
    const d = new Date(ts * 1000);
    return d.toLocaleDateString(undefined, { dateStyle: "short" }) + " " + d.toLocaleTimeString(undefined, { timeStyle: "short" });
  } catch (_) {
    return "—";
  }
}

export function DomainNoteCard({ note, onPinToggle, onDelete, busy }: DomainNoteCardProps) {
  const [showSources, setShowSources] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  useEffect(() => {
    if (!deleteConfirm) return;
    const t = setTimeout(() => setDeleteConfirm(false), 3000);
    return () => clearTimeout(t);
  }, [deleteConfirm]);

  const copyContent = () => {
    const withSources = note.sources?.length
      ? note.content_md + "\n\nSources:\n" + note.sources.map((s) => `${s.title || s.url}: ${s.url}`).join("\n")
      : note.content_md;
    void navigator.clipboard.writeText(withSources);
  };

  const handleDelete = () => {
    if (deleteConfirm) {
      onDelete(note.id);
      setDeleteConfirm(false);
    } else {
      setDeleteConfirm(true);
      const t = setTimeout(() => setDeleteConfirm(false), 3000);
      return () => clearTimeout(t);
    }
  };

  return (
    <div
      style={{
        marginBottom: 12,
        padding: 14,
        background: "var(--color-surface)",
        borderRadius: "var(--radius-lg)",
        border: "1px solid var(--color-border)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 10, marginBottom: 6 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <span style={{ fontWeight: 700, fontSize: 14, color: "#1e3a5f" }}>{note.topic}</span>
          {note.pinned && (
            <span style={{ marginLeft: 8, fontSize: 11, color: "var(--color-text-muted)", fontWeight: 500 }}>📌 pinned</span>
          )}
          <span style={{ marginLeft: 8, fontSize: 12, color: "var(--color-text-muted)" }}>confidence {(note.confidence ?? 0).toFixed(2)}</span>
        </div>
      </div>
      {note.tags?.length > 0 && (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
          {note.tags.map((t) => (
            <span
              key={t}
              style={{
                padding: "2px 8px",
                borderRadius: "var(--radius-sm)",
                background: "var(--color-bg)",
                fontSize: 11,
                color: "var(--color-text-muted)",
              }}
            >
              {t}
            </span>
          ))}
        </div>
      )}
      <pre
        style={{
          margin: "0 0 10px 0",
          padding: 10,
          background: "var(--color-bg)",
          borderRadius: "var(--radius-md)",
          fontSize: 12,
          lineHeight: 1.5,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          fontFamily: "inherit",
        }}
      >
        {note.content_md}
      </pre>
      <div style={{ fontSize: 11, color: "var(--color-text-muted)", marginBottom: 8 }}>
        usage: {note.usage_count ?? 0} · last used: {formatDate(note.last_used_at)}
      </div>
      {note.sources?.length > 0 && (
        <div style={{ marginBottom: 10 }}>
          <button
            type="button"
            onClick={() => setShowSources(!showSources)}
            style={{
              padding: "4px 0",
              border: "none",
              background: "none",
              fontSize: 11,
              color: "var(--color-primary)",
              cursor: "pointer",
              fontWeight: 600,
            }}
          >
            {showSources ? "Hide sources" : `Sources (${note.sources.length})`}
          </button>
          {showSources && (
            <ul style={{ margin: "6px 0 0 0", paddingLeft: 18, fontSize: 11, color: "var(--color-text-muted)", lineHeight: 1.6 }}>
              {note.sources.map((s, i) => (
                <li key={i}>
                  <a href={s.url} target="_blank" rel="noopener noreferrer" style={{ color: "var(--color-primary)" }}>
                    {s.title || s.url}
                  </a>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <button
          type="button"
          onClick={() => onPinToggle(note.id, !note.pinned)}
          disabled={busy}
          style={{
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            background: note.pinned ? "var(--color-primary)" : "var(--color-surface)",
            color: note.pinned ? "#fff" : "var(--color-text)",
            fontSize: 12,
            fontWeight: 600,
            cursor: busy ? "not-allowed" : "pointer",
          }}
        >
          {busy ? "…" : note.pinned ? "Unpin" : "Pin"}
        </button>
        <button
          type="button"
          onClick={copyContent}
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
          Copy
        </button>
        <button
          type="button"
          onClick={handleDelete}
          disabled={busy}
          style={{
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid #fecaca",
            background: deleteConfirm ? "#b91c1c" : "#fef2f2",
            color: deleteConfirm ? "#fff" : "#b91c1c",
            fontSize: 12,
            fontWeight: 600,
            cursor: busy ? "not-allowed" : "pointer",
          }}
        >
          {deleteConfirm ? "Confirm delete?" : "Delete"}
        </button>
      </div>
    </div>
  );
}
