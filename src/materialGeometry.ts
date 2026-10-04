import { isTauriRuntime, setMaterialGeometry } from "./runtime/tauri";

export async function syncMaterialGeometry(): Promise<void> {
  const rects = [...document.querySelectorAll<HTMLElement>(".collapsed-strip,.expanded-panel,.settings-window")].flatMap(element => {
    const bounds = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    if (style.visibility === "hidden" || style.display === "none" || bounds.width <= 0 || bounds.height <= 0) return [];
    return [[bounds.x, bounds.y, bounds.width, bounds.height, parseFloat(style.borderTopLeftRadius) || 0]];
  });
  await setMaterialGeometry(rects);
}

export function observeMaterialGeometry(): () => void {
  if (!isTauriRuntime()) return () => {};
  let frame = 0;
  const refresh = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => { void syncMaterialGeometry().catch(console.error); });
  };
  const resize = new ResizeObserver(refresh);
  const observe = () => {
    resize.disconnect();
    document.querySelectorAll(".collapsed-strip,.expanded-panel,.settings-window").forEach(element => resize.observe(element));
    refresh();
  };
  const mutations = new MutationObserver(observe);
  mutations.observe(document.getElementById("root")!, { childList: true, subtree: true });
  observe();
  window.addEventListener("resize", refresh);
  return () => { cancelAnimationFrame(frame); resize.disconnect(); mutations.disconnect(); window.removeEventListener("resize", refresh); };
}
