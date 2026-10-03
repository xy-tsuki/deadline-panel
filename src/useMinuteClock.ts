import { useSyncExternalStore } from "react";

let now = Date.now();
let timer: number | undefined;
const listeners = new Set<() => void>();
function publish() { now = Date.now(); for (const listener of listeners) listener(); }
function schedule() {
  window.clearTimeout(timer);
  if (document.hidden || !listeners.size) return;
  timer = window.setTimeout(() => { publish(); schedule(); }, 60_000 - Date.now() % 60_000);
}
function wake() { publish(); schedule(); }
function subscribe(listener: () => void) {
  listeners.add(listener);
  if (listeners.size === 1) {
    now = Date.now(); schedule();
    window.addEventListener("focus", wake); document.addEventListener("visibilitychange", wake);
  }
  return () => {
    listeners.delete(listener);
    if (!listeners.size) { window.clearTimeout(timer); window.removeEventListener("focus", wake); document.removeEventListener("visibilitychange", wake); }
  };
}
export function useMinuteClock() { return useSyncExternalStore(subscribe, () => now); }
