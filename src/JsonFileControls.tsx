import { useRef, useState } from "react";
import { Download, Upload } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getStrings } from "./i18n";
import { isTauriRuntime } from "./runtime/tauri";
import { useDeadlineStore } from "./store/deadlineStore";
import { DeadlineTask } from "./domain/task";
import { beginPanelInteraction } from "./panelInteraction";

export function JsonFileControls() {
  const language = useDeadlineStore(state => state.language);
  const tasks = useDeadlineStore(state => state.tasks);
  const importTasks = useDeadlineStore(state => state.importTasks);
  const ui = getStrings(language);
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  async function read(text: string) {
    const parsed = JSON.parse(text) as DeadlineTask[] | { tasks?: DeadlineTask[] };
    const values = Array.isArray(parsed) ? parsed : parsed?.tasks;
    if (!Array.isArray(values)) throw new Error(ui.import.noTasksInJson);
    if (!await importTasks(values)) throw new Error(useDeadlineStore.getState().error || ui.import.jsonImportFailed);
    setMessage(null);
  }
  async function importFile() {
    if (!isTauriRuntime()) { input.current?.click(); return; }
    setPending(true); setMessage(null);
    const end = beginPanelInteraction();
    try { const text = await invoke<string | null>("read_import_json"); if (text !== null) await read(text); }
    catch (error) { setMessage(String(error)); }
    finally { end(); setPending(false); }
  }
  async function exportFile() {
    setPending(true); setMessage(null);
    const end = beginPanelInteraction();
    try {
      if (isTauriRuntime()) {
        const path = await invoke<string | null>("export_tasks_json");
        if (path !== null) setMessage(`${ui.import.exported} ${path}`);
      } else {
        const blob = new Blob([JSON.stringify({ version: 1, exportedAt: new Date().toISOString(), tasks }, null, 2)], { type: "application/json" });
        const url = URL.createObjectURL(blob); const link = document.createElement("a"); link.href = url; link.download = "deadline-panel.json"; link.click();
        window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    } catch (error) { setMessage(String(error)); }
    finally { end(); setPending(false); }
  }
  return <div className="json-file-controls">
    <button type="button" className="text-button" disabled={pending} onClick={() => void importFile()}><Upload aria-hidden="true"/>{ui.import.importJson}</button>
    <button type="button" className="text-button" disabled={pending} onClick={() => void exportFile()}><Download aria-hidden="true"/>{ui.import.exportJson}</button>
    <input ref={input} hidden type="file" accept="application/json,.json" onChange={event => {
      const file = event.target.files?.[0]; if (!file) return;
      setPending(true); setMessage(null);
      void file.text().then(read).catch(error => setMessage(String(error))).finally(() => { setPending(false); if (input.current) input.current.value = ""; });
    }}/>
    {message ? <p className="setting-message" role="status">{message}</p> : null}
  </div>;
}
