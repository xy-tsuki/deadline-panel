import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "../runtime/tauri";

const REQUEST = "deadline-sync-request";
const RESULT = "deadline-sync-result";
interface SyncRequest { id: string; sender: string; silent: boolean }
interface SyncResult { id: string; error?: string }

export function isSyncOwner(): boolean {
  return !isTauriRuntime() || getCurrentWindow().label === "main";
}

export async function requestOwnerSync(silent = false): Promise<void> {
  const id = crypto.randomUUID();
  const sender = getCurrentWindow().label;
  await new Promise<void>((resolve, reject) => {
    let stop: (() => void) | undefined;
    let expired = false;
    const timer = window.setTimeout(() => { expired = true; stop?.(); reject(new Error("Cloud sync owner timed out")); }, 60_000);
    void listen<SyncResult>(RESULT, ({ payload }) => {
      if (payload.id !== id) return;
      window.clearTimeout(timer); stop?.();
      if (payload.error) reject(new Error(payload.error)); else resolve();
    }).then(async (unlisten) => {
      if (expired) { unlisten(); return; }
      stop = unlisten;
      await emitTo("main", REQUEST, { id, sender, silent } satisfies SyncRequest);
    }).catch((error) => { window.clearTimeout(timer); stop?.(); reject(error); });
  });
}

export function listenForSyncRequests(run: (silent: boolean) => Promise<void>): Promise<() => void> {
  return listen<SyncRequest>(REQUEST, ({ payload }) => {
    void run(payload.silent).then(
      () => emitTo(payload.sender, RESULT, { id: payload.id }),
      (error: unknown) => emitTo(payload.sender, RESULT, { id: payload.id, error: String(error) })
    ).catch(console.error);
  });
}
