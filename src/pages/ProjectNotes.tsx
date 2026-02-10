import { useState, useEffect } from "react";
import { getFolderLinks } from "@/lib/tauri";
import { ProjectNotesPanel } from "@/components";

const STORAGE_LINKS = "papa_yu_folder_links";

function loadLocalLinks(): string[] {
  try {
    const s = localStorage.getItem(STORAGE_LINKS);
    if (s) return JSON.parse(s);
  } catch (_) {}
  return [];
}

export default function ProjectNotes() {
  const [folderLinks, setFolderLinks] = useState<string[]>(loadLocalLinks());
  const [projectPath, setProjectPath] = useState<string>("");

  useEffect(() => {
    (async () => {
      try {
        const links = await getFolderLinks();
        if (links.paths?.length) {
          setFolderLinks(links.paths);
          if (!projectPath && links.paths[0]) setProjectPath(links.paths[0]);
        }
      } catch (_) {}
    })();
  }, []);

  const hasProjects = folderLinks.length > 0;

  return (
    <div style={{ maxWidth: 900, margin: "0 auto" }}>
      <h1 style={{ marginBottom: 24, fontSize: 24, fontWeight: 700, color: "#1e3a5f", letterSpacing: "-0.02em" }}>
        Project Notes
      </h1>

      {!hasProjects && (
        <p style={{ padding: 20, background: "var(--color-surface)", borderRadius: "var(--radius-lg)", color: "var(--color-text-muted)" }}>
          Добавьте папку проекта во вкладке «Задачи» (ссылки на папки), чтобы управлять заметками.
        </p>
      )}

      {hasProjects && (
        <>
          <div style={{ marginBottom: 16, display: "flex", flexWrap: "wrap", gap: 12, alignItems: "center" }}>
            <label style={{ fontWeight: 600, fontSize: 14, color: "var(--color-text)" }}>Проект:</label>
            <select
              value={projectPath}
              onChange={(e) => setProjectPath(e.target.value)}
              style={{
                minWidth: 280,
                padding: "8px 12px",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--color-border)",
                background: "var(--color-bg)",
                color: "var(--color-text)",
                fontSize: 14,
              }}
            >
              <option value="">— выберите —</option>
              {folderLinks.map((path) => (
                <option key={path} value={path}>
                  {path}
                </option>
              ))}
            </select>
          </div>
          <ProjectNotesPanel projectPath={projectPath} />
        </>
      )}
    </div>
  );
}
