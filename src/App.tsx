import { type FormEvent, type MouseEvent, type PointerEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  ChevronRight,
  Clock3,
  Copy,
  Database,
  EyeOff,
  FolderOpen,
  History,
  Plus,
  RefreshCw,
  RotateCcw,
  Settings,
  Star,
  Trash2,
  Upload,
  Download
} from "lucide-react";
import { parseCommand } from "./domain/commandParser";
import { parseQuickAdd } from "./domain/quickAddParser";
import { formatDue, formatTimeLeft, fromDateTimeLocalValue, toDateTimeLocalValue } from "./domain/date";
import { getCurrentTasks, getNearestDeadline, sortDeadlineTasks } from "./domain/taskSorting";
import { DeadlineTask, NewTaskInput, TaskPriority, priorityLabel } from "./domain/task";
import { FocusLimit, selectFocusTasks, useDeadlineStore } from "./store/deadlineStore";
import { getImportPrompt, getStrings, languageName, languageOptions } from "./i18n";
import {
  getAutostartEnabled,
  backupDatabase,
  getDataFilePath,
  getPanelPointerState,
  hidePanelTemporarily,
  hidePanelForMinutes,
  isForegroundWindowFullscreen,
  isTauriRuntime,
  openDataDir,
  resetPanelPosition,
  setAutostartEnabled,
  setPanelAcceptsInput,
  setPanelAutoHidden,
  setPanelExpanded as setNativePanelExpanded,
  showPanelContextMenu,
  startPanelDrag
} from "./runtime/tauri";
import { APP_VERSION } from "./version";

const priorityOptions: TaskPriority[] = ["urgent", "high", "medium", "low"];
const focusLimitOptions: FocusLimit[] = [3, 5, 10];
const COLLAPSE_DELAY_MS = 220;
const COLLAPSE_ANIMATION_MS = 210;
const FULLSCREEN_CHECK_INTERVAL_MS = 1200;
const TRIGGER_ACTIVE_CHECK_INTERVAL_MS = 16;
const TRIGGER_IDLE_CHECK_INTERVAL_MS = 140;
const COLLAPSED_HOVER_DWELL_MS = 180;
const RELEASES_API_URL = "https://api.github.com/repos/xy-tsuki/deadline-panel/releases/latest";

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
  const collapseTimerRef = useRef<number | null>(null);
  const animationTimerRef = useRef<number | null>(null);
  const lastAutoHiddenRef = useRef(false);
  const hoverStartRef = useRef<number | null>(null);
  const pointerButtonsRef = useRef({ leftDown: false, rightDown: false });
  const isDraggingPanelRef = useRef(false);
  const collapsedInputRef = useRef(false);
  const forceExpanded = new URLSearchParams(window.location.search).get("panel") === "expanded";
  const tasks = useDeadlineStore((state) => state.tasks);
  const focusLimit = useDeadlineStore((state) => state.focusLimit);
  const isLoading = useDeadlineStore((state) => state.isLoading);
  const error = useDeadlineStore((state) => state.error);
  const commandMessage = useDeadlineStore((state) => state.commandMessage);
  const clearCommandMessage = useDeadlineStore((state) => state.clearCommandMessage);
  const load = useDeadlineStore((state) => state.load);

  const focusTasks = useMemo(() => selectFocusTasks(tasks, focusLimit), [focusLimit, tasks]);
  const nearest = useMemo(() => getNearestDeadline(tasks), [tasks]);
  const currentTasks = useMemo(() => getCurrentTasks(tasks), [tasks]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (forceExpanded) {
      expandPanel();
    }
  }, [forceExpanded]);

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
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let isMounted = true;
    async function syncFullscreenState() {
      try {
        const isFullscreen = await isForegroundWindowFullscreen();
        if (!isMounted || isFullscreen === lastAutoHiddenRef.current) return;

        lastAutoHiddenRef.current = isFullscreen;
        if (isFullscreen) {
          setIsExpanded(false);
          clearAnimationTimer();
        }
        await setPanelAutoHidden(isFullscreen);
      } catch {
        // Fullscreen detection is best-effort; the panel should keep working if it fails.
      }
    }

    void syncFullscreenState();
    const interval = window.setInterval(() => void syncFullscreenState(), FULLSCREEN_CHECK_INTERVAL_MS);
    return () => {
      isMounted = false;
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let isMounted = true;
    let timer: number | null = null;

    async function pollPointerState() {
      let nextInterval = TRIGGER_IDLE_CHECK_INTERVAL_MS;

      if (!isMounted) return;
      if (lastAutoHiddenRef.current || ((isExpanded || forceExpanded) && !isDraggingPanelRef.current)) {
        timer = window.setTimeout(() => void pollPointerState(), nextInterval);
        return;
      }

      try {
        const state = await getPanelPointerState();
        if (!isMounted || !state) return;

        const previousButtons = pointerButtonsRef.current;
        const shouldAcceptCollapsedInput = !isExpanded && !forceExpanded && state.inTrigger;
        if (collapsedInputRef.current !== shouldAcceptCollapsedInput) {
          collapsedInputRef.current = shouldAcceptCollapsedInput;
          void setPanelAcceptsInput(shouldAcceptCollapsedInput);
        }

        if (state.inTrigger && state.rightDown && !previousButtons.rightDown) {
          hoverStartRef.current = null;
          void showPanelContextMenu();
        }

        if (state.inTrigger && state.leftDown && !previousButtons.leftDown) {
          hoverStartRef.current = null;
          isDraggingPanelRef.current = true;
          void startPanelDrag();
        }

        if (!state.leftDown && previousButtons.leftDown && isDraggingPanelRef.current) {
          isDraggingPanelRef.current = false;
        }

        const isBusyWithButtons = state.leftDown || state.rightDown || isDraggingPanelRef.current;
        if (!isExpanded && !forceExpanded && state.inTrigger && !isBusyWithButtons) {
          hoverStartRef.current ??= Date.now();
          nextInterval = TRIGGER_ACTIVE_CHECK_INTERVAL_MS;
          if (Date.now() - hoverStartRef.current >= COLLAPSED_HOVER_DWELL_MS) {
            expandPanel();
            hoverStartRef.current = null;
          }
        } else if (!state.inTrigger) {
          hoverStartRef.current = null;
        }

        if (state.inTrigger || isBusyWithButtons) {
          nextInterval = TRIGGER_ACTIVE_CHECK_INTERVAL_MS;
        }

        pointerButtonsRef.current = {
          leftDown: state.leftDown,
          rightDown: state.rightDown
        };
      } catch {
        // Cursor polling is a Windows overlay nicety; ignore transient failures.
      }

      if (isMounted) {
        timer = window.setTimeout(() => void pollPointerState(), nextInterval);
      }
    }

    timer = window.setTimeout(() => void pollPointerState(), 0);

    return () => {
      isMounted = false;
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
  }, [forceExpanded, isExpanded]);

  function clearAnimationTimer() {
    if (animationTimerRef.current !== null) {
      window.clearTimeout(animationTimerRef.current);
      animationTimerRef.current = null;
    }
  }

  function clearCollapseTimer() {
    if (collapseTimerRef.current !== null) {
      window.clearTimeout(collapseTimerRef.current);
      collapseTimerRef.current = null;
    }
  }

  function expandPanel() {
    clearCollapseTimer();
    clearAnimationTimer();
    collapsedInputRef.current = false;
    setIsExpanded(true);
    void setNativePanelExpanded(true);
  }

  function collapsePanel() {
    clearAnimationTimer();
    setIsExpanded(false);
    animationTimerRef.current = window.setTimeout(() => {
      collapsedInputRef.current = false;
      void setNativePanelExpanded(false);
      animationTimerRef.current = null;
    }, COLLAPSE_ANIMATION_MS);
  }

  function handlePointerEnter() {
    if (!isTauriRuntime()) {
      expandPanel();
    }
  }

  function handlePointerLeave() {
    if (forceExpanded) return;

    clearCollapseTimer();
    collapseTimerRef.current = window.setTimeout(() => {
      collapsePanel();
      collapseTimerRef.current = null;
    }, COLLAPSE_DELAY_MS);
  }

  async function handleHideTemporarily() {
    clearCollapseTimer();
    clearAnimationTimer();
    setIsExpanded(false);
    await setNativePanelExpanded(false);
    await hidePanelTemporarily();
  }

  function handlePanelContextMenu(event: MouseEvent<HTMLElement>) {
    if (isTextEditingTarget(event.target)) return;

    event.preventDefault();
    void showPanelContextMenu();
  }

  function handleExpandedDragStart(event: PointerEvent<HTMLElement>) {
    if (event.button !== 0 || isInteractiveTarget(event.target) || isScrollbarPointer(event)) return;

    event.preventDefault();
    hoverStartRef.current = null;
    isDraggingPanelRef.current = true;
    pointerButtonsRef.current = { ...pointerButtonsRef.current, leftDown: true };
    void startPanelDrag();
  }

  return (
    <main className="desktop-stage">
      <section
        className={`deadline-widget ${isExpanded || forceExpanded ? "deadline-widget--expanded" : ""}`}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onContextMenu={handlePanelContextMenu}
        aria-label="Deadline Panel"
      >
        <CollapsedStrip
          currentTasks={currentTasks}
          total={focusTasks.length}
          nearest={nearest}
          isLoading={isLoading}
          onStartDrag={handleExpandedDragStart}
        />
        <ExpandedPanel
          currentTasks={currentTasks}
          focusTasks={focusTasks}
          focusLimit={focusLimit}
          error={error}
          message={commandMessage}
          onHideTemporarily={handleHideTemporarily}
          onStartDrag={handleExpandedDragStart}
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
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const summary = isLoading
    ? ui.panel.restoring
    : currentTasks.length > 0
      ? `${ui.panel.currentPrefix}${currentTasks.map((task) => task.title).join(" + ")}｜${formatTimeLeft(currentTasks[0].dueAt, new Date(), language)}`
      : nearest
        ? `Top ${total}｜${ui.panel.nearestPrefix}${nearest.title}（${formatTimeLeft(nearest.dueAt, new Date(), language)}）`
        : ui.panel.emptyCollapsed;

  return (
    <div className="collapsed-strip" onPointerDown={onStartDrag}>
      <Clock3 aria-hidden="true" />
      <span>{summary}</span>
      <ChevronRight aria-hidden="true" />
    </div>
  );
}

function ExpandedPanel({
  currentTasks,
  focusTasks,
  focusLimit,
  error,
  message,
  onHideTemporarily,
  onStartDrag
}: {
  currentTasks: DeadlineTask[];
  focusTasks: DeadlineTask[];
  focusLimit: FocusLimit;
  error: string | null;
  message: string | null;
  onHideTemporarily: () => void;
  onStartDrag: (event: PointerEvent<HTMLElement>) => void;
}) {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const firstReminder = currentTasks[0] ?? focusTasks[0];

  return (
    <div className="expanded-panel" onPointerDown={onStartDrag}>
      {error ? <div className="notice notice--error">{error}</div> : null}
      {message ? <div className="notice">{message}</div> : null}

      <section className="panel-section panel-section--focus">
        <div className="focus-topline">
          <p className="section-label">{ui.panel.inProgress}</p>
          <button type="button" className="icon-button icon-button--quiet" title={ui.panel.hideTemporarily} onClick={onHideTemporarily}>
            <EyeOff aria-hidden="true" />
          </button>
        </div>
        {firstReminder ? (
          <>
            <h1>{firstReminder.title}</h1>
            <div className="focus-meta">
              <span>{currentTasks.length > 0 ? ui.panel.currentTask : ui.panel.nearestDeadline}</span>
              <span>{formatTimeLeft(firstReminder.dueAt, new Date(), language)}</span>
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

      <TaskComposer />
      <HistoryPanel />
      <SettingsPanel />
      <ImportPanel />
    </div>
  );
}

function FocusLimitControl({ selected }: { selected: FocusLimit }) {
  const setFocusLimit = useDeadlineStore((state) => state.setFocusLimit);
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);

  return (
    <div className="segmented-control" aria-label={ui.focus.countLabel}>
      {focusLimitOptions.map((limit) => (
        <button
          key={limit}
          type="button"
          className={selected === limit ? "segmented-control__button segmented-control__button--active" : "segmented-control__button"}
          onClick={() => void setFocusLimit(limit)}
        >
          Top {limit}
        </button>
      ))}
    </div>
  );
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

function TaskRow({ task, index }: { task: DeadlineTask; index: number }) {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const updateTask = useDeadlineStore((state) => state.updateTask);
  const completeTask = useDeadlineStore((state) => state.completeTask);
  const postponeTask = useDeadlineStore((state) => state.postponeTask);
  const toggleCurrentTask = useDeadlineStore((state) => state.toggleCurrentTask);
  const deleteTask = useDeadlineStore((state) => state.deleteTask);
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [isEditingDue, setIsEditingDue] = useState(false);
  const [draftTitle, setDraftTitle] = useState(task.title);
  const [draftDue, setDraftDue] = useState(() => toDateTimeLocalValue(task.dueAt));
  const [postponeMode, setPostponeMode] = useState<"closed" | "menu" | "custom">("closed");
  const [priorityMenuOpen, setPriorityMenuOpen] = useState(false);
  const [customDue, setCustomDue] = useState(() => toDateTimeLocalValue(addDays(task.dueAt, 1)));

  useEffect(() => {
    setDraftTitle(task.title);
  }, [task.title]);

  useEffect(() => {
    setDraftDue(toDateTimeLocalValue(task.dueAt));
    setCustomDue(toDateTimeLocalValue(addDays(task.dueAt, 1)));
  }, [task.dueAt]);

  async function commitTitle() {
    const title = draftTitle.trim();
    setIsEditingTitle(false);
    if (!title) {
      setDraftTitle(task.title);
      return;
    }
    if (title !== task.title) {
      await updateTask(task.id, { title });
    }
  }

  async function commitDue() {
    setIsEditingDue(false);
    if (!draftDue) {
      setDraftDue(toDateTimeLocalValue(task.dueAt));
      return;
    }
    const dueAt = fromDateTimeLocalValue(draftDue);
    if (dueAt !== task.dueAt) {
      await updateTask(task.id, { dueAt });
    }
  }

  async function handlePostpone(dueAt: string) {
    await postponeTask(task.id, dueAt);
    setPostponeMode("closed");
  }

  async function handlePriorityChange(priority: TaskPriority) {
    setPriorityMenuOpen(false);
    if (priority !== task.priority) {
      await updateTask(task.id, { priority });
    }
  }

  return (
    <li className={`task-row task-row--${task.priority} task-row--due-${getDueTone(task.dueAt)} ${task.isCurrent ? "task-row--current" : ""}`}>
      <span className="task-index">{index}</span>
      <div className="task-copy">
        {isEditingTitle ? (
          <input
            className="inline-edit inline-edit--title"
            autoFocus
            value={draftTitle}
            onChange={(event) => setDraftTitle(event.target.value)}
            onBlur={() => void commitTitle()}
            onKeyDown={(event) => {
              if (event.key === "Enter") void commitTitle();
              if (event.key === "Escape") {
                setDraftTitle(task.title);
                setIsEditingTitle(false);
              }
            }}
          />
        ) : (
          <button type="button" className="task-title task-title-button" onClick={() => setIsEditingTitle(true)}>
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
              if (event.key === "Enter") void commitDue();
              if (event.key === "Escape") {
                setDraftDue(toDateTimeLocalValue(task.dueAt));
                setIsEditingDue(false);
              }
            }}
          />
        ) : (
          <button type="button" className="task-due task-due-button" onClick={() => setIsEditingDue(true)}>
            {ui.task.due}
            {formatDue(task.dueAt, language)}
          </button>
        )}
      </div>
      <div className="task-side">
        <span>{formatTimeLeft(task.dueAt, new Date(), language)}</span>
        <div className="priority-control">
          <button
            type="button"
            className={`priority-pill priority-pill--${task.priority}`}
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
        <button
          type="button"
          className={`icon-button ${task.isCurrent ? "icon-button--active" : ""}`}
          title={task.isCurrent ? ui.common.cancelCurrent : ui.common.setCurrent}
          onClick={() => void toggleCurrentTask(task.id)}
        >
          <Star aria-hidden="true" />
        </button>
        <button type="button" className="icon-button" title={ui.common.complete} onClick={() => void completeTask(task.id)}>
          <Check aria-hidden="true" />
        </button>
        <div className="postpone-control">
          <button
            type="button"
            className="text-button"
            onClick={() => setPostponeMode((value) => (value === "closed" ? "menu" : "closed"))}
          >
            {ui.task.postponed}
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
        </div>
        <button type="button" className="icon-button" title={ui.common.delete} onClick={() => void deleteTask(task.id)}>
          <Trash2 aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}

function TaskComposer() {
  const language = useDeadlineStore((state) => state.language);
  const ui = getStrings(language);
  const addTask = useDeadlineStore((state) => state.addTask);
  const [isOpen, setIsOpen] = useState(false);
  const [quickInput, setQuickInput] = useState("");
  const [quickPreview, setQuickPreview] = useState<NewTaskInput | null>(null);
  const [quickMessage, setQuickMessage] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [dueAt, setDueAt] = useState(() => toDateTimeLocalValue(new Date(Date.now() + 2 * 24 * 60 * 60 * 1000).toISOString()));
  const [priority, setPriority] = useState<TaskPriority>("medium");

  function handleQuickParse() {
    const result = parseQuickAdd(quickInput);
    if (!result.ok || !result.input) {
      setQuickPreview(null);
      setQuickMessage(result.error === "empty" ? ui.add.quickEmpty : ui.add.quickError);
      return;
    }

    setQuickPreview(result.input);
    setQuickMessage(ui.add.quickReady);
  }

  async function handleQuickConfirm() {
    if (!quickPreview) return;

    await addTask(quickPreview);
    setQuickInput("");
    setQuickPreview(null);
    setQuickMessage(null);
    setIsOpen(false);
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || !dueAt) return;

    await addTask({
      title,
      dueAt: fromDateTimeLocalValue(dueAt),
      priority,
      source: "manual"
    });
    setTitle("");
    setPriority("medium");
    setIsOpen(false);
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
              </div>
              <button type="button" className="primary-button" onClick={() => void handleQuickConfirm()}>
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
          <button type="submit" className="primary-button">
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
  const [isOpen, setIsOpen] = useState(false);
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
            completedTasks.map((task) => (
              <div className="history-row" key={task.id}>
                <div>
                  <p>{task.title}</p>
                  <span>{formatDue(task.dueAt, language)}</span>
                </div>
                <button type="button" className="icon-button" title={ui.common.restore} onClick={() => void restoreTask(task.id)}>
                  <RotateCcw aria-hidden="true" />
                </button>
                <button type="button" className="icon-button" title={ui.common.delete} onClick={() => void deleteTask(task.id)}>
                  <Trash2 aria-hidden="true" />
                </button>
              </div>
            ))
          )}
        </div>
      ) : null}
    </section>
  );
}

function SettingsPanel() {
  const language = useDeadlineStore((state) => state.language);
  const setLanguage = useDeadlineStore((state) => state.setLanguage);
  const ui = getStrings(language);
  const focusLimit = useDeadlineStore((state) => state.focusLimit);
  const setFocusLimit = useDeadlineStore((state) => state.setFocusLimit);
  const [isAvailable] = useState(() => isTauriRuntime());
  const [isOpen, setIsOpen] = useState(false);
  const [isAutostartEnabled, setIsAutostartEnabled] = useState(false);
  const [isAutostartPending, setIsAutostartPending] = useState(false);
  const [isUpdatePending, setIsUpdatePending] = useState(false);
  const [dataPath, setDataPath] = useState<string | null>(null);
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

    void getDataFilePath()
      .then((path) => {
        if (isMounted) setDataPath(path);
      })
      .catch(() => {
        if (isMounted) setMessage(ui.settings.dataPathReadFailed);
      });

    return () => {
      isMounted = false;
    };
  }, [isAvailable, isOpen, ui.settings.autostartReadFailed, ui.settings.dataPathReadFailed]);

  useEffect(() => {
    if (!message) return;

    const timer = window.setTimeout(() => setMessage(null), 3200);
    return () => window.clearTimeout(timer);
  }, [message]);

  if (!isAvailable) {
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

  async function handleCheckUpdates() {
    setIsUpdatePending(true);
    setMessage(ui.settings.checkingUpdates);
    try {
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
      <button type="button" className="add-toggle" onClick={() => setIsOpen((value) => !value)}>
        <Settings aria-hidden="true" />
        <span>{ui.settings.title}</span>
      </button>

      {isOpen ? (
        <div className="settings-panel">
          <div className="settings-row">
            <div>
              <p className="setting-title">{ui.settings.focusCount}</p>
              <p className="setting-copy">{ui.settings.focusCopy}</p>
            </div>
            <div className="segmented-control" aria-label={ui.settings.focusCount}>
              {focusLimitOptions.map((limit) => (
                <button
                  key={limit}
                  type="button"
                  className={focusLimit === limit ? "segmented-control__button segmented-control__button--active" : "segmented-control__button"}
                  onClick={() => void setFocusLimit(limit)}
                >
                  Top {limit}
                </button>
              ))}
            </div>
          </div>

          <div className="settings-row">
            <div>
              <p className="setting-title">{ui.settings.language}</p>
              <p className="setting-copy">{ui.settings.languageCopy}</p>
            </div>
            <div className="segmented-control segmented-control--wrap" aria-label={ui.settings.language}>
              {languageOptions.map((option) => (
                <button
                  key={option}
                  type="button"
                  className={language === option ? "segmented-control__button segmented-control__button--active" : "segmented-control__button"}
                  onClick={() => void setLanguage(option)}
                >
                  {languageName(option, language)}
                </button>
              ))}
            </div>
          </div>

          <div className="settings-row">
            <div>
              <p className="setting-title">{ui.settings.autostart}</p>
              <p className="setting-copy">{ui.settings.autostartCopy}</p>
            </div>
            <label className="switch" aria-label={ui.settings.autostart}>
              <input
                type="checkbox"
                checked={isAutostartEnabled}
                disabled={isAutostartPending}
                onChange={(event) => void handleAutostartToggle(event.target.checked)}
              />
              <span />
            </label>
          </div>

          <div className="settings-actions">
            <button type="button" className="text-button" onClick={handleResetPosition}>
              <RotateCcw aria-hidden="true" />
              {ui.settings.resetPosition}
            </button>
            <button type="button" className="text-button" onClick={() => void handleHideFor(15)}>
              <EyeOff aria-hidden="true" />
              {ui.settings.hide15}
            </button>
            <button type="button" className="text-button" onClick={() => void handleHideFor(30)}>
              <EyeOff aria-hidden="true" />
              {ui.settings.hide30}
            </button>
            <button type="button" className="text-button" onClick={() => void handleHideFor(60)}>
              <EyeOff aria-hidden="true" />
              {ui.settings.hide60}
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
          </div>

          {dataPath ? <p className="data-path">{dataPath}</p> : null}
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
  const tasks = useDeadlineStore((state) => state.tasks);
  const bulkAddTasks = useDeadlineStore((state) => state.bulkAddTasks);
  const importTasks = useDeadlineStore((state) => state.importTasks);
  const runCommand = useDeadlineStore((state) => state.runCommand);
  const clearCommandMessage = useDeadlineStore((state) => state.clearCommandMessage);
  const [isOpen, setIsOpen] = useState(false);
  const [rawImport, setRawImport] = useState("");
  const [previewRows, setPreviewRows] = useState<ImportPreviewRow[]>([]);
  const [localMessage, setLocalMessage] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
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
    await runCommand(rawImport.trim());
    setPreviewRows([]);
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

    await bulkAddTasks(inputs);
    setRawImport("");
    setPreviewRows([]);
    setLocalMessage(null);
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
        target?.scrollIntoView({ block: "end", behavior: "smooth" });
      });
    });
  }

  function exportTasks() {
    const payload = {
      version: 1,
      exportedAt: new Date().toISOString(),
      tasks: sortDeadlineTasks(tasks)
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `deadline-panel-${new Date().toISOString().slice(0, 10)}.json`;
    link.click();
    URL.revokeObjectURL(url);
    setLocalMessage(ui.import.exported);
  }

  async function importJsonFile(file: File) {
    try {
      const text = await file.text();
      const parsed = JSON.parse(text) as { tasks?: DeadlineTask[] } | DeadlineTask[];
      const imported = Array.isArray(parsed) ? parsed : parsed.tasks;
      if (!Array.isArray(imported)) {
        setLocalMessage(ui.import.noTasksInJson);
        return;
      }

      await importTasks(imported);
      setLocalMessage(null);
    } catch {
      setLocalMessage(ui.import.jsonImportFailed);
    } finally {
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
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
        <button type="button" className="text-button" onClick={exportTasks}>
          <Download aria-hidden="true" />
          {ui.import.exportJson}
        </button>
        <button type="button" className="text-button" onClick={() => fileInputRef.current?.click()}>
          <Upload aria-hidden="true" />
          {ui.import.importJson}
        </button>
        <input
          ref={fileInputRef}
          className="file-input"
          type="file"
          accept="application/json,.json"
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) void importJsonFile(file);
          }}
        />
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

function isInteractiveTarget(target: EventTarget): boolean {
  return target instanceof Element && Boolean(target.closest("button,input,select,textarea,a"));
}

function isTextEditingTarget(target: EventTarget): boolean {
  return target instanceof Element && Boolean(target.closest("input,select,textarea"));
}

function isScrollbarPointer(event: PointerEvent<HTMLElement>): boolean {
  let element = event.target instanceof HTMLElement ? event.target : null;
  const boundary = event.currentTarget;

  while (element && boundary.contains(element)) {
    const style = window.getComputedStyle(element);
    const isScrollableY =
      (style.overflowY === "auto" || style.overflowY === "scroll") && element.scrollHeight > element.clientHeight;
    if (isScrollableY) {
      const rect = element.getBoundingClientRect();
      const scrollbarWidth = Math.max(12, element.offsetWidth - element.clientWidth);
      if (event.clientX >= rect.right - scrollbarWidth) {
        return true;
      }
    }

    if (element === boundary) break;
    element = element.parentElement;
  }

  return false;
}
