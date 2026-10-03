import { settingsWindowAction } from "./runtime/tauri";
import { getStrings, resolveLanguage } from "./i18n";
import { useDeadlineStore } from "./store/deadlineStore";

export function SettingsChrome() {
  const language = useDeadlineStore(state => state.language);
  const names = ({ zh: ["关闭设置", "最小化设置"], en: ["Close settings", "Minimize settings"], ja: ["設定を閉じる", "設定を最小化"] })[resolveLanguage(language)];
  return <header className="settings-chrome" onPointerDown={event => {
    if (event.button === 0 && !(event.target as HTMLElement).closest("button")) void settingsWindowAction("drag");
  }}>
    <div className="settings-chrome__controls">
      <button type="button" className="settings-chrome__close" aria-label={names[0]} onClick={() => void settingsWindowAction("hide")}><span>×</span></button>
      <button type="button" className="settings-chrome__minimize" aria-label={names[1]} onClick={() => void settingsWindowAction("minimize")}><span>−</span></button>
      <span className="settings-chrome__inactive" aria-hidden="true"/>
    </div>
    <span className="settings-chrome__title">Deadline Panel · {getStrings(language).settings.title}</span>
  </header>;
}
