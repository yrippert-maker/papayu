export type NotesEmptyStateProps = {
  onRunOnlineResearch?: () => void;
};

export function NotesEmptyState({ onRunOnlineResearch }: NotesEmptyStateProps) {
  return (
    <div
      style={{
        padding: 24,
        textAlign: "center",
        background: "var(--color-surface)",
        borderRadius: "var(--radius-lg)",
        border: "1px dashed var(--color-border)",
      }}
    >
      <p style={{ margin: "0 0 12px 0", fontSize: 14, color: "var(--color-text-muted)", lineHeight: 1.6 }}>
        Notes создаются автоматически после Online Research (при достаточной confidence).
      </p>
      {onRunOnlineResearch && (
        <button
          type="button"
          onClick={onRunOnlineResearch}
          style={{
            padding: "10px 18px",
            borderRadius: "var(--radius-md)",
            border: "none",
            background: "var(--color-primary)",
            color: "#fff",
            fontWeight: 600,
            fontSize: 13,
            cursor: "pointer",
          }}
        >
          Run Online Research
        </button>
      )}
      {!onRunOnlineResearch && (
        <p style={{ margin: 0, fontSize: 12, color: "var(--color-text-muted)" }}>
          Задайте запрос в поле выше и запустите анализ — при срабатывании online fallback заметка может быть сохранена.
        </p>
      )}
    </div>
  );
}
