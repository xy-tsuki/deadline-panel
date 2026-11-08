// Bound both response headers and body. Dropped networks must release the sync
// owner so a later request can retry; caller cancellation is preserved.
export async function fetchWithTimeout(input: RequestInfo | URL, init?: RequestInit, timeoutMs = 15_000): Promise<Response> {
  const controller = new AbortController();
  const signal = init?.signal ?? (typeof Request !== "undefined" && input instanceof Request ? input.signal : undefined);
  const cancel = () => controller.abort(signal?.reason);
  if (signal?.aborted) cancel(); else signal?.addEventListener("abort", cancel, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException("Cloud request timed out", "TimeoutError")), timeoutMs);
  try {
    const response = await fetch(input, { ...init, signal: controller.signal });
    const body = await response.arrayBuffer();
    return new Response([204, 205, 304].includes(response.status) ? null : body, { status: response.status, statusText: response.statusText, headers: response.headers });
  } finally { clearTimeout(timer); signal?.removeEventListener("abort", cancel); }
}
