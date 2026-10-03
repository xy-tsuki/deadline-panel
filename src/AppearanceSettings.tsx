import { useDeadlineStore } from "./store/deadlineStore";
import { resolveLanguage } from "./i18n";
import { AppTheme } from "./domain/appearance";
import { LiquidTextSegments } from "./LiquidTextSegments";
const copy = {
  zh: { theme: "主题", system: "跟随系统", light: "浅色", dark: "深色", lowPower: "低功耗模式", lowPowerCopy: "关闭界面动画，保留同步与提醒", fullscreen: "全屏时自动隐藏", fullscreenCopy: "关闭后停止全屏检测，保留手动隐藏状态" },
  ja: { theme: "テーマ", system: "システム", light: "ライト", dark: "ダーク", lowPower: "低電力モード", lowPowerCopy: "アニメーションを停止し、同期と通知は継続", fullscreen: "全画面で自動的に隠す", fullscreenCopy: "オフにすると全画面の監視を停止" },
  en: { theme: "Theme", system: "System", light: "Light", dark: "Dark", lowPower: "Low power mode", lowPowerCopy: "Disable interface animations; keep sync and reminders", fullscreen: "Hide automatically in fullscreen", fullscreenCopy: "Stop fullscreen detection when disabled; keep manual hiding" }
};
export function AppearanceSettings({ part = "all" }: { part?: "theme" | "behavior" | "all" }) {
  const appearance = useDeadlineStore((state) => state.appearance);
  const setAppearance = useDeadlineStore((state) => state.setAppearance);
  const language = useDeadlineStore((state) => state.language);
  const text = copy[resolveLanguage(language)];
  return <>
    {part !== "behavior" ? <div className="settings-row"><div><p className="setting-title">{text.theme}</p><p className="setting-copy">{({ zh: "控制面板、设置窗口和系统控件的浅色 / 深色外观。", ja: "パネル、設定ウィンドウとシステムコントロールの外観を設定します。", en: "Set the light or dark appearance of the panel, settings window and system controls." })[resolveLanguage(language)]}</p></div>
      <LiquidTextSegments value={appearance.theme} label={text.theme} options={(["system", "light", "dark"] as AppTheme[]).map(value => ({ value, label: text[value] }))} onChange={theme => setAppearance({ theme })}/>
    </div> : null}
    {part !== "theme" ? (["fullscreenAutoHide", "lowPowerMode"] as const).map((key) => <div className="settings-row settings-row--toggle" key={key}>
      <label className="setting-check"><input type="checkbox" checked={appearance[key]} onChange={(event) => void setAppearance({ [key]: event.target.checked })}/><span>{key === "lowPowerMode" ? text.lowPower : text.fullscreen}</span></label>
      <p className="setting-copy">{key === "lowPowerMode" ? text.lowPowerCopy : text.fullscreenCopy}</p>
    </div>) : null}
  </>;
}

