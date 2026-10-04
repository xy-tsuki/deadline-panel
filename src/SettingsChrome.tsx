import { settingsWindowAction } from "./runtime/tauri";
import { getStrings, resolveLanguage } from "./i18n";
import { useDeadlineStore } from "./store/deadlineStore";
import { Minus, Square, X } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { fitSettingsWindow } from "./runtime/tauri";

export function SettingsContent({ children }: { children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const content = ref.current;
    if (!content) return;
    const observer = new ResizeObserver(() => {
      const surface = content.parentElement!;
      const style = getComputedStyle(surface);
      void fitSettingsWindow(Math.ceil(content.getBoundingClientRect().height + parseFloat(style.paddingTop) + parseFloat(style.paddingBottom)));
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, []);
  return <div ref={ref} className="settings-content">{children}</div>;
}

export function SettingsChrome() {
  const language = useDeadlineStore(state => state.language);
  const names = ({ zh: ["关闭设置", "最小化设置", "最大化或还原设置"], en: ["Close settings", "Minimize settings", "Maximize or restore settings"], ja: ["設定を閉じる", "設定を最小化", "設定を最大化または元に戻す"] })[resolveLanguage(language)];
  return <header className="settings-chrome" onPointerDown={event => {
    if (event.button === 0 && !(event.target as HTMLElement).closest("button")) void settingsWindowAction("drag");
  }} onDoubleClick={event => {
    if (!(event.target as HTMLElement).closest("button")) void settingsWindowAction("toggle-maximize");
  }}>
    <span className="settings-chrome__title">Deadline Panel · {getStrings(language).settings.title}</span>
    <div className="settings-chrome__controls">
      <button type="button" aria-label={names[1]} onClick={() => void settingsWindowAction("minimize")}><Minus size={14}/></button>
      <button type="button" aria-label={names[2]} onClick={() => void settingsWindowAction("toggle-maximize")}><Square size={12}/></button>
      <button type="button" className="settings-chrome__close" aria-label={names[0]} onClick={() => void settingsWindowAction("hide")}><X size={15}/></button>
    </div>
  </header>;
}
