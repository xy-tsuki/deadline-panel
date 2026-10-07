import { NotificationSettings } from "./NotificationSettings";
import { SettingsChrome, SettingsContent } from "./SettingsChrome";
import { LiquidTextSegments } from "./LiquidTextSegments";
import { installMotionTokens } from "./motion";
import { useMinuteClock } from "./useMinuteClock";
import { hasPendingPanelInteraction } from "./panelInteraction";
import { JsonFileControls } from "./JsonFileControls";
import { ToolDisclosure, useToolDisclosure } from "./ToolDisclosure";
import { TopNSegments } from "./TopNSegments";
import { AppearanceSettings } from "./AppearanceSettings";
import { listenForSyncRequests } from "./sync/owner";
import { listen } from "@tauri-apps/api/event";
import { type FormEvent, type MouseEvent, type PointerEvent, useEffect, useMemo, useRef, useState } from "react";
import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import {
  Check,
  CalendarClock,
  ChevronDown,
  ChevronUp,
  Cloud,
  Clock3,
  Copy,
  Database,
  EyeOff,
  FolderOpen,
  History,
  KeyRound,
  Plus,
  RefreshCw,
  RotateCcw,
  Settings,
  Star,
  Trash2,
} from "lucide-react";
import { parseCommand } from "./domain/commandParser";
import { parseQuickAdd } from "./domain/quickAddParser";
import { formatDue, formatTimeLeft, fromDateTimeLocalValue, toDateTimeLocalValue } from "./domain/date";
import { getCurrentTasks, getNearestDeadline, sortDeadlineTasks } from "./domain/taskSorting";
import { DeadlineTask, NewTaskInput, TaskPriority, priorityLabel } from "./domain/task";
import { FocusLimit, selectFocusTasks, useDeadlineStore } from "./store/deadlineStore";
import { getImportPrompt, getStrings, languageName, languageOptions, resolveLanguage, setNativeSystemLanguage } from "./i18n";
import {
  getAutostartEnabled,
  getSystemLanguage,
  setWindowMaterial,
  backupDatabase,
  getPanelPointerState,
  getPanelExpandDirection,
  hidePanelTemporarily,
  hidePanelForMinutes,
  getPanelVisibility,
  type PanelVisibility,
  isTauriRuntime,
  openDataDir,
  openSettingsWindow,
  resetPanelPosition,
  setAutostartEnabled,
  restartApp,
  setPanelExpanded as setNativePanelExpanded,
  showPanelContextMenu,
  showPanel,
  startPanelDrag
} from "./runtime/tauri";
import {
  generateSyncCode,
  hasDefaultSupabaseConfig,
  loadSupabaseSyncSettings,
  saveSupabaseSyncSettings
} from "./sync/supabaseSync";
import { APP_VERSION } from "./version";

const priorityOptions: TaskPriority[] = ["urgent", "high", "medium", "low"];
const COLLAPSE_DELAY_MS = 220;
const COLLAPSE_ANIMATION_MS = 210;
const COLLAPSED_HOVER_DWELL_MS = 180;
const RELEASES_API_URL = "https://api.github.com/repos/xy-tsuki/deadline-panel/releases/latest";
const CLOUD_SYNC_START_DELAY_MS = 10_000;
const CLOUD_SYNC_INTERVAL_MS = 5 * 60 * 1000;
const CLOUD_SYNC_EXPAND_MIN_INTERVAL_MS = 60_000;
const UPDATE_DOWNLOAD_TIMEOUT_MS = 180_000;

const updaterStatusText = {
  zh: {
    available: "发现新版本 {version}，正在准备下载",
    confirmInstall: "发现新版本 {version}。是否现在下载并安装？",
    cancelled: "已取消更新",
    downloading: "正在下载更新...",
    downloadingProgress: "正在下载更新 {progress}%",
    installing: "正在安装更新...",
    restarting: "更新已安装，正在重启..."
  },
  ja: {
    available: "新しいバージョン {version} を見つけました。ダウンロードを準備しています",
    confirmInstall: "新しいバージョン {version} があります。今すぐダウンロードしてインストールしますか？",
    cancelled: "更新をキャンセルしました",
    downloading: "更新をダウンロードしています...",
    downloadingProgress: "更新をダウンロードしています {progress}%",
    installing: "更新をインストールしています...",
    restarting: "更新をインストールしました。再起動しています..."
  },
  en: {
    available: "Version {version} is available. Preparing download",
    confirmInstall: "Version {version} is available. Download and install it now?",
    cancelled: "Update cancelled",
    downloading: "Downloading update...",
    downloadingProgress: "Downloading update {progress}%",
    installing: "Installing update...",
    restarting: "Update installed. Restarting..."
  }
} as const;

interface ImportPreviewRow {
  id: string;
  lineNumber: number;
  ok: boolean;
  error?: string;
  title: string;
  dueAt: string;
  priority: TaskPriority;
  notes: string;
}

interface LatestReleaseResponse {
  tag_name?: string;
  html_url?: string;
}

export function App() {
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandDirection, setExpandDirection] = useState<"up" | "down">("up");
  const collapseTimerRef = useRef<number | null>(null);
  const animationTimerRef = useRef<number | null>(null);
  const collapsedHoverTimerRef = useRef<number | null>(null);
  const lastAutoHiddenRef = useRef(false);
  const isDraggingPanelRef = useRef(false);
  const cloudSyncInFlightRef = useRef(false);
  const lastCloudSyncAtRef = useRef(0);
  const requestedRole = new URLSearchParams(window.location.search).get("window");
  const windowRole = requestedRole === "settings" ? "settings" : requestedRole === "panel" ? "panel" : "strip";
  const isPanelWindow = windowRole !== "strip";
  const appearance = useDeadlineStore((state) => state.appearance);
  const materialRequest = useRef(0);
  const tasks = useDeadlineStore((state) => state.tasks);
  const focusLimit = useDeadlineStore((state) => state.focusLimit);
  const isLoading = useDeadlineStore((state) => state.isLoading);
  const taskError = useDeadlineStore((state) => state.error);
  const notificationError = useDeadlineStore((state) => state.notificationError);
  const error = [taskError, notificationError].filter(Boolean).join("\n") || null;
  const commandMessage = useDeadlineStore((state) => state.commandMessage);
  const clearCommandMessage = useDeadlineStore((state) => state.clearCommandMessage);
  const load = useDeadlineStore((state) => state.load);
  const syncWithCloud = useDeadlineStore((state) => state.syncWithCloud);

  useEffect(() => {
    installMotionTokens();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const request = ++materialRequest.current;
      document.documentElement.dataset.theme = appearance.theme === "system" ? (media.matches ? "dark" : "light") : appearance.theme;
      document.documentElement.dataset.lowPower = String(appearance.lowPowerMode);
      if (isTauriRuntime()) {
        const theme = document.documentElement.dataset.theme;
        void setWindowMaterial(theme === "dark", appearance.lowPowerMode).then(mode => {
          if (request === materialRequest.current && document.documentElement.dataset.theme === theme) document.documentElement.dataset.material = mode;
        }).catch(error => { if (request === materialRequest.current && document.documentElement.dataset.theme === theme) document.documentElement.dataset.material = "compatibility"; console.error("Desktop material request failed", error); });
      }
    };
    apply(); media.addEventListener("change", apply);
    return () => { ++materialRequest.current; media.removeEventListener("change", apply); };
  }, [appearance.theme, appearance.lowPowerMode]);


  useEffect(() => {
    const pointer = () => { document.documentElement.dataset.input = "pointer"; };
    const keyboard = () => { document.documentElement.dataset.input = "keyboard"; };
    document.addEventListener("pointerdown", pointer, true);
    document.addEventListener("keydown", keyboard, true);
    return () => { document.removeEventListener("pointerdown", pointer, true); document.removeEventListener("keydown", keyboard, true); };
  }, []);

  const focusTasks = useMemo(() => selectFocusTasks(tasks, focusLimit), [focusLimit, tasks]);
  const nearest = useMemo(() => getNearestDeadline(tasks), [tasks]);
  const currentTasks = useMemo(() => getCurrentTasks(tasks), [tasks]);

  useEffect(() => {
    if (!isTauriRuntime() || !isPanelWindow) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen<"up" | "down">("deadline-panel-direction", ({ payload }) => {
      if (!disposed) setExpandDirection(payload);
    }).then(async unlisten => {
      if (disposed) { unlisten(); return; }
      stop = unlisten;
      const result = await getPanelExpandDirection();
      if (!disposed) setExpandDirection(result.direction);
    });
    return () => { disposed = true; stop?.(); };
  }, [isPanelWindow]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenNotifications: (() => void) | undefined;
    let unlistenMaterial: (() => void) | undefined;
    async function initialize() {
      if (isTauriRuntime()) {
        const stop = await listen<number>("deadline-state-changed", () => void load({ silent: true }));
        if (disposed) { stop(); return; }
        unlisten = stop;
        const stopNotifications = await listen<string>("deadline-notification-error", ({ payload }) => useDeadlineStore.setState({ notificationError: payload || null }));
        if (disposed) { stopNotifications(); return; }
        unlistenNotifications = stopNotifications;
        const stopMaterial = await listen<string>("deadline-material-failed", ({ payload }) => {
          ++materialRequest.current;
          document.documentElement.dataset.material = "compatibility";
          console.error("Glass renderer recovered to theme color", payload);
        });
        if (disposed) { stopMaterial(); return; }
        unlistenMaterial = stopMaterial;
        setNativeSystemLanguage(await getSystemLanguage().catch(() => ""));
      }
      if (!disposed) await load();
    }
    void initialize();
    return () => { disposed = true; unlisten?.(); unlistenNotifications?.(); unlistenMaterial?.(); };
  }, [load]);

  useEffect(() => {
    if (!isTauriRuntime() || isPanelWindow) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    void listenForSyncRequests((silent) => syncWithCloud({ silent })).then((unlisten) => {
      if (disposed) unlisten(); else stop = unlisten;
    });
    return () => { disposed = true; stop?.(); };
  }, [isPanelWindow, syncWithCloud]);

  async function runSilentCloudSync(minIntervalMs = 0) {
    const now = Date.now();
    if (isLoading || cloudSyncInFlightRef.current) return;
    if (minIntervalMs > 0 && now - lastCloudSyncAtRef.current < minIntervalMs) return;

    cloudSyncInFlightRef.current = true;
    lastCloudSyncAtRef.current = now;
    try {
      const settings = await loadSupabaseSyncSettings();
      if (settings.url && settings.anonKey && settings.syncCode) await syncWithCloud({ silent: true });
    } catch {
      // The store keeps the failure; the next scheduled attempt may retry.
    } finally {
      cloudSyncInFlightRef.current = false;
    }
  }

  useEffect(() => {
    if (isLoading || isPanelWindow) return;

    let isMounted = true;

    async function runSilentSync() {
      if (!isMounted) return;
      await runSilentCloudSync();
    }

    const startupTimer = window.setTimeout(() => void runSilentSync(), CLOUD_SYNC_START_DELAY_MS);
    const interval = window.setInterval(() => void runSilentSync(), CLOUD_SYNC_INTERVAL_MS);

    return () => {
      isMounted = false;
      window.clearTimeout(startupTimer);
      window.clearInterval(interval);
    };
  }, [isLoading, isPanelWindow, syncWithCloud]);

  useEffect(() => {
    if (!commandMessage) return;

    const timer = window.setTimeout(() => {
      clearCommandMessage();
    }, 3200);
    return () => window.clearTimeout(timer);
  }, [clearCommandMessage, commandMessage]);

  useEffect(() => {
    return () => {
      if (collapseTimerRef.current !== null) {
        window.clearTimeout(collapseTimerRef.current);
      }
      if (animationTimerRef.current !== null) {
        window.clearTimeout(animationTimerRef.current);
      }
      if (collapsedHoverTimerRef.current !== null) {
        window.clearTimeout(collapsedHoverTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime() || windowRole === "settings") return;
    let disposed = false;
    let stop: (() => void) | undefined;
    const apply = (state: PanelVisibility) => {
      if (disposed) return;
      lastAutoHiddenRef.current = !state.visible;
      isDraggingPanelRef.current = state.dragging;
      setIsExpanded(state.expanded);
      setExpandDirection(state.direction);
      if (!state.expanded) {
        clearCollapseTimer();
        clearAnimationTimer();
        clearCollapsedHoverTimer();
      }
    };
    // Fullscreen visibility belongs to the native process. Its timer keeps
    // running even when Windows suspends timers in a hidden WebView.
    void listen<PanelVisibility>("deadline-panel-visibility", ({ payload }) => apply(payload))
      .then(async unlisten => {
        if (disposed) { unlisten(); return; }
        stop = unlisten;
        apply(await getPanelVisibility());
      }).catch(error => console.error("Panel visibility sync failed", error));
    return () => {
      disposed = true;
      stop?.();
    };
  }, [windowRole]);


  function clearAnimationTimer() {
    if (animationTimerRef.current !== null) {
      window.clearTimeout(animationTimerRef.current);
      animationTimerRef.current = null;
    }
  }

  function clearCollapsedHoverTimer() {
    if (collapsedHoverTimerRef.current !== null) {
      window.clearTimeout(collapsedHoverTimerRef.current);
      collapsedHoverTimerRef.current = null;
    }
  }

  function clearCollapseTimer() {
    if (collapseTimerRef.current !== null) {
      window.clearTimeout(collapseTimerRef.current);
      collapseTimerRef.current = null;
    }
  }

  function scheduleCollapseWhenPointerLeavesWindow(delayMs = COLLAPSE_DELAY_MS) {
    clearCollapseTimer();
    collapseTimerRef.current = window.setTimeout(() => {
      collapseTimerRef.current = null;
      void collapseIfPointerOutsideWindow();
    }, delayMs);
  }

  async function collapseIfPointerOutsideWindow() {
    if (hasPendingPanelInteraction()) { scheduleCollapseWhenPointerLeavesWindow(); return; }
    if (document.activeElement instanceof HTMLElement && document.activeElement.matches("input, textarea, select, [contenteditable=true]")) return;
    if (isTauriRuntime()) {
      try {
        const state = await getPanelPointerState();
        if (state?.inWindow) {
          scheduleCollapseWhenPointerLeavesWindow(180);
          return;
        }
      } catch {
        // Fall through to normal collapse if native pointer state is unavailable.
      }
    }

    collapsePanel();
  }

  async function expandPanel() {
    clearCollapseTimer();
    clearAnimationTimer();
    clearCollapsedHoverTimer();
    void runSilentCloudSync(CLOUD_SYNC_EXPAND_MIN_INTERVAL_MS);

    const result = await setNativePanelExpanded(true);
    setExpandDirection(result.direction);
    setIsExpanded(result.expanded ?? true);
  }

  function collapsePanel() {
    clearAnimationTimer();
    setIsExpanded(false);
    void (async () => {
      const result = await setNativePanelExpanded(false);
      setExpandDirection(result.direction);
      animationTimerRef.current = null;
    })();
  }

  async function handleStripPointerEnter() {
    clearCollapseTimer();
    if (isExpanded || lastAutoHiddenRef.current || isDraggingPanelRef.current) return;

    if (!isTauriRuntime()) {
      void expandPanel();
      return;
    }

    clearCollapsedHoverTimer();
    try {
      const state = await getPanelPointerState();
      if (!state?.inTrigger) return;
    } catch {
      return;
    }

    collapsedHoverTimerRef.current = window.setTimeout(() => {
      collapsedHoverTimerRef.current = null;
      if (!isExpanded && !lastAutoHiddenRef.current && !isDraggingPanelRef.current) {
        void (async () => {
          try {
            const state = await getPanelPointerState();
            if (!state?.inTrigger) return;
          } catch {
            return;
          }
          void expandPanel();
        })();
      }
    }, COLLAPSED_HOVER_DWELL_MS);
  }

  function handleStripPointerLeave() {
    clearCollapsedHoverTimer();
    if (!isExpanded) return;

    scheduleCollapseWhenPointerLeavesWindow();
  }

  function handlePanelPointerEnter() {
    clearCollapseTimer();
  }

  function handlePanelPointerLeave() {
    scheduleCollapseWhenPointerLeavesWindow();
  }

  async function handleHideTemporarily() {
    clearCollapseTimer();
    clearAnimationTimer();
    clearCollapsedHoverTimer();
    setIsExpanded(false);
    const result = await setNativePanelExpanded(false);
    setExpandDirection(result.direction);
    await hidePanelTemporarily();
  }

  function handlePanelContextMenu(event: MouseEvent<HTMLElement>) {
    if (isTextEditingTarget(event.target)) return;

    event.preventDefault();
    clearCollapsedHoverTimer();
    void showPanelContextMenu();
  }

  async function handleCollapsedDragStart(event: PointerEvent<HTMLElement>) {
    if (event.button !== 0 || isPanelWindow || isInteractiveTarget(event.target)) return;

    event.preventDefault();
    clearCollapseTimer();
    clearAnimationTimer();
    clearCollapsedHoverTimer();
    isDraggingPanelRef.current = true;
    const markDragDone = () => {
      isDraggingPanelRef.current = false;
      window.removeEventListener("pointerup", markDragDone);
      window.removeEventListener("mouseup", markDragDone);
    };
    window.addEventListener("pointerup", markDragDone, { once: true });
    window.addEventListener("mouseup", markDragDone, { once: true });

    if (isExpanded) {
      setIsExpanded(false);
      const result = await setNativePanelExpanded(false);
      setExpandDirection(result.direction);
    }

    if (isDraggingPanelRef.current) {
      await startPanelDrag();
    }
  }

  if (windowRole === "settings") {
    return <main className="settings-window">
      <SettingsContent>
      <h1>Deadline Panel</h1>
      {error ? <p className="error-message" role="alert">{error}</p> : null}
      <SettingsPanel standalone />
      </SettingsContent>
      <SettingsChrome />
    </main>;
  }

  if (isPanelWindow) {
    return (
      <main className="desktop-stage desktop-stage--panel">
        <section
          className={["panel-window", expandDirection === "down" ? "panel-window--open-down" : ""].filter(Boolean).join(" ")}
          onPointerEnter={handlePanelPointerEnter}
          onPointerLeave={handlePanelPointerLeave}
          onFocusCapture={clearCollapseTimer}
          onBlurCapture={() => scheduleCollapseWhenPointerLeavesWindow()}
          onContextMenu={handlePanelContextMenu}
          aria-label="Deadline Panel"
        >
          <ExpandedPanel
            currentTasks={currentTasks}
            focusTasks={focusTasks}
            focusLimit={focusLimit}
            error={error}
            message={commandMessage}
            onHideTemporarily={handleHideTemporarily}
          />
        </section>
      </main>
    );
  }

  return (
    <main className="desktop-stage desktop-stage--strip">
      <section
        className="strip-window"
        onPointerEnter={handleStripPointerEnter}
        onPointerLeave={handleStripPointerLeave}
        onContextMenu={handlePanelContextMenu}
        aria-label="Deadline Panel"
      >
        <CollapsedStrip
          currentTasks={currentTasks}
          total={focusLimit}
          nearest={nearest}
          isLoading={isLoading}
          onStartDrag={(event) => void handleCollapsedDragStart(event)}
        />
      </section>
    </main>
  );
}

function CollapsedStrip({
  currentTasks,
  total,
  nearest,
  isLoading,
  onStartDrag
}: {
  currentTasks: DeadlineTask[];
  total: number;
  nearest?: DeadlineTask;
  isLoading: boolean;
  onStartDrag: (event: PointerEvent<HTMLElement>) => void;
}) {
  const now = useMinuteClock();
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const summary = isLoading
    ? ui.panel.restoring
    : currentTasks.length > 0
      ? `${ui.panel.currentPrefix} | ${currentTasks.map((task) => task.title).join(" + ")}（${formatTimeLeft(currentTasks[0].dueAt, new Date(now), language)}）`
      : nearest
        ? `Top ${total} | ${nearest.title}（${formatTimeLeft(nearest.dueAt, new Date(now), language)}）`
        : ui.panel.emptyCollapsed;

  return (
    <div className="collapsed-strip" onPointerDown={onStartDrag}>
      <Clock3 aria-hidden="true" />
      <span>{summary}</span>
      <ChevronUp aria-hidden="true" />
    </div>
  );
}

function ExpandedPanel({
  currentTasks,
  focusTasks,
  focusLimit,
  error,
  message,
  onHideTemporarily
}: {
  currentTasks: DeadlineTask[];
  focusTasks: DeadlineTask[];
  focusLimit: FocusLimit;
  error: string | null;
  message: string | null;
  onHideTemporarily: () => void;
}) {
  const now = useMinuteClock();
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const firstReminder = currentTasks[0] ?? focusTasks[0];

  return (
    <div className="expanded-panel">
      {error ? <div className="notice notice--error">{error}</div> : null}
      {message ? <div className="notice">{message}</div> : null}

      <section className="panel-section panel-section--focus">
        <div className="focus-topline">
          <p className="section-label">{currentTasks.length > 0 ? ui.panel.inProgress : ui.panel.nearestDeadline}</p>
          <button type="button" className="icon-button icon-button--quiet" title={ui.panel.hideTemporarily} onClick={onHideTemporarily}>
            <EyeOff aria-hidden="true" />
          </button>
        </div>
        {firstReminder ? (
          <>
            <h1>{firstReminder.title}</h1>
            <div className="focus-meta">
              <span>{currentTasks.length > 0 ? ui.panel.currentTask : ui.panel.nearestDeadline}</span>
              <span>{formatTimeLeft(firstReminder.dueAt, new Date(now), language)}</span>
              <span className={`priority-pill priority-pill--${firstReminder.priority}`}>{priorityLabel[firstReminder.priority]}</span>
            </div>
            {currentTasks.length > 1 ? (
              <div className="current-stack">
                {currentTasks.slice(1).map((task) => (
                  <span key={task.id}>{task.title}</span>
                ))}
              </div>
            ) : null}
          </>
        ) : (
          <p className="empty-text">{ui.panel.noUrgentDeadline}</p>
        )}
      </section>

      <section className="panel-section">
        <div className="section-heading">
          <p className="section-label">{ui.panel.recentDeadline}</p>
          <FocusLimitControl selected={focusLimit} />
        </div>
        <TaskList tasks={focusTasks} />
      </section>

      <ToolDisclosure><TaskComposer />
      <HistoryPanel />
      <CloudSyncPanel />
      <SettingsPanel />
      <ImportPanel /></ToolDisclosure>
    </div>
  );
}

function FocusLimitControl({ selected }: { selected: FocusLimit }) {
  const setFocusLimit = useDeadlineStore((state) => state.setFocusLimit);
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);

  return <TopNSegments value={selected} label={ui.focus.countLabel} onChange={setFocusLimit} />;
}

function TaskList({ tasks }: { tasks: DeadlineTask[] }) {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);

  if (tasks.length === 0) {
    return <p className="empty-text">{ui.panel.emptyTaskList}</p>;
  }

  return (
    <ol className="task-list">
      {tasks.map((task, index) => (
        <TaskRow key={task.id} task={task} index={index + 1} />
      ))}
    </ol>
  );
}

function TaskRow({ task, index }: { task: DeadlineTask; index?: number }) {
  const now = useMinuteClock();
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const updateTask = useDeadlineStore((state) => state.updateTask);
  const completeTask = useDeadlineStore((state) => state.completeTask);
  const restoreTask = useDeadlineStore((state) => state.restoreTask);
  const completed = task.status === "completed";
  const postponeTask = useDeadlineStore((state) => state.postponeTask);
  const toggleCurrentTask = useDeadlineStore((state) => state.toggleCurrentTask);
  const deleteTask = useDeadlineStore((state) => state.deleteTask);
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [isEditingDue, setIsEditingDue] = useState(false);
  const [draftTitle, setDraftTitle] = useState(task.title);
  const [isEditingNotes, setIsEditingNotes] = useState(false);
  const [draftNotes, setDraftNotes] = useState(task.notes);
  const editBusyRef = useRef(false);
  const editCancelledRef = useRef(false);
  const [draftDue, setDraftDue] = useState(() => toDateTimeLocalValue(task.dueAt));
  const [postponeMode, setPostponeMode] = useState<"closed" | "menu" | "custom">("closed");
  const [priorityMenuOpen, setPriorityMenuOpen] = useState(false);
  const [customDue, setCustomDue] = useState(() => toDateTimeLocalValue(addDays(task.dueAt, 1)));
  const compactNotes = task.notes.trim();

  useEffect(() => {
    if (!isEditingTitle) setDraftTitle(task.title);
  }, [task.title, isEditingTitle]);

  useEffect(() => {
    if (!isEditingDue) setDraftDue(toDateTimeLocalValue(task.dueAt));
    setCustomDue(toDateTimeLocalValue(addDays(task.dueAt, 1)));
  }, [task.dueAt, isEditingDue]);

  useEffect(() => { if (!isEditingNotes) setDraftNotes(task.notes); }, [task.notes, isEditingNotes]);

  async function commitNotes() {
    if (editBusyRef.current || editCancelledRef.current) return;
    editBusyRef.current = true;
    try {
      const notes = draftNotes.trim();
      if (notes !== task.notes && !await updateTask(task.id, { notes })) return;
      setIsEditingNotes(false);
    } finally { editBusyRef.current = false; }
  }

  async function commitTitle() {
    if (editBusyRef.current || editCancelledRef.current) return;
    editBusyRef.current = true;
    try {
    const title = draftTitle.trim();
    if (!title) {
      setDraftTitle(task.title);
      return;
    }
    if (title !== task.title) {
      if (!await updateTask(task.id, { title })) return;
    }
    setIsEditingTitle(false);
    } catch (error) { useDeadlineStore.setState({ error: String(error) }); }
    finally { editBusyRef.current = false; }
  }

  async function commitDue() {
    if (editBusyRef.current || editCancelledRef.current) return;
    editBusyRef.current = true;
    try {
    if (!draftDue) {
      setDraftDue(toDateTimeLocalValue(task.dueAt));
      return;
    }
    const dueAt = fromDateTimeLocalValue(draftDue);
    if (dueAt !== task.dueAt) {
      if (!await updateTask(task.id, { dueAt })) return;
    }
    setIsEditingDue(false);
    } catch (error) { useDeadlineStore.setState({ error: String(error) }); }
    finally { editBusyRef.current = false; }
  }

  async function handlePostpone(dueAt: string) {
    if (!await postponeTask(task.id, dueAt)) return;
    setPostponeMode("closed");
  }

  async function handlePriorityChange(priority: TaskPriority) {
    if (priority !== task.priority) {
      if (!await updateTask(task.id, { priority })) return;
    }
    setPriorityMenuOpen(false);
  }

  return (
    <li className={`task-row task-row--${task.priority} task-row--due-${getDueTone(task.dueAt)} ${task.isCurrent ? "task-row--current" : ""}`}>
      <span className="task-index">{index ?? <Check aria-hidden="true" size={13}/>}</span>
      <div className="task-copy">
        {isEditingTitle ? (
          <input
            className="inline-edit inline-edit--title"
            autoFocus
            value={draftTitle}
            onChange={(event) => setDraftTitle(event.target.value)}
            onBlur={() => void commitTitle()}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) void commitTitle();
              if (event.key === "Escape") {
                editCancelledRef.current = true;
                setDraftTitle(task.title);
                setIsEditingTitle(false);
              }
            }}
          />
        ) : (
          <button type="button" className="task-title task-title-button" onClick={() => { editCancelledRef.current = false; setIsEditingTitle(true); }}>
            {task.title}
          </button>
        )}

        {isEditingDue ? (
          <input
            className="inline-edit inline-edit--due"
            type="datetime-local"
            autoFocus
            value={draftDue}
            onChange={(event) => setDraftDue(event.target.value)}
            onBlur={() => void commitDue()}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) void commitDue();
              if (event.key === "Escape") {
                editCancelledRef.current = true;
                setDraftDue(toDateTimeLocalValue(task.dueAt));
                setIsEditingDue(false);
              }
            }}
          />
        ) : (
          <button type="button" className="task-due task-due-button" onClick={() => { editCancelledRef.current = false; setIsEditingDue(true); }}>
            {ui.task.due}
            {formatDue(task.dueAt, language)}
          </button>
        )}
        {isEditingNotes ? <input className="inline-edit task-note" autoFocus aria-label={ui.import.notes} value={draftNotes}
          onChange={(event) => setDraftNotes(event.target.value)} onBlur={() => void commitNotes()}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing) void commitNotes();
            if (event.key === "Escape") { editCancelledRef.current = true; setDraftNotes(task.notes); setIsEditingNotes(false); }
          }} /> : <button type="button" className="task-note task-note-button" title={compactNotes || ui.import.notes}
          onClick={() => { editCancelledRef.current = false; setIsEditingNotes(true); }}>{compactNotes || ui.import.notes}</button>}
      </div>
      <div className="task-side">
        <span>{formatTimeLeft(task.dueAt, new Date(now), language)}</span>
        <div className="priority-control">
          <button
            type="button"
            className={`priority-pill priority-pill--${task.priority}`}
            disabled={completed}
            onClick={() => setPriorityMenuOpen((value) => !value)}
          >
            {priorityLabel[task.priority]}
          </button>
          {priorityMenuOpen ? (
            <div className="priority-menu">
              {priorityOptions.map((option) => (
                <button
                  key={option}
                  type="button"
                  className={option === task.priority ? "priority-menu__item priority-menu__item--active" : "priority-menu__item"}
                  onClick={() => void handlePriorityChange(option)}
                >
                  {option}
                </button>
              ))}
            </div>
          ) : null}
        </div>
        <span className="task-status">{ui.status[task.status]}</span>
      </div>
      <div className="task-actions" aria-label={`${task.title} ${ui.common.actions}`}>
        {!completed ? <button
          type="button"
          className={`icon-button ${task.isCurrent ? "icon-button--active" : ""}`}
          title={task.isCurrent ? ui.common.cancelCurrent : ui.common.setCurrent}
          onClick={() => void toggleCurrentTask(task.id)}
        >
          <Star aria-hidden="true" />
        </button> : null}
        <button type="button" className="icon-button" title={completed ? ui.common.restore : ui.common.complete} onClick={() => void (completed ? restoreTask(task.id) : completeTask(task.id))}>
          {completed ? <RotateCcw aria-hidden="true" /> : <Check aria-hidden="true" />}
        </button>
        <button type="button" className="icon-button" title={ui.common.delete} onClick={() => void deleteTask(task.id)}>
          <Trash2 aria-hidden="true" />
        </button>
        {!completed ? <div className="postpone-control">
          <button
            type="button"
            className="text-button"
            aria-expanded={postponeMode !== "closed"}
            onClick={() => setPostponeMode((value) => (value === "closed" ? "menu" : "closed"))}
          >
            <CalendarClock aria-hidden="true" />{ui.task.postponed}<ChevronDown aria-hidden="true" />
          </button>
          {postponeMode !== "closed" ? (
            <div className="postpone-menu">
              {postponeMode === "menu" ? (
                <>
                  <button type="button" onClick={() => void handlePostpone(addDays(task.dueAt, 1))}>
                    +1天
                  </button>
                  <button type="button" onClick={() => void handlePostpone(addDays(task.dueAt, 3))}>
                    +3天
                  </button>
                  <button type="button" onClick={() => void handlePostpone(addDays(task.dueAt, 7))}>
                    +7天
                  </button>
                  <button type="button" onClick={() => setPostponeMode("custom")}>
                    {ui.task.custom}
                  </button>
                </>
              ) : (
                <>
                  <input type="datetime-local" value={customDue} onChange={(event) => setCustomDue(event.target.value)} />
                  <button type="button" onClick={() => void handlePostpone(fromDateTimeLocalValue(customDue))}>
                    {ui.common.save}
                  </button>
                </>
              )}
            </div>
          ) : null}
        </div> : null}
      </div>
    </li>
  );
}

function scrollPanelToTop() {
  const panel = document.querySelector(".expanded-panel, .settings-window");
  panel?.scrollTo({ top: 0, behavior: document.documentElement.dataset.lowPower === "true" ? "auto" : "smooth" });
}

function TaskComposer() {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const addTask = useDeadlineStore((state) => state.addTask);
  const [isOpen, setIsOpen] = useToolDisclosure("add");
  const [quickInput, setQuickInput] = useState("");
  const [quickPreview, setQuickPreview] = useState<NewTaskInput | null>(null);
  const [quickMessage, setQuickMessage] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [dueAt, setDueAt] = useState(() => toDateTimeLocalValue(new Date(Date.now() + 2 * 24 * 60 * 60 * 1000).toISOString()));
  const [priority, setPriority] = useState<TaskPriority>("medium");
  const [isSaving, setIsSaving] = useState(false);
  const saving = useRef(false);

  function handleQuickParse() {
    const result = parseQuickAdd(quickInput);
    if (!result.ok || !result.input) {
      setQuickPreview(null);
      setQuickMessage(result.error ? ui.add.quickErrors[result.error] : ui.add.quickError);
      return;
    }

    setQuickPreview(result.input);
    setQuickMessage(ui.add.quickReady);
  }

  async function handleQuickConfirm() {
    if (!quickPreview || saving.current) return;
    saving.current = true; setIsSaving(true);
    try {
    if (!await addTask(quickPreview)) return;
    setQuickInput("");
    setQuickPreview(null);
    setQuickMessage(null);
    setIsOpen(false);
    scrollPanelToTop();
    } finally { saving.current = false; setIsSaving(false); }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || !dueAt || saving.current) return;
    saving.current = true; setIsSaving(true);
    try {
    const added = await addTask({
      title,
      dueAt: fromDateTimeLocalValue(dueAt),
      priority,
      notes,
      source: "manual"
    });
    if (!added) return;
    setTitle("");
    setNotes("");
    setPriority("medium");
    setIsOpen(false);
    scrollPanelToTop();
    } finally { saving.current = false; setIsSaving(false); }
  }

  return (
    <section className="panel-section">
      <button type="button" className="add-toggle" onClick={() => setIsOpen((value) => !value)}>
        <Plus aria-hidden="true" />
        <span>{ui.add.title}</span>
      </button>

      {isOpen ? (
        <div className="task-form">
          <div className="quick-add-row">
            <input
              value={quickInput}
              onChange={(event) => {
                setQuickInput(event.target.value);
                setQuickPreview(null);
                setQuickMessage(null);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  handleQuickParse();
                }
              }}
              placeholder={ui.add.quickPlaceholder}
            />
            <button type="button" className="primary-button" onClick={handleQuickParse}>
              {ui.add.quickParse}
            </button>
          </div>

          {quickPreview ? (
            <div className="quick-preview">
              <div>
                <p>{quickPreview.title}</p>
                <span>
                  {formatDue(quickPreview.dueAt, language)} · {priorityLabel[quickPreview.priority]}
                </span>
                {quickPreview.notes ? <p className="quick-preview-notes">{quickPreview.notes}</p> : null}
              </div>
              <button type="button" className="primary-button" disabled={isSaving} onClick={() => void handleQuickConfirm()}>
                {ui.add.quickConfirm}
              </button>
            </div>
          ) : null}
          {quickMessage ? <p className="setting-message">{quickMessage}</p> : null}

          <p className="section-label">{ui.add.manual}</p>
          <form className="task-form task-form--nested" onSubmit={(event) => void handleSubmit(event)}>
          <label>
            <span>{ui.task.title}</span>
            <input value={title} onChange={(event) => setTitle(event.target.value)} placeholder={ui.task.example} />
          </label>
          <label>
            <span>{ui.task.dueLabel}</span>
            <input type="datetime-local" value={dueAt} onChange={(event) => setDueAt(event.target.value)} />
          </label>
          <label>
            <span>{ui.task.priority}</span>
            <select value={priority} onChange={(event) => setPriority(event.target.value as TaskPriority)}>
              {priorityOptions.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          <label><span>{ui.import.notes}</span><input value={notes} onChange={event => setNotes(event.target.value)}/></label>
          <button type="submit" className="primary-button" disabled={isSaving}>
            {ui.common.save}
          </button>
          </form>
        </div>
      ) : null}
    </section>
  );
}

function HistoryPanel() {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const tasks = useDeadlineStore((state) => state.tasks);
  const restoreTask = useDeadlineStore((state) => state.restoreTask);
  const deleteTask = useDeadlineStore((state) => state.deleteTask);
  const [isOpen, setIsOpen] = useToolDisclosure("history");
  const completedTasks = useMemo(
    () =>
      [...tasks]
        .filter((task) => task.status === "completed")
        .sort((a, b) => new Date(b.completedAt ?? b.updatedAt).getTime() - new Date(a.completedAt ?? a.updatedAt).getTime()),
    [tasks]
  );

  return (
    <section className="panel-section">
      <button type="button" className="add-toggle" onClick={() => setIsOpen((value) => !value)}>
        <History aria-hidden="true" />
        <span>{ui.history.title}</span>
        <span className="count-badge">{completedTasks.length}</span>
      </button>
      {isOpen ? (
        <div className="history-list">
          {completedTasks.length === 0 ? (
            <p className="empty-text">{ui.history.empty}</p>
          ) : (
            <ol className="task-list">{completedTasks.map(task => <TaskRow key={task.id} task={task}/>)}</ol>
          )}
        </div>
      ) : null}
    </section>
  );
}

function CloudSyncPanel() {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const syncWithCloud = useDeadlineStore((state) => state.syncWithCloud);
  const [isOpen, setIsOpen] = useToolDisclosure("cloud");
  const [syncUrl, setSyncUrl] = useState("");
  const [syncAnonKey, setSyncAnonKey] = useState("");
  const [syncCode, setSyncCode] = useState("");
  const [isSyncPending, setIsSyncPending] = useState(false);
  const [showSyncAdvanced, setShowSyncAdvanced] = useState(() => !hasDefaultSupabaseConfig());
  const [isConfirmingSyncCode, setIsConfirmingSyncCode] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;
    void loadSupabaseSyncSettings()
      .then((settings) => {
        if (!isMounted) return;
        setSyncUrl(settings.url);
        setSyncAnonKey(settings.anonKey);
        setSyncCode(settings.syncCode);
      })
      .catch(() => {
        if (isMounted) setSyncCode("");
      });

    return () => {
      isMounted = false;
    };
  }, [isOpen]);

  useEffect(() => {
    if (!message) return;

    const timer = window.setTimeout(() => setMessage(null), 3200);
    return () => window.clearTimeout(timer);
  }, [message]);

  useEffect(() => {
    if (!isConfirmingSyncCode) return;

    const timer = window.setTimeout(() => setIsConfirmingSyncCode(false), 6000);
    return () => window.clearTimeout(timer);
  }, [isConfirmingSyncCode]);

  async function handleSaveSyncConfig() {
    setIsSyncPending(true);
    setMessage(null);
    try {
      const code = syncCode.trim() || generateSyncCode();
      setSyncCode(code);
      await saveSupabaseSyncSettings({ url: syncUrl, anonKey: syncAnonKey, syncCode: code });
      setMessage(ui.settings.syncConfigSaved);
    } catch {
      setMessage(ui.settings.syncConfigFailed);
    } finally {
      setIsSyncPending(false);
    }
  }

  function handleGenerateSyncCode() {
    if (!isConfirmingSyncCode) {
      setIsConfirmingSyncCode(true);
      setMessage(ui.settings.confirmGenerateSyncCode);
      return;
    }

    setSyncCode(generateSyncCode());
    setIsConfirmingSyncCode(false);
    setMessage(ui.settings.syncCodeGenerated);
  }

  async function handleCopySyncCode() {
    try {
      const code = syncCode.trim();
      if (!code) return;
      await navigator.clipboard.writeText(code);
      setMessage(ui.settings.syncCodeCopied);
    } catch {
      setMessage(ui.settings.syncCodeCopyFailed);
    }
  }

  async function handleSyncNow() {
    setIsSyncPending(true);
    setMessage(ui.settings.syncingNow);
    try {
      const code = syncCode.trim() || generateSyncCode();
      setSyncCode(code);
      await saveSupabaseSyncSettings({ url: syncUrl, anonKey: syncAnonKey, syncCode: code });
      await syncWithCloud();
      setMessage(ui.settings.syncDone);
    } catch {
      setMessage(ui.settings.syncFailed);
    } finally {
      setIsSyncPending(false);
    }
  }

  return (
    <section className="panel-section panel-section--sync">
      <button type="button" className="add-toggle" onClick={() => setIsOpen((value) => !value)}>
        <Cloud aria-hidden="true" />
        <span>{ui.settings.cloudSync}</span>
      </button>

      {isOpen ? (
        <div className="settings-panel">
          <div className="settings-row settings-row--stack">
            <div>
              <p className="setting-title">{ui.settings.cloudSync}</p>
              <p className="setting-copy">{ui.settings.cloudSyncCopy}</p>
            </div>
            <div className="sync-form">
              <div className="sync-code-row">
                <input
                  className="settings-input"
                  value={syncCode}
                  placeholder={ui.settings.syncCode}
                  onChange={(event) => setSyncCode(event.target.value)}
                />
                <button type="button" className="icon-button" title={ui.settings.copySyncCode} disabled={!syncCode.trim()} onClick={() => void handleCopySyncCode()}>
                  <Copy aria-hidden="true" />
                </button>
              </div>
              {showSyncAdvanced ? (
                <>
                  <input
                    className="settings-input"
                    value={syncUrl}
                    placeholder={ui.settings.supabaseUrl}
                    onChange={(event) => setSyncUrl(event.target.value)}
                  />
                  <input
                    className="settings-input"
                    type="password"
                    value={syncAnonKey}
                    placeholder={ui.settings.supabaseAnonKey}
                    onChange={(event) => setSyncAnonKey(event.target.value)}
                  />
                </>
              ) : null}
              <div className="settings-actions">
                <button type="button" className="text-button" disabled={isSyncPending} onClick={() => void handleSaveSyncConfig()}>
                  <Cloud aria-hidden="true" />
                  {ui.settings.saveSyncConfig}
                </button>
                <button type="button" className="text-button" disabled={isSyncPending} onClick={handleGenerateSyncCode}>
                  <RefreshCw aria-hidden="true" />
                  {isConfirmingSyncCode ? ui.settings.confirmGenerateSyncCodeButton : ui.settings.generateSyncCode}
                </button>
                <button type="button" className="text-button" onClick={() => setShowSyncAdvanced((value) => !value)}>
                  <KeyRound aria-hidden="true" />
                  {showSyncAdvanced ? ui.settings.hideSyncAdvanced : ui.settings.showSyncAdvanced}
                </button>
                <button type="button" className="text-button" disabled={isSyncPending} onClick={() => void handleSyncNow()}>
                  <RefreshCw aria-hidden="true" />
                  {ui.settings.syncNow}
                </button>
              </div>
              {message ? <p className="setting-message">{message}</p> : null}
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function SettingsPanel({ standalone = false }: { standalone?: boolean }) {
  const language = useDeadlineStore((state) => state.language);
  const setLanguage = useDeadlineStore((state) => state.setLanguage);
  const ui = getStrings(language);
  const focusLimit = useDeadlineStore((state) => state.focusLimit);
  const setFocusLimit = useDeadlineStore((state) => state.setFocusLimit);
  const [isAvailable] = useState(() => isTauriRuntime());
  const isOpen = standalone;
  const [isOpeningSettings, setIsOpeningSettings] = useState(false);
  const openingSettings = useRef(false);
  const [isAutostartEnabled, setIsAutostartEnabled] = useState(false);
  const [isAutostartPending, setIsAutostartPending] = useState(false);
  const [isUpdatePending, setIsUpdatePending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isAvailable || !isOpen) return;

    let isMounted = true;
    void getAutostartEnabled()
      .then((enabled) => {
        if (isMounted) setIsAutostartEnabled(enabled);
      })
      .catch(() => {
        if (isMounted) setMessage(ui.settings.autostartReadFailed);
      });

    return () => {
      isMounted = false;
    };
  }, [isAvailable, isOpen, ui.settings.autostartReadFailed]);

  useEffect(() => {
    if (!message) return;

    const timer = window.setTimeout(() => setMessage(null), 3200);
    return () => window.clearTimeout(timer);
  }, [message]);

  if (!isAvailable && !standalone) {
    return null;
  }

  async function handleAutostartToggle(nextEnabled: boolean) {
    setIsAutostartPending(true);
    setMessage(null);
    try {
      const confirmed = await setAutostartEnabled(nextEnabled);
      setIsAutostartEnabled(confirmed);
      setMessage(confirmed ? ui.settings.autostartOn : ui.settings.autostartOff);
    } catch {
      setMessage(ui.settings.autostartFailed);
    } finally {
      setIsAutostartPending(false);
    }
  }

  async function handleBackup() {
    try {
      const backupPath = await backupDatabase();
      setMessage(backupPath ? `${ui.settings.backupTo} ${backupPath}` : ui.settings.backupDone);
    } catch {
      setMessage(ui.settings.backupFailed);
    }
  }

  async function handleOpenDataDir() {
    try {
      await openDataDir();
    } catch {
      setMessage(ui.settings.dataDirFailed);
    }
  }

  async function handleResetPosition() {
    try {
      await resetPanelPosition();
      setMessage(ui.settings.resetDone);
    } catch {
      setMessage(ui.settings.resetFailed);
    }
  }

  async function handleHideFor(minutes: number) {
    try {
      await hidePanelForMinutes(minutes);
    } catch {
      setMessage(ui.settings.hideFailed);
    }
  }

  async function handleOpenSettings() {
    if (standalone || openingSettings.current) return;
    openingSettings.current = true; setIsOpeningSettings(true);
    try { await openSettingsWindow(ui.settings.title); }
    catch (error) { setMessage(String(error)); }
    finally { openingSettings.current = false; setIsOpeningSettings(false); }
  }

  async function handleCheckUpdates() {
    setIsUpdatePending(true);
    setMessage(ui.settings.checkingUpdates);
    try {
      if (isTauriRuntime()) {
        const updaterText = updaterStatusText[resolveLanguage(language)];
        const update = await check();
        if (!update) {
          setMessage(formatTemplate(ui.settings.upToDate, { version: APP_VERSION }));
          return;
        }

        let downloadedBytes = 0;
        let totalBytes = 0;
        const version = normalizeVersion(update.version);
        const confirmed = window.confirm(formatTemplate(updaterText.confirmInstall, { version }));
        if (!confirmed) {
          setMessage(updaterText.cancelled);
          return;
        }

        setMessage(formatTemplate(updaterText.available, { version }));
        await update.downloadAndInstall((event: DownloadEvent) => {
          if (event.event === "Started") {
            downloadedBytes = 0;
            totalBytes = event.data.contentLength ?? 0;
            setMessage(updaterText.downloading);
            return;
          }
          if (event.event === "Progress") {
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              const progress = Math.min(99, Math.round((downloadedBytes / totalBytes) * 100));
              setMessage(formatTemplate(updaterText.downloadingProgress, { progress: String(progress) }));
            } else {
              setMessage(updaterText.downloading);
            }
            return;
          }
          setMessage(updaterText.installing);
        }, { timeout: UPDATE_DOWNLOAD_TIMEOUT_MS });

        setMessage(updaterText.restarting);
        await restartApp();
        return;
      }

      const response = await fetch(RELEASES_API_URL, {
        headers: {
          Accept: "application/vnd.github+json"
        }
      });

      if (response.status === 404) {
        setMessage(ui.settings.noReleaseFound);
        return;
      }
      if (!response.ok) {
        throw new Error(`Update check failed: ${response.status}`);
      }

      const release = (await response.json()) as LatestReleaseResponse;
      const latestVersion = normalizeVersion(release.tag_name ?? "");
      if (!latestVersion) {
        setMessage(ui.settings.noReleaseFound);
        return;
      }

      if (compareVersions(latestVersion, APP_VERSION) > 0) {
        setMessage(formatTemplate(ui.settings.updateAvailable, {
          version: latestVersion,
          url: release.html_url ?? "https://github.com/xy-tsuki/deadline-panel/releases"
        }));
      } else {
        setMessage(formatTemplate(ui.settings.upToDate, { version: APP_VERSION }));
      }
    } catch {
      setMessage(ui.settings.updateCheckFailed);
    } finally {
      setIsUpdatePending(false);
    }
  }

  return (
    <section className="panel-section panel-section--settings">
      <button type="button" className="add-toggle" disabled={isOpeningSettings} onClick={() => void handleOpenSettings()}>
        <Settings aria-hidden="true" />
        <span>{ui.settings.title}</span>
      </button>

      {message && !standalone ? <p className="setting-message" role="alert">{message}</p> : null}
      {isOpen ? (
        <div className="settings-panel">
          <div className="settings-preferences">
          <div className="settings-row">
            <div>
              <p className="setting-title">{ui.settings.focusCount}</p>
              <p className="setting-copy">{ui.settings.focusCopy}</p>
            </div>
            <TopNSegments value={focusLimit} label={ui.settings.focusCount} onChange={setFocusLimit} />
          </div>

          <div className="settings-row">
            <div>
              <p className="setting-title">{ui.settings.language}</p>
              <p className="setting-copy">{ui.settings.languageCopy}</p>
            </div>
            <LiquidTextSegments value={language} label={ui.settings.language} options={languageOptions.map(value => ({ value, label: languageName(value, language) }))} onChange={setLanguage}/>
          </div>

          <AppearanceSettings part="theme" />
          <div className="settings-row settings-row--toggle">
            <label className="setting-check">
              <input
                type="checkbox"
                checked={isAutostartEnabled}
                disabled={isAutostartPending}
                onChange={(event) => void handleAutostartToggle(event.target.checked)}
              />
              <span>{ui.settings.autostart}</span>
            </label>
          </div>

          <AppearanceSettings part="behavior" />
          </div>
          <div className="settings-notifications"><NotificationSettings /></div>
          <div className="settings-actions">
            <button type="button" className="text-button" onClick={() => void showPanel().catch(error => setMessage(String(error)))}><Star aria-hidden="true"/>{({ zh: "显示面板", ja: "パネルを表示", en: "Show panel" })[resolveLanguage(language)]}</button>
            <button type="button" className="text-button" onClick={() => void handleHideFor(15)}><EyeOff aria-hidden="true"/>{ui.settings.hide15}</button>
            <button type="button" className="text-button" onClick={() => void handleHideFor(30)}><EyeOff aria-hidden="true"/>{ui.settings.hide30}</button>
            <button type="button" className="text-button" onClick={handleResetPosition}>
              <RotateCcw aria-hidden="true" />
              {ui.settings.resetPosition}
            </button>
            <button type="button" className="text-button" onClick={handleOpenDataDir}>
              <FolderOpen aria-hidden="true" />
              {ui.settings.dataDir}
            </button>
            <button type="button" className="text-button" onClick={() => void handleBackup()}>
              <Database aria-hidden="true" />
              {ui.settings.backup}
            </button>
            <button type="button" className="text-button" disabled={isUpdatePending} onClick={() => void handleCheckUpdates()}>
              <RefreshCw aria-hidden="true" />
              {ui.settings.checkUpdates}
            </button>
            <JsonFileControls />
          </div>

          {message ? <p className="setting-message">{message}</p> : null}
        </div>
      ) : null}
    </section>
  );
}

function AutostartSetting() {
  const [isAvailable] = useState(() => isTauriRuntime());
  const [isEnabled, setIsEnabled] = useState(false);
  const [isPending, setIsPending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isAvailable) return;

    let isMounted = true;
    void getAutostartEnabled()
      .then((enabled) => {
        if (isMounted) setIsEnabled(enabled);
      })
      .catch(() => {
        if (isMounted) setMessage("无法读取开机启动状态");
      });

    return () => {
      isMounted = false;
    };
  }, [isAvailable]);

  if (!isAvailable) {
    return null;
  }

  async function handleToggle(nextEnabled: boolean) {
    setIsPending(true);
    setMessage(null);
    try {
      const confirmed = await setAutostartEnabled(nextEnabled);
      setIsEnabled(confirmed);
      setMessage(confirmed ? "已开启开机启动" : "已关闭开机启动");
    } catch {
      setMessage("开机启动设置失败");
    } finally {
      setIsPending(false);
    }
  }

  return (
    <section className="panel-section setting-row">
      <div>
        <p className="setting-title">开机启动</p>
        <p className="setting-copy">打开电脑后自动显示右下角 Deadline 小条。</p>
        {message ? <p className="setting-message">{message}</p> : null}
      </div>
      <label className="switch" aria-label="开机启动">
        <input
          type="checkbox"
          checked={isEnabled}
          disabled={isPending}
          onChange={(event) => void handleToggle(event.target.checked)}
        />
        <span />
      </label>
    </section>
  );
}

function ImportPanel() {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);

  const bulkAddTasks = useDeadlineStore((state) => state.bulkAddTasks);

  const runCommand = useDeadlineStore((state) => state.runCommand);
  const clearCommandMessage = useDeadlineStore((state) => state.clearCommandMessage);
  const [isOpen, setIsOpen] = useToolDisclosure("import");
  const [rawImport, setRawImport] = useState("");
  const [previewRows, setPreviewRows] = useState<ImportPreviewRow[]>([]);
  const [localMessage, setLocalMessage] = useState<string | null>(null);

  const importPanelRef = useRef<HTMLDivElement | null>(null);
  const previewRef = useRef<HTMLDivElement | null>(null);

  async function copyPrompt() {
    try {
      await navigator.clipboard.writeText(getImportPrompt(language));
      setLocalMessage(ui.import.promptCopied);
    } catch {
      setLocalMessage(ui.import.promptCopyFailed);
    }
  }

  function parsePreview() {
    clearCommandMessage();
    const lines = rawImport
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    if (lines.length === 0) {
      setPreviewRows([]);
      setLocalMessage(ui.import.pasteFirst);
      return;
    }

    const rows = lines.map((line, index): ImportPreviewRow => {
      const result = parseCommand(line);
      if (!result.ok || !result.command) {
        return emptyPreviewRow(index + 1, result.error ?? ui.import.unknownCommand);
      }

      if (result.command.kind !== "add") {
        return emptyPreviewRow(index + 1, ui.import.addOnly);
      }

      return {
        id: crypto.randomUUID(),
        lineNumber: index + 1,
        ok: true,
        title: result.command.payload.title,
        dueAt: result.command.payload.dueAt,
        priority: result.command.payload.priority,
        notes: result.command.payload.notes ?? ""
      };
    });

    setPreviewRows(rows);
    scrollImportContentIntoView();
    setLocalMessage(rows.some((row) => !row.ok) ? ui.import.fixBeforeImport : ui.import.previewReady);
  }

  async function executeSingleCommand() {
    if (!rawImport.trim()) return;
    if (!await runCommand(rawImport.trim())) return;
    setPreviewRows([]);
    setRawImport("");
    setIsOpen(false);
    scrollPanelToTop();
  }

  async function confirmImport() {
    const inputs: NewTaskInput[] = previewRows
      .filter((row) => row.ok && row.title.trim() && row.dueAt)
      .map((row) => ({
        title: row.title,
        dueAt: row.dueAt,
        priority: row.priority,
        notes: row.notes,
        source: "command"
      }));

    if (inputs.length === 0) {
      setLocalMessage(ui.import.nothingToImport);
      return;
    }

    if (!await bulkAddTasks(inputs)) return;
    setRawImport("");
    setPreviewRows([]);
    setLocalMessage(null);
    setIsOpen(false);
    scrollPanelToTop();
  }

  function updatePreviewRow(id: string, fields: Partial<ImportPreviewRow>) {
    setPreviewRows((rows) => rows.map((row) => (row.id === id ? { ...row, ...fields } : row)));
  }

  function deletePreviewRow(id: string) {
    setPreviewRows((rows) => rows.filter((row) => row.id !== id));
  }

  function handleImportToggle() {
    setIsOpen((value) => {
      const next = !value;
      if (next) {
        scrollImportContentIntoView();
      }
      return next;
    });
  }

  function scrollImportContentIntoView() {
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        const target = previewRef.current ?? importPanelRef.current;
        target?.scrollIntoView({ block: "end", behavior: document.documentElement.dataset.lowPower === "true" ? "auto" : "smooth" });
      });
    });
  }

  return (
    <section className="panel-section panel-section--command">
      <div className="import-toolbar">
        <button type="button" className="add-toggle" onClick={handleImportToggle}>
          <Plus aria-hidden="true" />
          <span>{ui.import.title}</span>
        </button>
        <button type="button" className="text-button" onClick={() => void copyPrompt()}>
          <Copy aria-hidden="true" />
          {ui.import.copyPrompt}
        </button>
        <JsonFileControls />
      </div>

      {isOpen ? (
        <div className="import-panel" ref={importPanelRef}>
          <textarea
            value={rawImport}
            onChange={(event) => {
              setRawImport(event.target.value);
              clearCommandMessage();
            }}
            placeholder='/add title="Graph Mining Quiz" due="2026-06-23 23:59" priority="high"'
          />
          <div className="import-actions">
            <button type="button" className="primary-button" onClick={parsePreview}>
              {ui.import.parsePreview}
            </button>
            <button type="button" className="text-button" onClick={() => void executeSingleCommand()}>
              {ui.import.runCommand}
            </button>
          </div>

          {previewRows.length > 0 ? (
            <div className="import-preview" ref={previewRef}>
              {previewRows.map((row) =>
                row.ok ? (
                  <div className="preview-row" key={row.id}>
                    <span className="preview-line">{row.lineNumber}</span>
                    <input value={row.title} onChange={(event) => updatePreviewRow(row.id, { title: event.target.value })} />
                    <input
                      type="datetime-local"
                      value={toDateTimeLocalValue(row.dueAt)}
                      onChange={(event) => {
                        if (event.target.value) {
                          updatePreviewRow(row.id, { dueAt: fromDateTimeLocalValue(event.target.value) });
                        }
                      }}
                    />
                    <select
                      value={row.priority}
                      onChange={(event) => updatePreviewRow(row.id, { priority: event.target.value as TaskPriority })}
                    >
                      {priorityOptions.map((option) => (
                        <option key={option} value={option}>
                          {option}
                        </option>
                      ))}
                    </select>
                    <button type="button" className="icon-button preview-delete" title={ui.common.remove} onClick={() => deletePreviewRow(row.id)}>
                      <Trash2 aria-hidden="true" />
                    </button>
                    <input value={row.notes} onChange={(event) => updatePreviewRow(row.id, { notes: event.target.value })} placeholder={ui.import.notes} />
                  </div>
                ) : (
                  <div className="preview-row preview-row--error" key={row.id}>
                    <span className="preview-line">{row.lineNumber}</span>
                    <p>{row.error}</p>
                    <button type="button" className="icon-button preview-delete" title={ui.common.remove} onClick={() => deletePreviewRow(row.id)}>
                      <Trash2 aria-hidden="true" />
                    </button>
                  </div>
                )
              )}
              <button type="button" className="primary-button" onClick={() => void confirmImport()}>
                {ui.import.confirmImport}
              </button>
            </div>
          ) : null}
        </div>
      ) : null}
      {localMessage ? <p className="command-message">{localMessage}</p> : null}
    </section>
  );
}

function emptyPreviewRow(lineNumber: number, error: string): ImportPreviewRow {
  return {
    id: crypto.randomUUID(),
    lineNumber,
    ok: false,
    error,
    title: "",
    dueAt: "",
    priority: "medium",
    notes: ""
  };
}

function addDays(isoDate: string, days: number): string {
  const date = new Date(isoDate);
  const base = Number.isNaN(date.getTime()) ? new Date() : date;
  return new Date(base.getTime() + days * 24 * 60 * 60 * 1000).toISOString();
}

function getDueTone(isoDate: string): "overdue" | "today" | "soon" | "normal" {
  const due = new Date(isoDate).getTime();
  if (Number.isNaN(due)) return "normal";

  const diff = due - Date.now();
  const oneDay = 24 * 60 * 60 * 1000;
  if (diff < 0) return "overdue";
  if (diff <= oneDay) return "today";
  if (diff <= oneDay * 3) return "soon";
  return "normal";
}

function normalizeVersion(version: string): string {
  return version.trim().replace(/^v/i, "");
}

function compareVersions(left: string, right: string): number {
  const leftParts = normalizeVersion(left).split(".").map((part) => Number.parseInt(part, 10) || 0);
  const rightParts = normalizeVersion(right).split(".").map((part) => Number.parseInt(part, 10) || 0);
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const diff = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (diff !== 0) return diff;
  }

  return 0;
}

function formatTemplate(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce((result, [key, value]) => result.split(`{${key}}`).join(value), template);
}

function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}

function isInteractiveTarget(target: EventTarget): boolean {
  return target instanceof Element && Boolean(target.closest("button,input,select,textarea,a"));
}

function isTextEditingTarget(target: EventTarget): boolean {
  return target instanceof Element && Boolean(target.closest("input,select,textarea"));
}
