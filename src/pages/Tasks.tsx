import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  getFolderLinks,
  setFolderLinks as setFolderLinksBackend,
  getProjectProfile,
  runBatchCmd,
  applyActionsTx as apiApplyActionsTx,
  generateActionsFromReport,
  agenticRun,
  listProjects,
  listSessions,
  addProject,
  appendSessionEvent,
  proposeActions,
  previewActions,
  verifyProject,
  getTrendsRecommendations,
  fetchTrendsRecommendations,
  exportSettings,
  importSettings,
} from "@/lib/tauri";
import { AgenticResult } from "@/pages/tasks/AgenticResult";
import { useUndoRedo } from "@/pages/tasks/useUndoRedo";
import { useTheme } from "@/lib/useTheme";
import type {
  Action,
  ActionGroup,
  AnalyzeReport,
  ChatMessage,
  DiffItem,
  ProjectProfile,
  ApplyTxResult,
  AgenticRunRequest,
  AgenticRunResult,
  Session,
  TrendsRecommendation,
  TrendsResult,
  VerifyResult,
} from "@/lib/types";

const STORAGE_LINKS = "papa_yu_folder_links";

function loadLocalLinks(): string[] {
  try {
    const s = localStorage.getItem(STORAGE_LINKS);
    if (s) return JSON.parse(s);
  } catch (_) {}
  return [];
}

function saveLocalLinks(paths: string[]) {
  localStorage.setItem(STORAGE_LINKS, JSON.stringify(paths));
}

export default function Tasks() {
  const [folderLinks, setFolderLinks] = useState<string[]>(loadLocalLinks());
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [lastReport, setLastReport] = useState<AnalyzeReport | null>(null);
  const [selectedActions, setSelectedActions] = useState<Action[]>([]);
  const [pendingPreview, setPendingPreview] = useState<{ path: string; actions: Action[]; diffs: DiffItem[] } | null>(null);
  const [autoCheck, setAutoCheck] = useState(true);
  const [lastPath, setLastPath] = useState<string | null>(null);
  const [lastReportJson, setLastReportJson] = useState<string | null>(null);
  const [pendingActions, setPendingActions] = useState<Action[] | null>(null);
  const [pendingActionIdx, setPendingActionIdx] = useState<Record<number, boolean>>({});
  const [selectedFixGroupIds, setSelectedFixGroupIds] = useState<Record<string, boolean>>({});
  const [selectedPackIds, setSelectedPackIds] = useState<Record<string, boolean>>({});
  const [suggestedActions, setSuggestedActions] = useState<Action[]>([]);
  const [selectedActionIdx, setSelectedActionIdx] = useState<Record<number, boolean>>({});
  const [isGeneratingActions, setIsGeneratingActions] = useState(false);
  const [createOnlyMode, setCreateOnlyMode] = useState(true);
  const [agenticRunning, setAgenticRunning] = useState(false);
  const [agenticProgress, setAgenticProgress] = useState<{ stage: string; message: string; attempt: number } | null>(null);
  const [agenticResult, setAgenticResult] = useState<AgenticRunResult | null>(null);
  const [profile, setProfile] = useState<ProjectProfile | null>(null);
  const [attachedFiles, setAttachedFiles] = useState<string[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [sessionsExpanded, setSessionsExpanded] = useState(false);
  const [agentGoal, setAgentGoal] = useState("");
  const [verifyResult, setVerifyResult] = useState<VerifyResult | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [designStyle, setDesignStyle] = useState("");
  const [trends, setTrends] = useState<TrendsResult | null>(null);
  const [trendsLoading, setTrendsLoading] = useState(false);
  const [applyProgressVisible, setApplyProgressVisible] = useState(false);
  const [applyProgressLog, setApplyProgressLog] = useState<string[]>([]);
  const [applyResult, setApplyResult] = useState<ApplyTxResult | null>(null);
  const applyingRef = useRef(false);
  const [requestHistory, setRequestHistory] = useState<{ id: string; title: string; messages: ChatMessage[]; lastPath: string | null; lastReport: AnalyzeReport | null }[]>([]);
  const [trendsModalOpen, setTrendsModalOpen] = useState(false);
  const [selectedRecommendation, setSelectedRecommendation] = useState<TrendsRecommendation | null>(null);
  const [attachmentMenuOpen, setAttachmentMenuOpen] = useState(false);
  const [lastPlanJson, setLastPlanJson] = useState<string | null>(null);
  const [lastPlanContext, setLastPlanContext] = useState<string | null>(null);

  const { undoAvailable, redoAvailable, refreshUndoRedo, handleUndo, handleRedo, setUndoAvailable } = useUndoRedo(lastPath, {
    setMessages,
    setPendingPreview,
    setLastReport,
  });

  const { toggleTheme, isDark } = useTheme();

  useEffect(() => {
    saveLocalLinks(folderLinks);
    refreshUndoRedo();
    (async () => {
      try {
        const links = await getFolderLinks();
        if (links.paths?.length) setFolderLinks(links.paths);
      } catch (_) {}
    })();
  }, []);

  useEffect(() => {
    if (!lastPath) {
      setSessions([]);
      return;
    }
    (async () => {
      try {
        const projects = await listProjects();
        const projectId = projects.find((p) => p.path === lastPath)?.id;
        if (projectId) {
          const list = await listSessions(projectId);
          setSessions(list);
        } else {
          setSessions([]);
        }
      } catch (_) {
        setSessions([]);
      }
    })();
  }, [lastPath]);

  useEffect(() => {
    (async () => {
      try {
        const res = await getTrendsRecommendations();
        setTrends(res);
      } catch (_) {
        setTrends(null);
      }
    })();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPendingPreview(null);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        if (!loading) onAnalyze();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [loading]);

  useEffect(() => {
    const unlisten = listen<{ stage: string; message: string; attempt: number }>("agentic_progress", (ev) => {
      setAgenticProgress(ev.payload);
      const stageToText: Record<string, string> = {
        analyze: "Сканирую проект…",
        plan: "Составляю план исправлений…",
        preview: "Показываю, что изменится…",
        apply: "Применяю изменения…",
        verify: "Проверяю сборку/типы…",
        revert: "Обнаружены ошибки. Откатываю изменения…",
        done: "Готово.",
        failed: "Не удалось безопасно применить изменения.",
      };
      const text = stageToText[ev.payload.stage] ?? ev.payload.message;
      setMessages((m) => [...m, { role: "system", text: ev.payload.attempt > 0 ? `Попытка ${ev.payload.attempt}/2. ${text}` : text }]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("analyze_progress", (ev) => {
      if (applyingRef.current && typeof ev.payload === "string") {
        setApplyProgressLog((prev) => [...prev, ev.payload]);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const syncFolderLinksToBackend = async (paths: string[]) => {
    try {
      await setFolderLinksBackend(paths);
    } catch (_) {}
  };

  const addFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === "string") {
      const next = [...folderLinks, selected];
      setFolderLinks(next);
      saveLocalLinks(next);
      syncFolderLinksToBackend(next);
      setLastPath(selected);
      try {
        const p = await getProjectProfile(selected);
        setProfile(p);
        setMessages((m) => [...m, { role: "system", text: `Профиль: ${p.project_type} · Safe Mode · Attempts: ${p.max_attempts}` }]);
      } catch (_) {
        setProfile(null);
      }
    }
  };

  const removeLink = (idx: number) => {
    const next = folderLinks.filter((_, i) => i !== idx);
    setFolderLinks(next);
    saveLocalLinks(next);
    syncFolderLinksToBackend(next);
  };

  const ALLOWED_FILE_EXT = new Set(
    ["ts", "tsx", "js", "jsx", "rs", "py", "json", "toml", "md", "yml", "yaml", "css", "html", "xml"].map((e) => e.toLowerCase())
  );

  const addFile = async () => {
    const selected = await open({
      directory: false,
      multiple: true,
      title: "Выберите файлы (исходники, конфиги)",
      // Без filters — на macOS диалог показывает все файлы; разрешённые форматы отбираем ниже.
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      const valid = paths
        .filter((p): p is string => typeof p === "string" && p.trim().length > 0)
        .filter((p) => {
          const ext = p.split(/[/\\]/).pop()?.split(".").pop()?.toLowerCase() ?? "";
          return ext && ALLOWED_FILE_EXT.has(ext);
        });
      if (valid.length) setAttachedFiles((prev) => [...prev, ...valid]);
    }
  };

  const removeFile = (idx: number) => {
    setAttachedFiles((prev) => prev.filter((_, i) => i !== idx));
  };
  void [addFolder, removeLink, addFile, removeFile]; // Reserved for PathSelector UI

  /** Считаем ввод путём, только если он похож на путь к папке/файлу (иначе это вопрос — не анализировать как путь). */
  const inputLooksLikePath = (t: string): boolean => {
    if (!t || t.length > 260) return false;
    if (/^[/~.]/.test(t) || /^[A-Za-z]:[/\\]/.test(t)) return true;
    if (/[/\\]/.test(t)) return true;
    if (!/\s/.test(t) && t.length < 80) return true;
    return false;
  };

  const pathsToUse = (): string[] => {
    const t = input.trim();
    if (t && inputLooksLikePath(t)) return [t];
    const folders = folderLinks.length ? folderLinks : [];
    const fileParentDirs = attachedFiles
      .map((f) => f.replace(/[/\\][^/\\]+$/, ""))
      .filter((d) => d && !folders.includes(d));
    const uniq = [...new Set([...folders, ...fileParentDirs])];
    return uniq.length ? uniq : ["."];
  };

  const runBatch = async (confirmApply: boolean, actionsToApply: Action[], pathsOverride?: string[], userConfirmed?: boolean) => {
    const paths = pathsOverride ?? pathsToUse();
    const usedInputAsPath = !!input.trim() && inputLooksLikePath(input.trim());
    setLoading(true);
    if (!pathsOverride) {
      if (input.trim() && !usedInputAsPath) {
        setMessages((m) => [
          ...m,
          { role: "system", text: "Это похоже на вопрос, а не на путь. Для анализа введите путь к проекту в поле ввода (например ./papa-yu или полный путь к папке)." },
        ]);
      }
      setMessages((m) => [...m, { role: "user", text: paths.length ? `Анализ: ${paths.join(", ")}` : "Анализ проекта" }]);
    }
    else setMessages((m) => [...m, { role: "user", text: "Применить изменения" }]);
    if (confirmApply) setPendingPreview(null);
    try {
      if (confirmApply) setMessages((m) => [...m, { role: "system", text: "Применяю изменения пакетом…" }]);
      const events = await runBatchCmd({
        paths,
        confirm_apply: confirmApply,
        auto_check: autoCheck,
        selected_actions: actionsToApply.length ? actionsToApply : undefined,
        user_confirmed: userConfirmed ?? confirmApply,
        attached_files: attachedFiles.length ? attachedFiles : undefined,
      });
      for (const ev of events) {
        if (ev.kind === "report" && ev.report) {
          setLastReport(ev.report);
          setLastPath(ev.report.path);
          setLastReportJson(JSON.stringify(ev.report, null, 2));
          try {
            const p = await getProjectProfile(ev.report.path);
            setProfile(p);
          } catch (_) {
            setProfile(null);
          }
          setSelectedActions(ev.report.actions || []);
          setSuggestedActions([]);
          setSelectedActionIdx({});
          const groups = ev.report.action_groups ?? [];
          const initial: Record<string, boolean> = {};
          for (const g of groups) initial[g.id] = true;
          setSelectedFixGroupIds(initial);
          const packs = ev.report.fix_packs ?? [];
          const rec = ev.report.recommended_pack_ids ?? [];
          const packInit: Record<string, boolean> = {};
          for (const p of packs) packInit[p.id] = rec.includes(p.id);
          setSelectedPackIds(packInit);
          const x = ev.report.findings?.length ?? 0;
          const y = (ev.report.actions?.length ?? 0);
          setMessages((m) => [
            ...m,
            {
              role: "assistant",
              text: `Нашёл ${x} проблем. Могу исправить ${y}.`,
              report: ev.report,
            },
          ]);
        } else if (ev.kind === "preview" && ev.preview) {
          setPendingPreview({
            path: lastPath || paths[0] || ".",
            actions: actionsToApply.length ? actionsToApply : lastReport?.actions || [],
            diffs: ev.preview.diffs,
          });
          setMessages((m) => [
            ...m,
            {
              role: "assistant",
              text: `Предпросмотр: ${ev.preview!.summary}`,
              preview: ev.preview,
            },
          ]);
          if (ev.preview.diffs.some((d) => (d.summary || "").includes("BLOCKED"))) {
            setMessages((m) => [...m, { role: "system", text: "Некоторые изменения заблокированы политикой (защищённые/не-текстовые файлы)." }]);
          }
        } else if (ev.kind === "apply" && ev.apply_result) {
          const r = ev.apply_result;
          setPendingPreview(null);
          if (ev.undo_available !== undefined) setUndoAvailable(!!ev.undo_available);
          const isAutoRollback = r.error_code === "AUTO_ROLLBACK_DONE";
          const isReverted = r.error_code === "AUTO_CHECK_FAILED_REVERTED" || r.error_code === "AUTO_CHECK_FAILED_ROLLED_BACK";
          if (isAutoRollback) {
            setMessages((m) => [
              ...m,
              { role: "system", text: "Обнаружены ошибки. Откатываю изменения…", applyResult: r },
              { role: "system", text: "Изменения привели к ошибкам, откат выполнен." },
              { role: "assistant", text: "Изменения привели к ошибкам, откат выполнен." },
            ]);
          } else {
            const code = r.error_code || "";
            let systemText = isReverted ? "Ошибки после изменений. Откат выполнен." : (r.error || (r.ok ? "Применено." : "Ошибка."));
            if (code === "CONFIRM_REQUIRED") systemText = "Подтверждение обязательно перед применением.";
            else if (code === "PROTECTED_PATH") systemText = "Изменения отклонены: попытка изменить защищённые/не-текстовые файлы.";
            setMessages((m) => [
              ...m,
              { role: "system", text: systemText, applyResult: r },
              {
                role: "assistant",
                text: r.ok
                  ? "Изменения применены. Проверьте проект (тесты/сборка)."
                  : (isReverted ? "Ошибки после изменений. Откат выполнен." : code === "CONFIRM_REQUIRED" ? "Подтверждение обязательно перед применением." : code === "PROTECTED_PATH" ? "Изменения отклонены: защищённые/не-текстовые файлы." : r.error || "Ошибка применения."),
              },
            ]);
          }
          refreshUndoRedo();
        }
      }
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
    } finally {
      setLoading(false);
    }
  };

  /** Отправить: если ввод — команда/задача (не путь), в первую очередь выполнить её через ИИ; иначе — анализ. */
  const onAnalyze = () => {
    const text = input.trim();
    if (!text) {
      runBatch(false, []);
      return;
    }
    if (inputLooksLikePath(text)) {
      runBatch(false, []);
      return;
    }
    const pathToUse = lastPath || folderLinks[0] || pathsToUse()[0];
    if (pathToUse && pathToUse !== ".") {
      setAgentGoal(text);
      setInput("");
      if (!lastPath) setLastPath(pathToUse);
      handleProposeFixes(text, pathToUse, lastReportJson ?? "{}");
      return;
    }
    if (pathToUse === ".") {
      setAgentGoal(text);
      setInput("");
      handleProposeFixes(text, ".", lastReportJson ?? "{}");
      return;
    }
    setMessages((m) => [
      ...m,
      { role: "system", text: "Укажите папку проекта (скрепка → Папки или введите путь) и повторите команду." },
    ]);
    runBatch(false, []);
  };

  const onShowFixes = () => {
    if (!lastReport?.actions?.length) return;
    setSelectedActions([...lastReport.actions]);
    runBatch(false, lastReport.actions);
  };

  /** v2.9.1: один клик — применить все рекомендованные исправления из отчёта (confirm=true, auto_check=true) */
  const onApplyFixes = () => {
    if (!lastReport?.actions?.length || !lastReport?.path) return;
    setSelectedActions([...lastReport.actions]);
    runBatch(true, lastReport.actions, [lastReport.path]);
  };

  function collectSelectedActions(report: AnalyzeReport | null, selected: Record<string, boolean>): Action[] {
    const groups = report?.action_groups ?? [];
    const actions: Action[] = [];
    for (const g of groups) {
      if (selected[g.id]) actions.push(...(g.actions ?? []));
    }
    return actions;
  }

  function collectGroupIdsFromPacks(report: AnalyzeReport | null, selected: Record<string, boolean>): string[] {
    const packs = report?.fix_packs ?? [];
    const ids = new Set<string>();
    for (const p of packs) {
      if (selected[p.id]) for (const gid of p.group_ids ?? []) ids.add(gid);
    }
    return Array.from(ids);
  }

  function collectActionsByGroupIds(report: AnalyzeReport | null, groupIds: string[]): Action[] {
    const groups = report?.action_groups ?? [];
    const map = new Map<string, ActionGroup>();
    for (const g of groups) map.set(g.id, g);
    const actions: Action[] = [];
    for (const id of groupIds) {
      const g = map.get(id);
      if (g?.actions?.length) actions.push(...g.actions);
    }
    return actions;
  }

  const handlePreview = (path: string | null, actions: Action[]) => {
    if (!actions.length) return;
    const p = path ? [path] : pathsToUse();
    runBatch(false, actions, p);
  };

  /** v3.1: применить через apply_actions_tx (snapshot + autocheck + rollback) */
  const applyActionsTx = async (path: string, actions: Action[], useAutoCheck = true) => {
    setLoading(true);
    try {
      const res = await apiApplyActionsTx(path, actions, {
        auto_check: useAutoCheck,
        user_confirmed: true,
      });
      if (res.ok) {
        setMessages((m) => [...m, { role: "system", text: "Изменения применены. Проверки пройдены." }]);
        setPendingPreview(null);
        await refreshUndoRedo();
      } else {
        const code = res.error_code || "";
        if (code === "CONFIRM_REQUIRED") {
          setMessages((m) => [...m, { role: "system", text: "Подтверждение обязательно перед применением." }]);
        } else if (code === "PROTECTED_PATH") {
          setMessages((m) => [...m, { role: "system", text: "Изменения отклонены: попытка изменить защищённые/не-текстовые файлы." }]);
        } else if (code === "AUTO_CHECK_FAILED_ROLLED_BACK") {
          setMessages((m) => [...m, { role: "system", text: "Изменения привели к ошибкам, откат выполнен." }]);
        } else {
          setMessages((m) => [...m, { role: "system", text: res.error || res.error_code || "Ошибка применения." }]);
        }
      }
      return res;
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
      return { ok: false, applied: false, rolled_back: false, checks: [] } as ApplyTxResult;
    } finally {
      setLoading(false);
    }
  };

  /** v3.1: предпросмотр + применить всё безопасное (с autocheck) */
  const applyAllSafe = async (projectPath: string, actions: Action[]) => {
    setMessages((m) => [...m, { role: "system", text: "Предпросмотр изменений…" }]);
    await handlePreview(projectPath, actions);
    setMessages((m) => [...m, { role: "system", text: "Применяю изменения…" }]);
    await applyActionsTx(projectPath, actions, true);
  };

  /** Применить изменения с отображением процесса в диалоге */
  const applyWithProgressDialog = async (path: string, actions: Action[]) => {
    setApplyProgressVisible(true);
    setApplyProgressLog(["Подготовка…"]);
    setApplyResult(null);
    applyingRef.current = true;
    try {
      const res = await apiApplyActionsTx(path, actions, {
        auto_check: autoCheck,
        user_confirmed: true,
      });
      setApplyResult(res);
      setApplyProgressLog((prev) => [
        ...prev,
        res.ok ? "Готово. Изменения применены." : (res.error || "Ошибка"),
      ]);
      if (res.ok) {
        setMessages((m) => [...m, { role: "system", text: "Изменения применены. Проверки пройдены." }]);
        setPendingPreview(null);
        setPendingActions(null);
        setPendingActionIdx({});
        await refreshUndoRedo();
      } else {
        const code = res.error_code || "";
        if (code === "CONFIRM_REQUIRED") {
          setMessages((m) => [...m, { role: "system", text: "Подтверждение обязательно перед применением." }]);
        } else if (code === "AUTO_CHECK_FAILED_ROLLED_BACK") {
          setMessages((m) => [...m, { role: "system", text: "Изменения привели к ошибкам, откат выполнен." }]);
        } else {
          setMessages((m) => [...m, { role: "system", text: res.error || res.error_code || "Ошибка применения." }]);
        }
      }
    } catch (e) {
      const err = String(e);
      setApplyProgressLog((prev) => [...prev, `Ошибка: ${err}`]);
      setApplyResult({ ok: false, applied: false, rolled_back: false, checks: [], error: err } as ApplyTxResult);
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${err}` }]);
    } finally {
      applyingRef.current = false;
    }
  };

  const handleApplyFixesWithActions = (path: string | null, actions: Action[]) => {
    if (!actions.length) return;
    if (path) {
      const ok = window.confirm(`Применить ${actions.length} изменений к проекту?`);
      if (!ok) return;
      applyWithProgressDialog(path, actions);
      return;
    }
    const p = pathsToUse();
    runBatch(true, actions, p);
  };

  const onApplyPending = () => {
    if (!pendingPreview) return;
    const ok = window.confirm("Применить изменения к проекту? Это изменит файлы на диске.");
    if (!ok) return;
    if (lastPath) {
      applyWithProgressDialog(lastPath, pendingPreview.actions);
      return;
    }
    const paths = pathsToUse();
    runBatch(true, pendingPreview.actions, paths, true);
  };

  const onCancelPending = () => {
    setPendingPreview(null);
    setMessages((m) => [...m, { role: "system", text: "Предпросмотр отменён. Ничего не изменено. Можно отправить новый запрос — введите путь или цель и нажмите «Отправить»." }]);
  };

  /** Сохранить текущий диалог в историю и переключиться на новый запрос. Контекст старого запроса сбрасывается, чтобы следующий выполнялся как новый. */
  const onNewRequest = () => {
    if (messages.length > 0) {
      const title = messages.find((m) => m.role === "user")?.text?.slice(0, 45) || "Запрос";
      setRequestHistory((prev) => [
        ...prev,
        { id: String(Date.now()), title: title + (title.length >= 45 ? "…" : ""), messages: [...messages], lastPath, lastReport },
      ]);
    }
    setInput("");
    setAgentGoal("");
    setLastReport(null);
    setLastReportJson(null);
    setPendingPreview(null);
    setPendingActions(null);
    setPendingActionIdx({});
    setAgenticResult(null);
    setAgenticProgress(null);
    setVerifyResult(null);
    setMessages((m) => [...m, { role: "system", text: "Готов к новому запросу. Введите путь или задачу и нажмите «Отправить»." }]);
  };

  /** Вернуться к обсуждению выбранного запроса из истории. */
  const switchToRequest = (item: { id: string; title: string; messages: ChatMessage[]; lastPath: string | null; lastReport: AnalyzeReport | null }) => {
    setMessages(item.messages);
    setLastPath(item.lastPath);
    setLastReport(item.lastReport);
    setPendingPreview(null);
    setPendingActions(null);
    setPendingActionIdx({});
  };

  /** Удалить чат из истории. */
  const removeFromHistory = (id: string) => {
    setRequestHistory((prev) => prev.filter((item) => item.id !== id));
  };

  /** Список для отображения: текущий запрос (если есть сообщения) + история. */
  const displayRequests: { id: string; title: string; isCurrent?: boolean; item?: typeof requestHistory[0] }[] = [];
  if (messages.length > 0) {
    const currentTitle = messages.find((m) => m.role === "user")?.text?.slice(0, 45) || "Текущий запрос";
    displayRequests.push({ id: "current", title: currentTitle + (currentTitle.length >= 45 ? "…" : ""), isCurrent: true });
  }
  requestHistory.forEach((item) => displayRequests.push({ id: item.id, title: item.title, item }));

  /** v3.3: один клик — generate → preview → apply (без показа списка) */
  const handleOneClickFix = async () => {
    if (!lastPath || !lastReport) return;
    setLoading(true);
    setMessages((m) => [...m, { role: "system", text: "Формирую безопасные исправления…" }]);
    try {
      const res = await generateActionsFromReport(
        lastPath,
        lastReport,
        createOnlyMode ? "safe_create_only" : "safe"
      );
      if (!res.ok || res.actions.length === 0) {
        setMessages((m) => [...m, { role: "assistant", text: res.error ?? res.actions.length === 0 ? "Нет безопасных правок." : "Ошибка генерации." }]);
        return;
      }
      setSuggestedActions(res.actions);
      const allSelected: Record<number, boolean> = {};
      res.actions.forEach((_, i) => { allSelected[i] = true; });
      setSelectedActionIdx(allSelected);
      setMessages((m) => [...m, { role: "assistant", text: "Предпросмотр изменений" }]);
      await handlePreview(lastPath, res.actions);
      setMessages((m) => [...m, { role: "system", text: "Применяю…" }]);
      const applyRes = await applyActionsTx(lastPath, res.actions, true);
      if (applyRes.ok) {
        setMessages((m) => [...m, { role: "assistant", text: "Готово. Проверки пройдены." }]);
      } else if (applyRes.error_code === "AUTO_CHECK_FAILED_ROLLED_BACK") {
        setMessages((m) => [...m, { role: "assistant", text: "Откат выполнен." }]);
      } else {
        setMessages((m) => [...m, { role: "assistant", text: applyRes.error ?? "Ошибка применения." }]);
      }
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
    } finally {
      setLoading(false);
    }
  };

  /** v2.4: Agentic Run — analyze → plan → preview → apply → verify → auto-rollback → retry */
  const handleAgenticRun = async () => {
    if (!lastPath) return;
    setAgenticRunning(true);
    setAgenticResult(null);
    setAgenticProgress(null);
    setMessages((m) => [...m, { role: "user", text: "Исправить проект автоматически" }]);
    try {
      const payload: AgenticRunRequest = {
        path: lastPath,
        goal: "Исправь критические проблемы и улучшай качество проекта",
        constraints: { auto_check: true, max_attempts: 2, max_actions: 12 },
      };
      const result = await agenticRun(payload);
      setAgenticResult(result);
      setMessages((m) => [
        ...m,
        { role: "assistant", text: result.final_summary },
      ]);
      try {
        const projects = await listProjects();
        let projectId = projects.find((p) => p.path === lastPath)?.id;
        if (!projectId) {
          const added = await addProject(lastPath, null);
          projectId = added.id;
        }
        await appendSessionEvent(projectId, "agentic_run", "assistant", result.final_summary);
        const list = await listSessions(projectId);
        setSessions(list);
      } catch (_) {}
      await refreshUndoRedo();
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
    } finally {
      setAgenticRunning(false);
      setAgenticProgress(null);
    }
  };

  /** v3.2: исправить автоматически (безопасно) — generate_actions_from_report → список с чекбоксами */
  const handleFixAuto = async () => {
    if (!lastPath || !lastReport) return;
    setIsGeneratingActions(true);
    setMessages((m) => [...m, { role: "system", text: "Формирую безопасные исправления…" }]);
    try {
      const res = await generateActionsFromReport(
        lastPath,
        lastReport,
        createOnlyMode ? "safe_create_only" : "safe"
      );
      if (!res.ok) {
        setMessages((m) => [...m, { role: "assistant", text: res.error ?? res.error_code ?? "Ошибка генерации" }]);
        return;
      }
      setSuggestedActions(res.actions);
      const allSelected: Record<number, boolean> = {};
      res.actions.forEach((_, i) => { allSelected[i] = true; });
      setSelectedActionIdx(allSelected);
      const summary = res.actions.length
        ? `Предложено ${res.actions.length} действий. Выберите и примените.`
        : "Нет безопасных правок для автоматического применения.";
      if (res.skipped.length) {
        setMessages((m) => [...m, { role: "assistant", text: `${summary} Пропущено: ${res.skipped.join(", ")}` }]);
      } else {
        setMessages((m) => [...m, { role: "assistant", text: summary }]);
      }
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
    } finally {
      setIsGeneratingActions(false);
    }
  };

  /** Выбранные действия из suggestedActions по selectedActionIdx */
  const getSelectedSuggestedActions = (): Action[] =>
    suggestedActions.filter((_, i) => selectedActionIdx[i] !== false);

  /** Выбранные рекомендации ИИ из pendingActions по pendingActionIdx */
  const getSelectedPendingActions = (): Action[] =>
    (pendingActions ?? []).filter((_, i) => pendingActionIdx[i] !== false);

  /** Собрать контекст трендов для ИИ: ИИ использует его самостоятельно при предложениях. */
  const getTrendsContextForAI = (): string | undefined => {
    if (!trends?.recommendations?.length) return undefined;
    return trends.recommendations
      .map((r) => `• ${r.title}${r.summary ? `: ${r.summary}` : ""}`)
      .join("\n");
  };

  /** v3.0: предложить исправления (агент) → план по цели. ИИ в первую очередь выполняет команду пользователя. path и reportJson можно передать явно (при вводе команды без предварительного анализа). */
  const handleProposeFixes = async (overrideGoal?: string, overridePath?: string, overrideReportJson?: string) => {
    const pathToUse = overridePath ?? lastPath;
    const reportToUse = overrideReportJson ?? lastReportJson ?? "{}";
    if (!pathToUse) return;
    const goal = (overrideGoal ?? agentGoal).trim() || "Повысить качество проекта и привести структуру к стандарту";
    if (goal) setMessages((m) => [...m, { role: "user", text: goal }]);
    setMessages((m) => [...m, { role: "system", text: "Выполняю команду…" }]);
    setLoading(true);
    try {
      let trendsContext = getTrendsContextForAI();
      if (!trendsContext && !trends) {
        try {
          const t = await getTrendsRecommendations();
          setTrends(t);
          trendsContext = t.recommendations?.length
            ? t.recommendations.map((r) => `• ${r.title}${r.summary ? `: ${r.summary}` : ""}`).join("\n")
            : undefined;
        } catch (_) {}
      }
      const plan = await proposeActions(
        pathToUse,
        reportToUse,
        goal,
        designStyle.trim() || undefined,
        trendsContext,
        lastPlanJson ?? undefined,
        lastPlanContext ?? undefined
      );
      if (!plan.ok) {
        setMessages((m) => [...m, { role: "assistant", text: plan.error ?? "Ошибка формирования плана" }]);
        return;
      }
      // Сохраняем план и контекст для Apply (когда пользователь напишет "ok" или "применяй")
      if (plan.plan_json) {
        setLastPlanJson(plan.plan_json);
        setLastPlanContext(plan.plan_context ?? null);
      } else {
        setLastPlanJson(null);
        setLastPlanContext(null);
      }
      const actionLines = plan.actions.length
        ? "\n\nПлан действий:\n" + plan.actions.map((a) => `• ${a.kind}: ${a.path}`).join("\n")
        : "";
      setMessages((m) => [...m, { role: "assistant", text: plan.summary + actionLines }]);
      if (plan.actions.length > 0) {
        setPendingActions(plan.actions);
        try {
          const preview = await previewActions(pathToUse, plan.actions);
          setPendingPreview({ path: pathToUse, actions: plan.actions, diffs: preview.diffs });
        } catch (_) {
          setPendingPreview(null);
        }
      } else {
        setPendingActions(null);
        setPendingPreview(null);
      }
      setPendingActionIdx({});
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка: ${String(e)}` }]);
    } finally {
      setLoading(false);
    }
  };

  const hasPendingPreview = !!pendingPreview;

  const handleDownloadReport = () => {
    if (!lastReport) return;
    const blob = new Blob([JSON.stringify(lastReport, null, 2)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "report.json";
    a.click();
    URL.revokeObjectURL(a.href);
  };

  /** Обновить тренды и рекомендации (мониторинг не реже раз в месяц). */
  const handleFetchTrends = async () => {
    setTrendsLoading(true);
    try {
      const res = await fetchTrendsRecommendations();
      setTrends(res);
    } catch (_) {
      setTrends(null);
    } finally {
      setTrendsLoading(false);
    }
  };

  /** Проверка целостности проекта (типы, сборка, тесты). Вызывается автоматически после применений или вручную. */
  const handleVerifyIntegrity = async () => {
    if (!lastPath) return;
    setVerifying(true);
    setVerifyResult(null);
    try {
      const res = await verifyProject(lastPath);
      setVerifyResult(res);
      const msg = res.ok
        ? `Проверка целостности: всё в порядке (${res.checks.length} проверок).`
        : `Проверка целостности: обнаружены ошибки. ${res.error ?? ""}`;
      setMessages((m) => [...m, { role: "system", text: msg }]);
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка проверки: ${String(e)}` }]);
    } finally {
      setVerifying(false);
    }
  };

  const handleDownloadDiff = () => {
    if (!agenticResult?.attempts?.length) return;
    const last = agenticResult.attempts[agenticResult.attempts.length - 1];
    if (!last?.preview?.diffs?.length) return;
    const lines = last.preview.diffs.map((d) =>
      `--- ${d.path}\n+++ ${d.path}\n${(d.old_content ?? "") ? `- ${(d.old_content ?? "").split("\n").join("\n- ")}\n` : ""}${(d.new_content ?? "") ? `+ ${(d.new_content ?? "").split("\n").join("\n+ ")}` : ""}`
    );
    const blob = new Blob([lines.join("\n\n")], { type: "text/plain" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "changes.diff";
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const handleExportSettings = async () => {
    try {
      const json = await exportSettings();
      const blob = new Blob([json], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `papa-yu-settings-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      setMessages((m) => [...m, { role: "system", text: "Настройки экспортированы в файл." }]);
    } catch (e) {
      setMessages((m) => [...m, { role: "system", text: `Ошибка экспорта: ${e}` }]);
    }
  };

  const handleImportSettings = () => {
    // Create hidden file input
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,application/json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      
      try {
        const json = await file.text();
        const result = await importSettings(json, "merge");
        setMessages((m) => [
          ...m,
          {
            role: "system",
            text: `Импортировано: ${result.projects_imported} проектов, ${result.profiles_imported} профилей, ${result.sessions_imported} сессий, ${result.folder_links_imported} папок.`,
          },
        ]);
        // Reload folder links
        const links = await getFolderLinks();
        if (links.paths?.length) setFolderLinks(links.paths);
      } catch (err) {
        setMessages((m) => [...m, { role: "system", text: `Ошибка импорта: ${err}` }]);
      }
    };
    input.click();
  };

  return (
    <div style={{ display: "flex", minHeight: "100vh", overflow: "visible" }}>
      {/* Левая панель: запросы и кнопки */}
      <aside
        style={{
          width: 220,
          minWidth: 220,
          padding: "16px 12px",
          borderRight: "1px solid var(--color-border)",
          background: isDark 
            ? "linear-gradient(180deg, var(--color-surface) 0%, var(--color-bg) 100%)" 
            : "linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%)",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
        }}
      >
        <button
          type="button"
          onClick={onNewRequest}
          style={{
            padding: "10px 14px",
            background: "var(--color-primary)",
            color: "#fff",
            border: "none",
            borderRadius: "var(--radius-md)",
            fontWeight: 600,
            fontSize: "13px",
            boxShadow: "0 2px 6px rgba(37, 99, 235, 0.3)",
          }}
        >
          Новый запрос
        </button>
        <button
          type="button"
          onClick={() => { setTrendsModalOpen(true); setSelectedRecommendation(null); if (!trends) handleFetchTrends(); }}
          style={{
            padding: "10px 14px",
            background: "#1e40af",
            color: "#fff",
            border: "none",
            borderRadius: "var(--radius-md)",
            fontWeight: 600,
            fontSize: "13px",
            boxShadow: "0 2px 6px rgba(30, 64, 175, 0.3)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "8px",
          }}
          title="Тренды и рекомендации"
        >
          <img src="/send-icon.png" alt="" style={{ height: "20px", width: "auto", objectFit: "contain" }} />
          Тренды и рекомендации
        </button>
        {displayRequests.length > 0 && (
          <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--color-text-muted)", marginBottom: "4px", marginTop: "8px" }}>
            Запросы
          </div>
        )}
        {displayRequests.map((entry) => (
          <div
            key={entry.id}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "4px 0",
            }}
          >
            <button
              type="button"
              onClick={() => !entry.isCurrent && entry.item && switchToRequest(entry.item)}
              style={{
                flex: 1,
                minWidth: 0,
                padding: "8px 12px",
                background: entry.isCurrent ? "#e0e7ff" : "#fff",
                border: `1px solid ${entry.isCurrent ? "#818cf8" : "var(--color-border)"}`,
                borderRadius: "var(--radius-md)",
                fontSize: "12px",
                textAlign: "left",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                cursor: entry.isCurrent ? "default" : "pointer",
                boxShadow: "var(--shadow-sm)",
                fontWeight: entry.isCurrent ? 600 : 400,
              }}
              title={entry.isCurrent ? "Текущее обсуждение" : entry.title}
            >
              {entry.isCurrent ? "● " : ""}{entry.title}
            </button>
            {!entry.isCurrent && entry.item && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); removeFromHistory(entry.id); }}
                title="Удалить чат из истории"
                style={{
                  padding: "6px 8px",
                  background: "#fef2f2",
                  border: "1px solid #fecaca",
                  borderRadius: "var(--radius-md)",
                  cursor: "pointer",
                  color: "#dc2626",
                  fontSize: "14px",
                  lineHeight: 1,
                  flexShrink: 0,
                }}
              >
                🗑
              </button>
            )}
          </div>
        ))}
        {/* Spacer */}
        <div style={{ flex: 1 }} />
        {/* Settings Export/Import */}
        <div style={{ display: "flex", gap: "6px", marginBottom: "8px" }}>
          <button
            type="button"
            onClick={handleExportSettings}
            title="Экспорт настроек"
            style={{
              flex: 1,
              padding: "8px",
              background: isDark ? "var(--color-surface)" : "#fff",
              border: "1px solid var(--color-border)",
              borderRadius: "var(--radius-md)",
              cursor: "pointer",
              fontSize: "12px",
              color: "var(--color-text-muted)",
            }}
          >
            📤 Экспорт
          </button>
          <button
            type="button"
            onClick={handleImportSettings}
            title="Импорт настроек"
            style={{
              flex: 1,
              padding: "8px",
              background: isDark ? "var(--color-surface)" : "#fff",
              border: "1px solid var(--color-border)",
              borderRadius: "var(--radius-md)",
              cursor: "pointer",
              fontSize: "12px",
              color: "var(--color-text-muted)",
            }}
          >
            📥 Импорт
          </button>
        </div>
        {/* Theme Toggle */}
        <button
          type="button"
          onClick={toggleTheme}
          className="theme-toggle"
          title={isDark ? "Светлая тема" : "Тёмная тема"}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            padding: "10px 12px",
            background: isDark ? "var(--color-surface)" : "#fff",
            border: "1px solid var(--color-border)",
            borderRadius: "var(--radius-md)",
            cursor: "pointer",
            fontSize: "13px",
            color: "var(--color-text-muted)",
            width: "100%",
            justifyContent: "center",
          }}
        >
          <span style={{ fontSize: "16px" }}>{isDark ? "☀️" : "🌙"}</span>
          {isDark ? "Светлая тема" : "Тёмная тема"}
        </button>
      </aside>

      <main style={{ flex: 1, maxWidth: 720, margin: "0 auto", padding: "0 16px", overflow: "visible" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "20px",
          marginBottom: "28px",
          padding: "24px",
          background: "linear-gradient(135deg, #fff 0%, var(--color-bg-warm) 100%)",
          borderRadius: "var(--radius-xl)",
          boxShadow: "var(--shadow-md)",
          border: "1px solid var(--color-border)",
        }}
      >
        <img src="/logo.png" alt="PAPAYU" style={{ height: "120px", width: "auto", objectFit: "contain", filter: "drop-shadow(0 2px 4px rgba(0,0,0,0.06))" }} />
        <div>
          <h1 style={{ fontSize: "26px", marginBottom: "6px", fontWeight: 700, color: "#1e3a5f", letterSpacing: "-0.02em" }}>PAPA YU</h1>
          <p style={{ fontSize: "14px", color: "var(--color-text-muted)", margin: 0, fontWeight: 500 }}>инженерная система с контролем качества и эксплуатационными инструментами</p>
        </div>
      </div>

      {profile && (
        <div style={{ marginBottom: "10px", fontSize: "12px", opacity: 0.9, color: "var(--color-text-muted)", padding: "8px 12px", background: "var(--color-bg-warm)", borderRadius: "var(--radius-md)", border: "1px solid var(--color-border)" }}>
          Профиль: {profile.project_type} · Safe Mode · Attempts: {profile.max_attempts}
          {profile.limits && (
            <> · Лимиты: {profile.limits.max_actions_per_tx} действий/транзакция, таймаут {profile.limits.timeout_sec} с</>
          )}
        </div>
      )}
      {lastPath && (
        <div style={{ marginBottom: "14px" }}>
          <button
            type="button"
            onClick={handleVerifyIntegrity}
            disabled={verifying}
            style={{
              padding: "10px 18px",
              background: verifying ? "var(--color-bg)" : "var(--color-secondary)",
              color: verifying ? "var(--color-text-soft)" : "#fff",
              border: "none",
              borderRadius: "var(--radius-md)",
              fontWeight: 600,
              boxShadow: verifying ? "none" : "0 2px 6px rgba(13, 148, 136, 0.3)",
            }}
          >
            {verifying ? "Проверяю…" : "Проверить целостность"}
          </button>
          <span style={{ marginLeft: "10px", fontSize: "13px", color: "var(--color-text-muted)" }}>
            Автоматическая проверка типов, сборки и тестов после изменений
          </span>
        </div>
      )}
      {verifyResult && (
        <section style={{ marginBottom: "16px", padding: "14px", background: verifyResult.ok ? "#ecfdf5" : "#fef2f2", borderRadius: "var(--radius-lg)", border: `1px solid ${verifyResult.ok ? "#a7f3d0" : "#fecaca"}` }}>
          <h3 style={{ fontSize: "14px", fontWeight: 700, color: verifyResult.ok ? "#065f46" : "#991b1b", marginBottom: "8px" }}>
            {verifyResult.ok ? "Целостность в порядке" : "Обнаружены ошибки"}
          </h3>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, fontSize: "13px" }}>
            {verifyResult.checks.map((c, i) => (
              <li key={i} style={{ marginBottom: "6px", display: "flex", alignItems: "flex-start", gap: "8px", flexDirection: "column" }}>
                <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontWeight: 600, color: c.ok ? "#059669" : "#dc2626" }}>{c.ok ? "✓" : "✗"} {c.name}</span>
                {!c.ok && c.output && (
                  <pre style={{ margin: "4px 0 0 0", padding: "8px", background: "#fff", borderRadius: "6px", fontSize: "11px", overflow: "auto", maxHeight: "120px", whiteSpace: "pre-wrap" }}>{c.output}</pre>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
      {/* Инлайн-запрос к ИИ: постановка задачи и ответ с вариантами в этом же окне */}
      {lastPath && lastReport && (
        <section
          style={{
            marginBottom: "18px",
            padding: "18px 20px",
            background: "linear-gradient(135deg, #f5f3ff 0%, #ede9fe 100%)",
            borderRadius: "var(--radius-lg)",
            border: "1px solid #c4b5fd",
            boxShadow: "var(--shadow-sm)",
          }}
        >
          <h3 style={{ fontSize: "15px", fontWeight: 700, color: "#5b21b6", marginBottom: "8px", letterSpacing: "-0.01em" }}>
            Запрос к ИИ
          </h3>
          <p style={{ fontSize: "13px", color: "#6d28d9", marginBottom: "12px" }}>
            Опишите задачу. При создании программ можно выбрать стиль дизайна (ИИ или сторонние: Material, Tailwind/shadcn, Bootstrap). Ответ и варианты появятся ниже.
          </p>
          <div style={{ marginBottom: "12px" }}>
            <label style={{ fontSize: "13px", fontWeight: 600, color: "#5b21b6", marginRight: "8px" }}>Стиль дизайна:</label>
            <select
              value={designStyle}
              onChange={(e) => setDesignStyle(e.target.value)}
              style={{
                padding: "8px 12px",
                border: "1px solid #a78bfa",
                borderRadius: "var(--radius-md)",
                fontSize: "13px",
                background: "#fff",
                minWidth: "200px",
              }}
            >
              <option value="">По умолчанию (ИИ)</option>
              <option value="Material Design">Material Design</option>
              <option value="Tailwind / shadcn/ui">Tailwind / shadcn/ui</option>
              <option value="Bootstrap">Bootstrap</option>
              <option value="Сторонние ресурсы (UI-библиотеки)">Сторонние ресурсы</option>
            </select>
          </div>
          <div style={{ display: "flex", gap: "10px", flexWrap: "wrap", alignItems: "flex-start" }}>
            <textarea
              value={agentGoal}
              onChange={(e) => setAgentGoal(e.target.value)}
              placeholder="Например: добавь README, создай проект с нуля, настрой линтер…"
              rows={2}
              style={{
                flex: "1 1 280px",
                minWidth: "200px",
                padding: "12px 14px",
                border: "1px solid #a78bfa",
                borderRadius: "var(--radius-md)",
                fontSize: "14px",
                resize: "vertical",
                boxSizing: "border-box",
                background: "#fff",
              }}
            />
            <button
              type="button"
              onClick={() => handleProposeFixes()}
              disabled={loading}
              style={{
                padding: "12px 20px",
                background: "var(--color-accent)",
                color: "#fff",
                border: "none",
                borderRadius: "var(--radius-md)",
                fontWeight: 600,
                boxShadow: "0 2px 8px rgba(124, 58, 237, 0.35)",
                alignSelf: "flex-end",
              }}
            >
              {loading ? "…" : "Получить рекомендации"}
            </button>
          </div>
        </section>
      )}
      {lastPath && sessions.length > 0 && (
        <section style={{ marginBottom: "14px", padding: "14px", background: "var(--color-bg-warm)", borderRadius: "var(--radius-lg)", border: "1px solid var(--color-border)", boxShadow: "var(--shadow-sm)" }}>
          <button
            type="button"
            onClick={() => setSessionsExpanded(!sessionsExpanded)}
            style={{ display: "flex", alignItems: "center", gap: "6px", background: "none", border: "none", cursor: "pointer", fontSize: "13px", fontWeight: 600, color: "#334155" }}
          >
            {sessionsExpanded ? "▼" : "▶"} История сессий ({sessions.length})
          </button>
          {sessionsExpanded && (
            <ul style={{ listStyle: "none", padding: "8px 0 0 0", margin: 0, fontSize: "12px", color: "#64748b" }}>
              {sessions.slice(0, 10).map((s) => (
                <li key={s.id} style={{ marginBottom: "6px", padding: "6px 8px", background: "#fff", borderRadius: "6px", border: "1px solid #e2e8f0" }}>
                  <span title={s.updated_at}>{new Date(s.updated_at).toLocaleString()}</span>
                  {s.events.length > 0 && (
                    <span style={{ marginLeft: "8px" }}>— {s.events.length} событий</span>
                  )}
                  {s.events.length > 0 && s.events[s.events.length - 1]?.text && (
                    <div style={{ marginTop: "4px", fontSize: "11px", color: "#94a3b8", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={s.events[s.events.length - 1].text}>
                      {s.events[s.events.length - 1].text}
                    </div>
                  )}
                </li>
              ))}
              {sessions.length > 10 && <li style={{ color: "#94a3b8" }}>… ещё {sessions.length - 10}</li>}
            </ul>
          )}
        </section>
      )}
      <section
        aria-label="Диалог с ИИ"
        style={{
          border: "2px solid var(--color-accent)",
          borderRadius: "var(--radius-lg)",
          background: "linear-gradient(180deg, #faf5ff 0%, #fff 30%)",
          minHeight: "260px",
          padding: "0 0 20px 0",
          boxShadow: "var(--shadow-md)",
        }}
      >
        <div style={{ padding: "14px 20px", borderBottom: "2px solid var(--color-accent)", background: "linear-gradient(135deg, #7c3aed 0%, #5b21b6 100%)", borderRadius: "var(--radius-lg) var(--radius-lg) 0 0", color: "#fff" }}>
          <h2 style={{ margin: 0, fontSize: "18px", fontWeight: 700, letterSpacing: "-0.02em" }}>Диалог с ИИ</h2>
          <p style={{ margin: "6px 0 0 0", fontSize: "13px", opacity: 0.95 }}>Ответы анализа и ИИ-агента появляются ниже. Введите запрос в поле под заголовком.</p>
        </div>
        <div style={{ padding: "16px 20px", borderBottom: "1px solid #e9d5ff", background: "#faf5ff" }}>
          <p style={{ fontSize: "13px", color: "#6d28d9", marginBottom: "10px", fontWeight: 600 }}>Введите запрос</p>
          <div style={{ display: "flex", gap: "10px", flexWrap: "wrap", alignItems: "center", position: "relative" }}>
            <button
              type="button"
              onClick={() => setAttachmentMenuOpen((v) => !v)}
              title="Прикрепить файл, папку или архив"
              style={{
                padding: "12px 14px",
                background: attachmentMenuOpen ? "#e9d5ff" : "#fff",
                border: "2px solid #a78bfa",
                borderRadius: "var(--radius-md)",
                cursor: "pointer",
                fontSize: "18px",
                lineHeight: 1,
              }}
            >
              📎
            </button>
            {attachmentMenuOpen && (
              <div
                style={{
                  position: "absolute",
                  top: "100%",
                  left: 0,
                  marginTop: "4px",
                  padding: "8px",
                  background: "#fff",
                  border: "1px solid var(--color-border)",
                  borderRadius: "var(--radius-md)",
                  boxShadow: "var(--shadow-md)",
                  zIndex: 10,
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                  minWidth: "160px",
                }}
              >
                {[
                  { label: "Изображения", filter: "image" },
                  { label: "Файлы", filter: "file" },
                  { label: "Папки", filter: "folder" },
                  { label: "Архивы", filter: "archive" },
                ].map(({ label, filter }) => (
                  <button
                    key={filter}
                    type="button"
                    onClick={async () => {
                      setAttachmentMenuOpen(false);
                      if (filter === "folder") {
                        const selected = await open({ directory: true, multiple: true });
                        if (selected) {
                          const paths = (Array.isArray(selected) ? selected : [selected]) as string[];
                          setFolderLinks((prev) => {
                            const next = [...prev, ...paths];
                            saveLocalLinks(next);
                            syncFolderLinksToBackend(next);
                            return next;
                          });
                          if (paths.length) setLastPath(paths[0]);
                        }
                      } else {
                        const filters = filter === "image"
                          ? [{ name: "Изображения", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }]
                          : filter === "archive"
                            ? [{ name: "Архивы", extensions: ["zip", "tar", "gz", "tgz", "rar", "7z"] }]
                            : undefined;
                        const selected = await open({ multiple: true, filters });
                        if (selected) {
                          const paths = Array.isArray(selected) ? selected : [selected];
                          setAttachedFiles((prev) => [...prev, ...paths]);
                        }
                      }
                    }}
                    style={{ padding: "8px 12px", textAlign: "left", background: "none", border: "none", borderRadius: "6px", cursor: "pointer", fontSize: "13px" }}
                  >
                    {label}
                  </button>
                ))}
              </div>
            )}
            <input
              type="text"
              placeholder="Введите задачу"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              style={{
                flex: "1 1 260px",
                minWidth: "200px",
                padding: "12px 16px",
                border: "2px solid #a78bfa",
                borderRadius: "var(--radius-md)",
                background: "#fff",
                fontSize: "14px",
                boxSizing: "border-box",
              }}
            />
            <button
              type="button"
              onClick={onAnalyze}
              disabled={loading}
              title="Отправить"
              style={{
                padding: "10px 14px",
                background: "var(--color-accent)",
                color: "#fff",
                border: "none",
                borderRadius: "var(--radius-md)",
                fontWeight: 600,
                boxShadow: "0 2px 8px rgba(124, 58, 237, 0.35)",
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              {loading ? "…" : <img src="/send-icon.png" alt="Отправить" style={{ height: "24px", width: "auto", objectFit: "contain" }} />}
            </button>
          </div>
          <p style={{ fontSize: "12px", color: "#64748b", marginTop: "6px", marginBottom: 0 }}>Пиши как в чате: «сделай README», «добавь тесты», «создай проект с нуля» — агент поймёт.</p>
          <p style={{ fontSize: "12px", color: "#7c3aed", marginTop: "8px", marginBottom: 0 }}>Путь к папке → анализ. После анализа введите задачу или обратную команду для выполнения рекомендаций и нажмите кнопку отправки. Скрепка: прикрепить файлы, папки или архивы.</p>
        </div>
        <div style={{ padding: "20px 20px 0" }}>
        {messages.length === 0 && (
          <div style={{ padding: "24px 16px", textAlign: "center", color: "var(--color-text-muted)", fontSize: "14px", lineHeight: 1.7 }}>
            <p style={{ marginBottom: "14px", fontWeight: 600, color: "var(--color-text)", fontSize: "15px" }}>Всё в одном окне</p>
            <p style={{ margin: 0 }}>1. Введите путь к папке проекта в поле ввода ниже и нажмите «Отправить» — здесь появится отчёт.</p>
            <p style={{ margin: "10px 0 0 0" }}>2. В блоке «Запрос к ИИ» введите задачу (например «добавь README» или «создай проект с нуля») и нажмите «Получить рекомендации» — ИИ анализирует всё содержимое папки и даёт ответ с вариантами в этом же окне.</p>
            <p style={{ margin: "10px 0 0 0" }}>3. После изменений нажмите «Проверить целостность» для автоматической проверки типов, сборки и тестов.</p>
          </div>
        )}
        {messages.length > 0 && messages.map((msg, i) => (
          <div key={i} style={{ marginBottom: "16px", padding: "12px 14px", background: msg.role === "assistant" ? "#f8fafc" : msg.role === "system" ? "#f1f5f9" : "transparent", borderRadius: "var(--radius-md)", border: msg.role === "assistant" ? "1px solid #e2e8f0" : "none" }}>
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: "10px", flexWrap: "wrap" }}>
              <span style={{ fontWeight: 600, color: msg.role === "system" ? "#64748b" : msg.role === "user" ? "#2563eb" : "#0f172a" }}>
                {msg.role === "system" ? "Система" : msg.role === "user" ? "Вы" : "Ассистент"}:
              </span>
              {(msg.role === "assistant" || msg.role === "system") && msg.text && (
                <button
                  type="button"
                  onClick={() => { navigator.clipboard.writeText(msg.text); }}
                  style={{ padding: "4px 10px", fontSize: "12px", background: "#e2e8f0", border: "none", borderRadius: "6px", cursor: "pointer", fontWeight: 500 }}
                  title="Скопировать ответ"
                >
                  Копировать
                </button>
              )}
            </div>
            <div style={{ marginTop: "6px", whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: "14px", lineHeight: 1.5 }}>{msg.text}</div>
            {msg.preview && msg.preview.diffs && (() => {
              const diffs = msg.preview.diffs;
              const create = diffs.filter((d) => d.kind === "CreateFile" || d.kind === "CreateDir").length;
              const update = diffs.filter((d) => d.kind === "UpdateFile").length;
              const del = diffs.filter((d) => d.kind === "DeleteFile" || d.kind === "DeleteDir").length;
              const mkdir = diffs.filter((d) => d.kind === "CreateDir").length;
              const rmdir = diffs.filter((d) => d.kind === "DeleteDir").length;
              const label = (d: DiffItem) => {
                if (d.kind === "CreateDir") return `Создать папку ${d.path}`;
                if (d.kind === "CreateFile") return `Создать файл ${d.path}`;
                if (d.kind === "UpdateFile") return `Изменить файл ${d.path}`;
                if (d.kind === "DeleteDir") return `Удалить папку ${d.path}`;
                if (d.kind === "DeleteFile") return `Удалить файл ${d.path}`;
                return `${d.kind}: ${d.path}`;
              };
              return (
                <div style={{ marginTop: "8px", padding: "12px", background: "#f8fafc", borderRadius: "8px", fontSize: "13px", border: "1px solid #e2e8f0" }}>
                  <div style={{ fontWeight: 600, marginBottom: "8px" }}>Вот что изменится:</div>
                  <div style={{ color: "#64748b", marginBottom: "8px" }}>
                    Итого: создать {create}, изменить {update}, удалить {del} · папок: +{mkdir} −{rmdir}
                  </div>
                  <ol style={{ margin: 0, paddingLeft: "20px" }}>
                    {diffs.map((d, j) => (
                      <li key={j} style={{ marginBottom: "4px", fontFamily: "monospace", fontSize: "12px" }}>{label(d)}</li>
                    ))}
                  </ol>
                  <p style={{ marginTop: "10px", marginBottom: 0, fontSize: "12px", color: "#64748b" }}>
                    Если всё верно — нажмите «Применить изменения». Иначе — «Отмена», затем можно отправить новый запрос.
                  </p>
                </div>
              );
            })()}
            {msg.report && (msg.report.actions?.length > 0 || lastPath) && (
              <div style={{ marginTop: "8px", display: "flex", gap: "8px", flexWrap: "wrap" }}>
                {lastPath && lastReport && !pendingPreview && (
                  <>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
                      <button
                        type="button"
                        onClick={handleAgenticRun}
                        disabled={loading || agenticRunning}
                        className="btn"
                        style={{ padding: "8px 16px", background: "#1d4ed8", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
                      >
                        {agenticRunning ? "…" : "🛠 Исправить автоматически"}
                      </button>
                      {agenticRunning && agenticProgress && (
                        <span style={{ fontSize: "12px", color: "#64748b" }}>{agenticProgress.message}</span>
                      )}
                    </span>
                    <button
                      type="button"
                      onClick={handleOneClickFix}
                      disabled={loading}
                      className="btn"
                      style={{ padding: "8px 16px", background: "#059669", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
                    >
                      ✅ Применить безопасные исправления (1 клик)
                    </button>
                    <button
                      type="button"
                      onClick={handleFixAuto}
                      disabled={loading || isGeneratingActions}
                      className="btn"
                      style={{ padding: "8px 16px", background: "#0d9488", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
                    >
                      {isGeneratingActions ? "…" : "Настроить (список исправлений)"}
                    </button>
                  </>
                )}
                {lastPath && lastReportJson && !pendingPreview && (
                  <button
                    type="button"
                    onClick={() => handleProposeFixes()}
                    disabled={loading}
                    className="btn"
                    style={{ padding: "8px 16px", background: "#7c3aed", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
                  >
                    Предложить исправления
                  </button>
                )}
                <button
                  type="button"
                  onClick={onApplyFixes}
                  disabled={loading}
                  style={{ padding: "8px 16px", background: "#16a34a", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
                >
                  Применить рекомендованные исправления
                </button>
                <button type="button" onClick={onShowFixes} disabled={loading} style={{ padding: "8px 14px", background: "#e2e8f0", border: "none", borderRadius: "8px" }}>
                  Показать исправления
                </button>
              </div>
            )}
            {msg.preview && pendingPreview && (
              <div style={{ marginTop: "8px" }}>
                <label style={{ display: "flex", alignItems: "center", gap: "6px", marginBottom: "8px", fontSize: "13px" }}>
                  <input type="checkbox" checked={autoCheck} onChange={(e) => setAutoCheck(e.target.checked)} />
                  Проверять код после изменений
                </label>
                <div style={{ display: "flex", gap: "8px" }}>
                  <button type="button" onClick={onApplyPending} disabled={loading} style={{ padding: "6px 12px", background: "#2563eb", color: "#fff", border: "none", borderRadius: "6px" }}>
                    Применить изменения
                  </button>
                  <button type="button" onClick={onCancelPending} style={{ padding: "6px 12px", background: "#e2e8f0", border: "none", borderRadius: "6px" }}>Отмена</button>
                </div>
              </div>
            )}
          </div>
        ))}
        {messages.length > 0 && (
          <p style={{ fontSize: "12px", color: "#7c3aed", marginTop: "12px", marginBottom: 0 }}>
            Скопируйте ответ кнопкой «Копировать» или введите обратную команду в поле выше для выполнения рекомендаций или новой задачи.
          </p>
        )}
        </div>
      </section>

      {/* Рекомендации ИИ: все пункты с возможностью согласовать/отклонить по одному */}
      {pendingActions && pendingActions.length > 0 && (
        <section style={{ marginTop: "16px", padding: "14px", background: "#f5f3ff", borderRadius: "8px", border: "1px solid #c4b5fd" }}>
          <h3 style={{ fontSize: "14px", marginBottom: "8px", fontWeight: 600, color: "#5b21b6" }}>Рекомендации ИИ ({pendingActions.length})</h3>
          <p style={{ fontSize: "12px", color: "#64748b", marginBottom: "12px" }}>
            Отметьте, какие рекомендации применить. Затем нажмите «Предпросмотр выбранного» или «Применить выбранное».
          </p>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, marginBottom: "12px" }}>
            {pendingActions.map((a, i) => (
              <li key={i} style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "8px", padding: "8px 10px", background: "#fff", borderRadius: "6px", border: "1px solid #ddd6fe" }}>
                <input
                  type="checkbox"
                  checked={pendingActionIdx[i] !== false}
                  onChange={(e) => setPendingActionIdx((prev) => ({ ...prev, [i]: e.target.checked }))}
                />
                <span style={{ flex: 1, fontSize: "13px", fontFamily: "monospace" }}>{String(a.kind)}: {a.path}</span>
              </li>
            ))}
          </ul>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            <button
              type="button"
              disabled={loading || getSelectedPendingActions().length === 0}
              onClick={() => { const sel = getSelectedPendingActions(); if (sel.length && lastPath) handlePreview(lastPath, sel); }}
              style={{ padding: "8px 14px", background: "#7c3aed", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
            >
              Предпросмотр выбранного
            </button>
            <button
              type="button"
              disabled={loading || getSelectedPendingActions().length === 0}
              onClick={async () => {
                const sel = getSelectedPendingActions();
                if (!sel.length || !lastPath) return;
                const ok = window.confirm(`Применить ${sel.length} выбранных изменений к проекту?`);
                if (!ok) return;
                setPendingPreview(null);
                setPendingActions(null);
                setPendingActionIdx({});
                await applyWithProgressDialog(lastPath, sel);
              }}
              style={{ padding: "8px 14px", background: "#16a34a", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
            >
              Применить выбранное
            </button>
            <button
              type="button"
              onClick={() => { const next: Record<number, boolean> = {}; pendingActions?.forEach((_, i) => { next[i] = true; }); setPendingActionIdx(next); }}
              style={{ padding: "8px 14px", border: "1px solid #a78bfa", borderRadius: "8px", background: "#fff", color: "#5b21b6", fontWeight: 500 }}
            >
              Согласовать все
            </button>
            <button
              type="button"
              onClick={() => { const next: Record<number, boolean> = {}; pendingActions?.forEach((_, i) => { next[i] = false; }); setPendingActionIdx(next); }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Отклонить все
            </button>
          </div>
        </section>
      )}

      <AgenticResult
        agenticResult={agenticResult}
        lastReport={lastReport}
        undoAvailable={undoAvailable}
        onUndo={handleUndo}
        onDownloadReport={handleDownloadReport}
        onDownloadDiff={handleDownloadDiff}
      />

      {/* v3.2: блок «Исправления» после «Исправить автоматически» — чекбоксы + предпросмотр/применить */}
      {suggestedActions.length > 0 && (
        <section style={{ marginTop: "16px", padding: "12px", background: "#ecfdf5", borderRadius: "8px", border: "1px solid #a7f3d0" }}>
          <h3 style={{ fontSize: "14px", marginBottom: "8px", fontWeight: 600 }}>Исправления (безопасные)</h3>
          <p style={{ fontSize: "12px", color: "#64748b", marginBottom: "10px" }}>
            Изменения только добавляют файлы и папки. Не изменяют существующие файлы. Не трогают node_modules/.git/dist/build. Есть предпросмотр и откат.
          </p>
          <div style={{ marginBottom: "12px" }}>
            <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "13px", marginBottom: "8px" }}>
              <input type="checkbox" checked={createOnlyMode} onChange={(e) => setCreateOnlyMode(e.target.checked)} />
              Только создание файлов
            </label>
          </div>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, marginBottom: "12px" }}>
            {suggestedActions.map((a, i) => (
              <li key={i} style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "6px", padding: "6px 8px", background: "#fff", borderRadius: "6px", border: "1px solid #d1fae5" }}>
                <input
                  type="checkbox"
                  checked={selectedActionIdx[i] !== false}
                  onChange={(e) => setSelectedActionIdx((prev) => ({ ...prev, [i]: e.target.checked }))}
                />
                <span style={{ flex: 1, fontSize: "13px", fontFamily: "monospace" }}>{String(a.kind)}: {a.path}</span>
                <span style={{ fontSize: "11px", padding: "2px 6px", background: "#a7f3d0", color: "#065f46", borderRadius: "4px", fontWeight: 600 }}>SAFE</span>
              </li>
            ))}
          </ul>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            <button
              type="button"
              disabled={loading || getSelectedSuggestedActions().length === 0}
              onClick={() => { const sel = getSelectedSuggestedActions(); if (sel.length && lastPath) handlePreview(lastPath, sel); }}
              style={{ padding: "8px 14px", background: "#0d9488", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
            >
              Предпросмотр выбранного
            </button>
            <button
              type="button"
              disabled={loading || getSelectedSuggestedActions().length === 0}
              onClick={() => { const sel = getSelectedSuggestedActions(); if (sel.length && lastPath) applyAllSafe(lastPath, sel); }}
              style={{ padding: "8px 14px", background: "#059669", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
            >
              Применить выбранное безопасно
            </button>
            <button
              type="button"
              onClick={() => { const next: Record<number, boolean> = {}; suggestedActions.forEach((_, i) => { next[i] = false; }); setSelectedActionIdx(next); }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Снять выделение
            </button>
            <button
              type="button"
              onClick={() => { const next: Record<number, boolean> = {}; suggestedActions.forEach((_, i) => { next[i] = true; }); setSelectedActionIdx(next); }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Выбрать все
            </button>
          </div>
        </section>
      )}

      {lastReport?.action_groups?.length ? (
        <section style={{ marginTop: "16px", padding: "12px", background: "#f1f5f9", borderRadius: "8px" }}>
          <h3 style={{ fontSize: "14px", marginBottom: "8px", fontWeight: 600 }}>Исправления</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginBottom: "12px" }}>
            {lastReport.action_groups.map((g) => (
              <label
                key={g.id}
                style={{ display: "flex", alignItems: "flex-start", gap: "12px", padding: "10px 12px", borderRadius: "8px", border: "1px solid #e2e8f0", background: "#fff", cursor: "pointer" }}
              >
                <input
                  type="checkbox"
                  style={{ marginTop: "3px" }}
                  checked={!!selectedFixGroupIds[g.id]}
                  onChange={(e) => setSelectedFixGroupIds((prev) => ({ ...prev, [g.id]: e.target.checked }))}
                />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontSize: "13px", fontWeight: 600 }}>{g.title}</div>
                  <div style={{ fontSize: "12px", color: "#64748b" }}>{g.description}</div>
                </div>
              </label>
            ))}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
            <button
              type="button"
              disabled={loading}
              onClick={() => {
                const actions = collectSelectedActions(lastReport, selectedFixGroupIds);
                if (actions.length) handlePreview(lastPath ?? null, actions);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Предпросмотр изменений
            </button>
            <button
              type="button"
              disabled={loading}
              onClick={() => {
                const actions = collectSelectedActions(lastReport, selectedFixGroupIds);
                if (actions.length) handleApplyFixesWithActions(lastPath ?? null, actions);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#16a34a", color: "#fff", fontWeight: 600 }}
            >
              Применить выбранное
            </button>
            {lastPath && (pendingActions?.length ?? collectSelectedActions(lastReport, selectedFixGroupIds).length) > 0 && (
              <button
                type="button"
                disabled={loading}
                onClick={() => {
                  const actions = pendingActions ?? collectSelectedActions(lastReport, selectedFixGroupIds);
                  if (actions.length) applyAllSafe(lastPath!, actions);
                }}
                style={{ padding: "8px 14px", border: "1px solid #0d9488", borderRadius: "8px", background: "#0d9488", color: "#fff", fontWeight: 600 }}
              >
                Применить всё безопасное
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                const groups = lastReport?.action_groups ?? [];
                const all: Record<string, boolean> = {};
                for (const g of groups) all[g.id] = true;
                setSelectedFixGroupIds(all);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Выбрать всё
            </button>
            <button
              type="button"
              onClick={() => {
                const groups = lastReport?.action_groups ?? [];
                const none: Record<string, boolean> = {};
                for (const g of groups) none[g.id] = false;
                setSelectedFixGroupIds(none);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Снять всё
            </button>
          </div>
          {hasPendingPreview && (
            <div style={{ marginTop: "12px", display: "flex", gap: "8px" }}>
              <button type="button" onClick={onApplyPending} disabled={loading} style={{ padding: "8px 14px", background: "#2563eb", color: "#fff", border: "none", borderRadius: "8px" }}>Применить</button>
              <button type="button" onClick={onCancelPending} style={{ padding: "8px 14px", background: "#e2e8f0", border: "none", borderRadius: "8px" }}>Отмена</button>
            </div>
          )}
        </section>
      ) : lastReport && lastReport.actions?.length > 0 ? (
        <section style={{ marginTop: "16px", padding: "12px", background: "#f1f5f9", borderRadius: "8px" }}>
          <h3 style={{ fontSize: "14px", marginBottom: "8px" }}>Исправления</h3>
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {lastReport.actions.map((a, i) => (
              <li key={i} style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "4px" }}>
                <input
                  type="checkbox"
                  checked={selectedActions.some((s) => s.path === a.path && s.kind === a.kind)}
                  onChange={(e) => {
                    if (e.target.checked) setSelectedActions((prev) => [...prev, a]);
                    else setSelectedActions((prev) => prev.filter((s) => !(s.path === a.path && s.kind === a.kind)));
                  }}
                />
                <span style={{ fontSize: "13px" }}>{a.kind}: {a.path}</span>
              </li>
            ))}
          </ul>
          <div style={{ marginTop: "12px", display: "flex", gap: "8px", flexWrap: "wrap" }}>
            <button type="button" onClick={onApplyFixes} disabled={loading} style={{ padding: "8px 16px", background: "#16a34a", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}>Применить рекомендованные исправления</button>
            {!hasPendingPreview && <button type="button" onClick={onShowFixes} disabled={loading} style={{ padding: "8px 14px", background: "#e2e8f0", border: "none", borderRadius: "8px" }}>Предпросмотр изменений</button>}
            {hasPendingPreview && (
              <>
                <button type="button" onClick={onApplyPending} disabled={loading} style={{ padding: "8px 14px", background: "#2563eb", color: "#fff", border: "none", borderRadius: "8px" }}>Применить</button>
                <button type="button" onClick={onCancelPending} style={{ padding: "8px 14px", background: "#e2e8f0", border: "none", borderRadius: "8px" }}>Отмена</button>
              </>
            )}
          </div>
        </section>
      ) : null}

      {lastReport?.fix_packs?.length ? (
        <section style={{ marginTop: "16px", padding: "12px", background: "#eff6ff", borderRadius: "8px", border: "1px solid #bfdbfe" }}>
          <h3 style={{ fontSize: "14px", marginBottom: "8px", fontWeight: 600 }}>Пакеты улучшений</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginBottom: "12px" }}>
            {lastReport.fix_packs.map((p) => (
              <label
                key={p.id}
                style={{ display: "flex", alignItems: "flex-start", gap: "12px", padding: "10px 12px", borderRadius: "8px", border: "1px solid #bfdbfe", background: "#fff", cursor: "pointer" }}
              >
                <input
                  type="checkbox"
                  style={{ marginTop: "3px" }}
                  checked={!!selectedPackIds[p.id]}
                  onChange={(e) => setSelectedPackIds((prev) => ({ ...prev, [p.id]: e.target.checked }))}
                />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontSize: "13px", fontWeight: 600 }}>{p.title}</div>
                  <div style={{ fontSize: "12px", color: "#64748b" }}>{p.description}</div>
                </div>
              </label>
            ))}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
            <button
              type="button"
              disabled={loading}
              onClick={() => {
                const packs = lastReport?.fix_packs ?? [];
                const rec = lastReport?.recommended_pack_ids ?? [];
                const next: Record<string, boolean> = {};
                for (const p of packs) next[p.id] = rec.includes(p.id);
                setSelectedPackIds(next);
                const groupIds = collectGroupIdsFromPacks(lastReport, next);
                const actions = collectActionsByGroupIds(lastReport, groupIds);
                if (actions.length) handlePreview(lastPath ?? null, actions);
              }}
              style={{ padding: "8px 14px", border: "1px solid #3b82f6", borderRadius: "8px", background: "#2563eb", color: "#fff", fontWeight: 600 }}
            >
              Применить рекомендованное (предпросмотр)
            </button>
            <button
              type="button"
              disabled={loading}
              onClick={() => {
                const groupIds = collectGroupIdsFromPacks(lastReport, selectedPackIds);
                const actions = collectActionsByGroupIds(lastReport, groupIds);
                if (actions.length) handlePreview(lastPath ?? null, actions);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#fff" }}
            >
              Предпросмотр выбранных пакетов
            </button>
            <button
              type="button"
              disabled={loading}
              onClick={() => {
                const groupIds = collectGroupIdsFromPacks(lastReport, selectedPackIds);
                const actions = collectActionsByGroupIds(lastReport, groupIds);
                if (actions.length) handleApplyFixesWithActions(lastPath ?? null, actions);
              }}
              style={{ padding: "8px 14px", border: "1px solid #cbd5e1", borderRadius: "8px", background: "#16a34a", color: "#fff", fontWeight: 600 }}
            >
              Применить выбранные пакеты
            </button>
          </div>
        </section>
      ) : null}

      <div style={{ marginTop: "24px", display: "flex", gap: "10px", flexWrap: "wrap" }}>
        <button
          type="button"
          onClick={onNewRequest}
          style={{ padding: "10px 18px", background: "var(--color-primary)", color: "#fff", border: "none", borderRadius: "var(--radius-md)", fontWeight: 600, boxShadow: "0 2px 6px rgba(37, 99, 235, 0.3)" }}
        >
          Новый запрос
        </button>
        <button
          type="button"
          onClick={handleUndo}
          disabled={!undoAvailable || loading}
          style={{ padding: "10px 18px", background: undoAvailable ? "#475569" : "var(--color-bg)", color: undoAvailable ? "#fff" : "var(--color-text-soft)", border: undoAvailable ? "none" : "1px solid var(--color-border-strong)", borderRadius: "var(--radius-md)", fontWeight: 500 }}
        >
          Откатить
        </button>
        <button
          type="button"
          onClick={handleRedo}
          disabled={!redoAvailable || loading}
          style={{ padding: "10px 18px", background: redoAvailable ? "var(--color-secondary)" : "var(--color-bg)", color: redoAvailable ? "#fff" : "var(--color-text-soft)", border: redoAvailable ? "none" : "1px solid var(--color-border-strong)", borderRadius: "var(--radius-md)", fontWeight: 500 }}
        >
          Повторить
        </button>
      </div>

      {applyProgressVisible ? (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.4)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 9999,
          }}
          onClick={(e) => e.target === e.currentTarget && applyResult && setApplyProgressVisible(false)}
        >
          <div
            style={{
              background: "#fff",
              borderRadius: "var(--radius-xl)",
              boxShadow: "0 20px 60px rgba(0,0,0,0.2)",
              maxWidth: 480,
              width: "90%",
              maxHeight: "80vh",
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ padding: "16px 20px", borderBottom: "1px solid var(--color-border)", fontWeight: 700, fontSize: "16px", color: "#1e3a5f" }}>
              Процесс изменений
            </div>
            <div
              style={{
                padding: "16px 20px",
                overflowY: "auto",
                flex: 1,
                fontFamily: "monospace",
                fontSize: "13px",
                lineHeight: 1.6,
                color: "#334155",
              }}
            >
              {applyProgressLog.map((line, i) => (
                <div key={i} style={{ marginBottom: "4px" }}>
                  {line}
                </div>
              ))}
              {applyResult && applyResult.checks?.length > 0 ? (
                <div style={{ marginTop: "12px", paddingTop: "12px", borderTop: "1px solid var(--color-border)" }}>
                  {applyResult.checks.map((c, i) => (
                    <div key={i} style={{ color: c.ok ? "#16a34a" : "#dc2626" }}>
                      {c.ok ? "✓" : "✗"} {c.stage}: {c.output?.slice(0, 80)}{c.output?.length > 80 ? "…" : ""}
                    </div>
                  ))}
                </div>
              ) : null}
            </div>
            <div style={{ padding: "12px 20px", borderTop: "1px solid var(--color-border)", display: "flex", justifyContent: "flex-end" }}>
              <button
                type="button"
                onClick={() => setApplyProgressVisible(false)}
                style={{ padding: "8px 16px", background: "var(--color-primary)", color: "#fff", border: "none", borderRadius: "8px", fontWeight: 600 }}
              >
                Закрыть
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {trendsModalOpen && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.4)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 9998,
          }}
          onClick={(e) => e.target === e.currentTarget && (setTrendsModalOpen(false), setSelectedRecommendation(null))}
        >
          <div
            style={{
              background: "#fff",
              borderRadius: "var(--radius-xl)",
              boxShadow: "0 20px 60px rgba(0,0,0,0.2)",
              maxWidth: 520,
              width: "90%",
              maxHeight: "85vh",
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ padding: "16px 20px", borderBottom: "1px solid var(--color-border)", fontWeight: 700, fontSize: "16px", color: "#1e40af", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              Тренды и рекомендации
              <button type="button" onClick={() => (setTrendsModalOpen(false), setSelectedRecommendation(null))} style={{ padding: "6px 12px", background: "#e2e8f0", border: "none", borderRadius: "8px", fontWeight: 600 }}>Закрыть</button>
            </div>
            <div style={{ padding: "16px 20px", overflowY: "auto", flex: 1 }}>
              {selectedRecommendation ? (
                <div style={{ marginBottom: "16px" }}>
                  <button type="button" onClick={() => setSelectedRecommendation(null)} style={{ marginBottom: "12px", padding: "6px 12px", background: "#f1f5f9", border: "none", borderRadius: "8px", cursor: "pointer" }}>← Назад к списку</button>
                  <h3 style={{ fontSize: "16px", fontWeight: 700, color: "#1e40af", marginBottom: "8px" }}>{selectedRecommendation.title}</h3>
                  {selectedRecommendation.summary && <p style={{ fontSize: "14px", color: "#334155", lineHeight: 1.6, marginBottom: "8px" }}>{selectedRecommendation.summary}</p>}
                  {selectedRecommendation.source && <p style={{ fontSize: "12px", color: "var(--color-text-muted)" }}>Источник: {selectedRecommendation.source}</p>}
                  {selectedRecommendation.url && (
                    <a href={selectedRecommendation.url} target="_blank" rel="noopener noreferrer" style={{ fontSize: "14px", color: "#2563eb", marginTop: "8px", display: "inline-block" }}>Подробнее</a>
                  )}
                </div>
              ) : (
                <>
                  {trends && (
                    <>
                      <p style={{ fontSize: "12px", color: "var(--color-text-muted)", marginBottom: "12px" }}>
                        Обновлено: {trends.last_updated ? new Date(trends.last_updated).toLocaleDateString() : "—"}
                        {trends.should_update && <span style={{ marginLeft: "8px", color: "#b91c1c", fontWeight: 600 }}>· Рекомендуется обновить</span>}
                      </p>
                      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
                        {trends.recommendations.map((r, i) => (
                          <li key={i}>
                            <button
                              type="button"
                              onClick={() => setSelectedRecommendation(r)}
                              style={{
                                display: "block",
                                width: "100%",
                                padding: "12px 14px",
                                marginBottom: "8px",
                                background: "#eff6ff",
                                border: "1px solid #93c5fd",
                                borderRadius: "var(--radius-md)",
                                textAlign: "left",
                                cursor: "pointer",
                                fontSize: "14px",
                                fontWeight: 600,
                                color: "#1e40af",
                              }}
                            >
                              {r.title}
                            </button>
                          </li>
                        ))}
                      </ul>
                    </>
                  )}
                  {(!trends || trends.recommendations.length === 0) && !trendsLoading && <p style={{ color: "var(--color-text-muted)" }}>Нет данных. Нажмите «Обновить тренды».</p>}
                  {trendsLoading && <p style={{ color: "var(--color-text-muted)" }}>Загрузка…</p>}
                  <button type="button" onClick={handleFetchTrends} disabled={trendsLoading} style={{ marginTop: "12px", padding: "10px 18px", background: "var(--color-primary)", color: "#fff", border: "none", borderRadius: "var(--radius-md)", fontWeight: 600 }}>
                    {trendsLoading ? "Обновляю…" : "Обновить тренды"}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      </main>
    </div>
  );
}
