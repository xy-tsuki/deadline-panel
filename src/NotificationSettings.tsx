import { useState } from "react";
import { resolveLanguage } from "./i18n";
import { isTauriRuntime, scheduleNotifications } from "./runtime/tauri";
import { useDeadlineStore } from "./store/deadlineStore";
const copy = {
  zh: { title: "任务通知", description: "手动安排最近 10 条未完成且未来截止的任务；已安排任务变更后更新或取消计划。", button: "安排通知", pending: "正在安排…", done: "已安排 {count} 条通知", failed: "安排失败，请检查 Windows 通知开关和安装状态：", unavailable: "需在已安装的 Windows 应用中验证通知。", note: "关机超过系统投递窗口时，提醒可能被系统丢弃。" },
  ja: { title: "タスク通知", description: "未完了の将来のタスクを最大10件、手動で予約。変更すると既存の予約を更新または取り消します。", button: "通知を予約", pending: "予約中…", done: "{count}件の通知を予約しました", failed: "予約失敗。Windowsの通知設定とインストール状態を確認：", unavailable: "インストール済みWindowsアプリで検証してください。", note: "配信時間帯を過ぎて電源がオフの場合、通知は破棄されることがあります。" },
  en: { title: "Task notifications", description: "Manually schedule up to 10 upcoming unfinished tasks; changes update or cancel existing schedules.", button: "Schedule notifications", pending: "Scheduling…", done: "Scheduled {count} notifications", failed: "Scheduling failed. Check Windows notification settings and installation: ", unavailable: "Verify in the installed Windows application.", note: "Windows may drop reminders if the PC is off beyond the delivery window." }
};
export function NotificationSettings() {
  const language = useDeadlineStore((state) => state.language);
  const text = copy[resolveLanguage(language)];
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  async function schedule() {
    setPending(true); setMessage(null);
    try { const count = await scheduleNotifications(); useDeadlineStore.setState({ notificationError: null }); setMessage(text.done.replace("{count}", String(count))); }
    catch (error) { setMessage(text.failed + String(error)); }
    finally { setPending(false); }
  }
  return <div className="notification-settings">
    <p className="setting-title">{text.title}</p><p className="setting-copy">{text.description}</p>
    <button type="button" className="text-button" disabled={pending || !isTauriRuntime()} onClick={() => void schedule()}>{pending ? text.pending : text.button}</button>
    <p className="setting-copy">{isTauriRuntime() ? text.note : text.unavailable}</p>
    {message ? <p className="setting-message" role="status">{message}</p> : null}
  </div>;
}
