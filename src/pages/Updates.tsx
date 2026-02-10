import { useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export default function Updates() {
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<Awaited<ReturnType<typeof check>> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);

  const handleCheck = async () => {
    setChecking(true);
    setError(null);
    setUpdate(null);
    try {
      const u = await check();
      setUpdate(u);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  };

  const handleInstall = async () => {
    if (!update) return;
    setDownloading(true);
    setError(null);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDownloading(false);
    }
  };

  return (
    <div style={{ maxWidth: 600 }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>Обновления</h1>
      <p style={{ color: "var(--color-text-muted)", marginBottom: 24 }}>
        Проверка и установка обновлений PAPA YU.
      </p>
      <button
        type="button"
        onClick={handleCheck}
        disabled={checking}
        style={{
          padding: "10px 20px",
          background: "#2563eb",
          color: "#fff",
          border: "none",
          borderRadius: 8,
          fontWeight: 600,
          cursor: checking ? "not-allowed" : "pointer",
        }}
      >
        {checking ? "Проверка…" : "Проверить обновления"}
      </button>
      {error && (
        <p style={{ marginTop: 16, color: "#dc2626", fontSize: 14 }}>{error}</p>
      )}
      {update && (
        <div
          style={{
            marginTop: 24,
            padding: 16,
            background: "var(--color-card)",
            borderRadius: 12,
            border: "1px solid var(--color-border)",
          }}
        >
          <p style={{ fontWeight: 600, marginBottom: 8 }}>
            Доступна версия {update.version}
          </p>
          {update.body && (
            <p style={{ fontSize: 14, color: "var(--color-text-muted)", marginBottom: 16 }}>
              {update.body.slice(0, 200)}
              {update.body.length > 200 ? "…" : ""}
            </p>
          )}
          <button
            type="button"
            onClick={handleInstall}
            disabled={downloading}
            style={{
              padding: "8px 16px",
              background: "#16a34a",
              color: "#fff",
              border: "none",
              borderRadius: 8,
              fontWeight: 600,
              cursor: downloading ? "not-allowed" : "pointer",
            }}
          >
            {downloading ? "Загрузка…" : "Установить"}
          </button>
        </div>
      )}
      {!update && !checking && !error && (
        <p style={{ marginTop: 16, fontSize: 14, color: "var(--color-text-muted)" }}>
          Нажмите «Проверить обновления» для поиска новых версий.
        </p>
      )}
    </div>
  );
}
