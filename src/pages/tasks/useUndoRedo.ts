import { useState, useCallback } from "react";
import { getUndoRedoState, getUndoStatus, undoLastTx, undoLast, redoLast } from "@/lib/tauri";
import type { Action, AnalyzeReport, ChatMessage, DiffItem } from "@/lib/types";

export interface UseUndoRedoSetters {
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  setPendingPreview: React.Dispatch<React.SetStateAction<{ path: string; actions: Action[]; diffs: DiffItem[] } | null>>;
  setLastReport: React.Dispatch<React.SetStateAction<AnalyzeReport | null>>;
}

export function useUndoRedo(lastPath: string | null, setters: UseUndoRedoSetters) {
  const [undoAvailable, setUndoAvailable] = useState(false);
  const [redoAvailable, setRedoAvailable] = useState(false);

  const refreshUndoRedo = useCallback(async () => {
    try {
      const r = await getUndoRedoState();
      const st = await getUndoStatus();
      setUndoAvailable(!!r.undo_available || !!st.available);
      setRedoAvailable(!!r.redo_available);
    } catch (_) {}
  }, []);

  const handleUndo = useCallback(async () => {
    const { setMessages, setPendingPreview, setLastReport } = setters;
    try {
      if (lastPath) {
        try {
          const ok = await undoLastTx(lastPath);
          if (ok) {
            setMessages((m) => [...m, { role: "system", text: "Последнее действие отменено." }]);
            setPendingPreview(null);
          } else {
            setMessages((m) => [...m, { role: "system", text: "Откат недоступен для этого пути." }]);
          }
          await refreshUndoRedo();
          return;
        } catch (_) {}
      }
      const r = await undoLast();
      if (r.ok) {
        setMessages((m) => [...m, { role: "system", text: "Откат выполнен." }]);
        setPendingPreview(null);
        setLastReport(null);
      } else {
        setMessages((m) => [...m, { role: "system", text: r.error || "Откат не выполнен." }]);
      }
      await refreshUndoRedo();
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка отката: ${String(e)}` }]);
      await refreshUndoRedo();
    }
  }, [lastPath, setters, refreshUndoRedo]);

  const handleRedo = useCallback(async () => {
    const { setMessages } = setters;
    try {
      const r = await redoLast();
      if (r.ok) {
        setMessages((m) => [...m, { role: "system", text: "Изменения повторно применены." }]);
      } else {
        setMessages((m) => [...m, { role: "system", text: r.error || "Повтор не выполнен." }]);
      }
      await refreshUndoRedo();
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка повтора: ${String(e)}` }]);
    }
  }, [setters, refreshUndoRedo]);

  return {
    undoAvailable,
    redoAvailable,
    refreshUndoRedo,
    handleUndo,
    handleRedo,
    setUndoAvailable,
    setRedoAvailable,
  };
}
