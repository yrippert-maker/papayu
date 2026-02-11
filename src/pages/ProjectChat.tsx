import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export default function ProjectChat() {
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectFolder = async () => {
    const selected = await open({ directory: true });
    if (selected) setProjectPath(selected);
  };

  const ask = async () => {
    if (!projectPath || !question.trim()) {
      setError("Выберите папку проекта и введите вопрос");
      return;
    }
    setLoading(true);
    setError(null);
    setAnswer(null);
    try {
      const result = await invoke<string>("rag_query_cmd", {
        projectPath,
        question: question.trim(),
      });
      setAnswer(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ maxWidth: 720 }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>Вопрос по проекту</h1>
      <p style={{ color: "var(--color-text-muted)", marginBottom: 24 }}>
        Задайте вопрос по коду — ответ будет с учётом контекста файлов проекта (нужен PAPAYU_LLM_API_URL).
      </p>
      <div style={{ display: "flex", gap: 12, alignItems: "center", marginBottom: 16, flexWrap: "wrap" }}>
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
      </div>
      <textarea
        value={question}
        onChange={(e) => setQuestion(e.target.value)}
        placeholder="Например: где настраивается роутинг? или какие тесты есть для API?"
        rows={3}
        style={{
          width: "100%",
          padding: 12,
          borderRadius: 8,
          border: "1px solid var(--color-border)",
          marginBottom: 12,
          resize: "vertical",
          fontFamily: "inherit",
        }}
      />
      <button
        type="button"
        onClick={ask}
        disabled={loading || !projectPath}
        style={{
          padding: "10px 20px",
          background: "#2563eb",
          color: "#fff",
          border: "none",
          borderRadius: 8,
          fontWeight: 600,
          cursor: loading || !projectPath ? "not-allowed" : "pointer",
        }}
      >
        {loading ? "Отправка…" : "Спросить"}
      </button>
      {error && <p style={{ marginTop: 16, color: "#dc2626", fontSize: 14 }}>{error}</p>}
      {answer && (
        <div
          style={{
            marginTop: 24,
            padding: 16,
            background: "var(--color-card)",
            borderRadius: 12,
            border: "1px solid var(--color-border)",
            whiteSpace: "pre-wrap",
            fontSize: 14,
          }}
        >
          {answer}
        </div>
      )}
    </div>
  );
}
