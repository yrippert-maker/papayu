export interface PathSelectorProps {
  folderLinks: string[];
  attachedFiles: string[];
  onAddFolder: () => void;
  onAddFile: () => void;
  onRemoveLink: (index: number) => void;
  onRemoveFile: (index: number) => void;
}

const sectionStyle: React.CSSProperties = {
  marginBottom: "24px",
  padding: "24px",
  border: "1px solid var(--color-border)",
  borderRadius: "var(--radius-xl)",
  background: "linear-gradient(180deg, #ffffff 0%, var(--color-bg-warm) 100%)",
  boxShadow: "var(--shadow-md)",
  minHeight: "140px",
};

export function PathSelector({
  folderLinks,
  attachedFiles,
  onAddFolder,
  onAddFile,
  onRemoveLink,
  onRemoveFile,
}: PathSelectorProps) {
  return (
    <section
      className="tasks-sources"
      data-section="path-selection"
      style={sectionStyle}
    >
      <h2 style={{ fontSize: "17px", color: "var(--color-text)", marginBottom: "6px", fontWeight: 700, letterSpacing: "-0.01em" }}>
        Папки и файлы проекта
      </h2>
      <p style={{ fontSize: "13px", color: "var(--color-text-muted)", marginBottom: "18px", lineHeight: 1.5 }}>
        Укажите папку для анализа или прикрепите файлы (исходники, конфиги) для создания и правок. Можно ввести путь вручную в поле ниже.
      </p>
      <div style={{ display: "flex", gap: "12px", marginBottom: "18px", flexWrap: "wrap", alignItems: "center" }}>
        <button
          type="button"
          onClick={onAddFolder}
          aria-label="Выбрать папку"
          style={{
            padding: "12px 22px",
            background: "var(--color-primary)",
            color: "#fff",
            border: "none",
            borderRadius: "var(--radius-md)",
            fontSize: "15px",
            fontWeight: 600,
            boxShadow: "0 2px 8px rgba(37, 99, 235, 0.35)",
          }}
        >
          Выбрать папку
        </button>
        <button
          type="button"
          onClick={onAddFile}
          aria-label="Прикрепить файл"
          style={{
            padding: "12px 22px",
            background: "var(--color-secondary)",
            color: "#fff",
            border: "none",
            borderRadius: "var(--radius-md)",
            fontSize: "15px",
            fontWeight: 600,
            boxShadow: "0 2px 8px rgba(13, 148, 136, 0.35)",
          }}
        >
          Прикрепить файл
        </button>
        <button
          type="button"
          onClick={onAddFolder}
          style={{
            padding: "12px 20px",
            background: "#fff",
            border: "1px solid var(--color-border-strong)",
            borderRadius: "var(--radius-md)",
            fontSize: "14px",
            color: "var(--color-text)",
          }}
        >
          + Добавить ещё папку
        </button>
      </div>
      {folderLinks.length > 0 && (
        <>
          <p style={{ fontSize: "12px", color: "var(--color-text-muted)", marginBottom: "8px", fontWeight: 600 }}>Папки</p>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, marginBottom: "14px" }}>
            {folderLinks.map((p, i) => (
              <li
                key={`f-${i}`}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  marginBottom: "8px",
                  padding: "10px 14px",
                  background: "#fff",
                  borderRadius: "var(--radius-md)",
                  border: "1px solid var(--color-border)",
                  boxShadow: "var(--shadow-sm)",
                }}
              >
                <span style={{ flex: 1, fontSize: "13px", overflow: "hidden", textOverflow: "ellipsis" }} title={p}>{p}</span>
                <button type="button" onClick={() => onRemoveLink(i)} style={{ padding: "6px 12px", fontSize: "12px", background: "var(--color-bg)", border: "none", borderRadius: "var(--radius-sm)", fontWeight: 500 }}>Удалить</button>
              </li>
            ))}
          </ul>
        </>
      )}
      {attachedFiles.length > 0 && (
        <>
          <p style={{ fontSize: "12px", color: "var(--color-text-muted)", marginBottom: "8px", fontWeight: 600 }}>Прикреплённые файлы</p>
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {attachedFiles.map((p, i) => (
              <li
                key={`file-${i}`}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  marginBottom: "8px",
                  padding: "10px 14px",
                  background: "linear-gradient(90deg, #f0fdfa 0%, #fff 100%)",
                  borderRadius: "var(--radius-md)",
                  border: "1px solid #99f6e4",
                  boxShadow: "var(--shadow-sm)",
                }}
              >
                <span style={{ flex: 1, fontSize: "13px", overflow: "hidden", textOverflow: "ellipsis" }} title={p}>{p.split(/[/\\]/).pop() ?? p}</span>
                <button type="button" onClick={() => onRemoveFile(i)} style={{ padding: "6px 12px", fontSize: "12px", background: "#ccfbf1", border: "none", borderRadius: "var(--radius-sm)", fontWeight: 500 }}>Удалить</button>
              </li>
            ))}
          </ul>
        </>
      )}
      {folderLinks.length === 0 && attachedFiles.length === 0 && (
        <p style={{ fontSize: "13px", color: "var(--color-text-muted)", padding: "10px 0" }}>
          Папки и файлы не выбраны. Нажмите «Выбрать папку» или «Прикрепить файл».
        </p>
      )}
    </section>
  );
}
