let pending = 0;
export function hasPendingPanelInteraction() { return pending > 0; }
export function beginPanelInteraction() {
  pending++;
  let ended = false;
  return () => { if (!ended) { ended = true; pending--; } };
}
