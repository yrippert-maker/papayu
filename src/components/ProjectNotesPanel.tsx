import { useState, useEffect, useCallback } from "react";
import {
  loadDomainNotes,
  deleteDomainNote,
  pinDomainNote,
  clearExpiredDomainNotes,
} from "@/lib/tauri";
import type { DomainNotes, DomainNote } from "@/lib/types";
import { DomainNoteCard } from "./DomainNoteCard";
import { NotesEmptyState } from "./NotesEmptyState";

export type ProjectNotesPanelProps = {
  projectPath: string;
  onDistillLastOnline?: () => void;
};

type SortOption = "recent" | "usage" | "confidence";

function filterAndSortNotes(
  notes: DomainNote[],
  query: string,
  tagFilter: string | null,
  showExpired: boolean,
  sort: SortOption
): DomainNote[] {
  const now = Math.floor(Date.now() / 1000);
  let list = notes;
  if (!showExpired) {
    list = list.filter((n) => {
      const ttl = (n.ttl_days ?? 30) * 24 * 3600;
      return (n.created_at ?? 0) + ttl >= now;
    });
  }
  const q = query.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (n) =>
        (n.topic ?? "").toLowerCase().includes(q) ||
        (n.tags ?? []).some((t) => t.toLowerCase().includes(q)) ||
        (n.content_md ?? "").toLowerCase().includes(q)
    );
  }
  if (tagFilter) {
    list = list.filter((n) => (n.tags ?? []).includes(tagFilter));
  }
  if (sort === "recent") {
    list = [...list].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
  } else if (sort === "usage") {
    list = [...list].sort((a, b) => (b.usage_count ?? 0) - (a.usage_count ?? 0));
  } else if (sort === "confidence") {
    list = [...list].sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0));
  }
  return list;
}

export function ProjectNotesPanel({ projectPath, onDistillLastOnline }: ProjectNotesPanelProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>(undefined);
  const [notes, setNotes] = useState<DomainNotes | undefined>(undefined);
  const [query, setQuery] = useState("");
  const [tagFilter, setTagFilter] = useState<string | null>(null);
  const [showExpired, setShowExpired] = useState(false);
  const [sort, setSort] = useState<SortOption>("recent");
  const [busy, setBusy] = useState<Record<string, boolean>>({});

  const refresh = useCallback(async () => {
    if (!projectPath) {
      setNotes(undefined);
      return;
    }
    setLoading(true);
    setError(undefined);
    try {
      const data = await loadDomainNotes(projectPath);
      setNotes(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setNotes(undefined);
    } finally {
      setLoading(false);
    }
  }, [projectPath]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handlePinToggle = async (id: string, pinned: boolean) => {
    if (!projectPath) return;
    setBusy((b) => ({ ...b, [id]: true }));
    try {
      await pinDomainNote(projectPath, id, pinned);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy((b) => ({ ...b, [id]: false }));
    }
  };

  const handleDelete = async (id: string) => {
    if (!projectPath) return;
    setBusy((b) => ({ ...b, [id]: true }));
    try {
      await deleteDomainNote(projectPath, id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy((b) => ({ ...b, [id]: false }));
    }
  };

  const handleClearExpired = async () => {
    if (!projectPath) return;
    setBusy((b) => ({ ...b, clear_expired: true }));
    try {
      const removed = await clearExpiredDomainNotes(projectPath);
      if (removed > 0) await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy((b) => ({ ...b, clear_expired: false }));
    }
  };

  const list = notes?.notes ?? [];
  const allTags = Array.from(new Set(list.flatMap((n) => n.tags ?? []))).sort();
  const filtered = filterAndSortNotes(list, query, tagFilter, showExpired, sort);

  if (!projectPath) {
    return (
      <div style={{ padding: 16, color: "var(--color-text-muted)", fontSize: 14 }}>
        Выберите проект (папку) для просмотра заметок.
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12, minHeight: 0 }}>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: "#1e3a5f" }}>Project Notes</span>
        <button
          type="button"
          onClick={refresh}
          disabled={loading || busy.clear_expired}
          style={{
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            background: "var(--color-surface)",
            fontSize: 12,
            fontWeight: 600,
            cursor: loading ? "not-allowed" : "pointer",
          }}
        >
          Refresh
        </button>
        <button
          type="button"
          onClick={handleClearExpired}
          disabled={loading || busy.clear_expired}
          style={{
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            background: "var(--color-surface)",
            fontSize: 12,
            fontWeight: 600,
            cursor: busy.clear_expired ? "not-allowed" : "pointer",
          }}
        >
          {busy.clear_expired ? "…" : "Clear expired"}
        </button>
        {onDistillLastOnline && (
          <button
            type="button"
            onClick={onDistillLastOnline}
            style={{
              padding: "5px 10px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--color-primary)",
              background: "var(--color-primary-light)",
              color: "var(--color-primary)",
              fontSize: 12,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Distill last OnlineAnswer → Note
          </button>
        )}
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <input
          type="text"
          placeholder="Search topic/tags/text…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{
            width: 180,
            padding: "6px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            fontSize: 12,
          }}
        />
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value as SortOption)}
          style={{
            padding: "6px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            fontSize: 12,
          }}
        >
          <option value="recent">Sort: recent</option>
          <option value="usage">Sort: usage</option>
          <option value="confidence">Sort: confidence</option>
        </select>
        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--color-text-muted)" }}>
          <input type="checkbox" checked={showExpired} onChange={(e) => setShowExpired(e.target.checked)} />
          Show expired
        </label>
      </div>
      {allTags.length > 0 && (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
          <span style={{ fontSize: 11, color: "var(--color-text-muted)", alignSelf: "center" }}>Tags:</span>
          {allTags.map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTagFilter(tagFilter === t ? null : t)}
              style={{
                padding: "3px 8px",
                borderRadius: "var(--radius-sm)",
                border: "1px solid var(--color-border)",
                background: tagFilter === t ? "var(--color-primary)" : "var(--color-surface)",
                color: tagFilter === t ? "#fff" : "var(--color-text)",
                fontSize: 11,
                cursor: "pointer",
              }}
            >
              {t}
            </button>
          ))}
        </div>
      )}
      {error && (
        <div style={{ padding: 10, background: "#fef2f2", borderRadius: "var(--radius-md)", color: "#b91c1c", fontSize: 13 }}>
          {error}
        </div>
      )}
      {loading && <p style={{ margin: 0, fontSize: 13, color: "var(--color-text-muted)" }}>Загрузка…</p>}
      {!loading && notes && (
        <>
          <p style={{ margin: 0, fontSize: 12, color: "var(--color-text-muted)" }}>
            Заметок: {filtered.length} {list.length !== filtered.length ? `(из ${list.length})` : ""}
          </p>
          <div style={{ overflowY: "auto", flex: 1, minHeight: 0 }}>
            {filtered.length === 0 ? (
              <NotesEmptyState onRunOnlineResearch={onDistillLastOnline} />
            ) : (
              filtered.map((note) => (
                <DomainNoteCard
                  key={note.id}
                  note={note}
                  onPinToggle={handlePinToggle}
                  onDelete={handleDelete}
                  busy={busy[note.id]}
                />
              ))
            )}
          </div>
        </>
      )}
    </div>
  );
}
