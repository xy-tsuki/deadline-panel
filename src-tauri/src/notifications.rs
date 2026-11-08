use rusqlite::{params, Connection};

#[cfg(windows)]
mod platform {
    use super::*;
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        Foundation::DateTime,
        UI::Notifications::{
            NotificationSetting, ScheduledToastNotification, ToastNotificationManager,
            ToastNotifier,
        },
    };

    struct Apartment(bool);
    impl Apartment {
        fn initialize() -> Result<Self, String> {
            match unsafe {
                windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                )
            } {
                Ok(()) => Ok(Self(true)),
                Err(error) if error.code() == windows::Win32::Foundation::RPC_E_CHANGED_MODE => {
                    Ok(Self(false))
                }
                Err(error) => Err(error.to_string()),
            }
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    windows::Win32::System::WinRT::RoUninitialize();
                }
            }
        }
    }
    fn notification_id(id: &str) -> String {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in id.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("dp{:014x}", hash & 0x00ffffffffffffff)
    }
    fn notifier(identifier: &str) -> Result<ToastNotifier, String> {
        let notifier =
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(identifier))
                .map_err(|error| error.to_string())?;
        if notifier.Setting().map_err(|error| error.to_string())? != NotificationSetting::Enabled {
            return Err("notification_disabled_or_unregistered".into());
        }
        Ok(notifier)
    }
    fn remove(notifier: &ToastNotifier, id: &str) -> Result<(), String> {
        let expected = notification_id(id);
        let scheduled = notifier
            .GetScheduledToastNotifications()
            .map_err(|error| error.to_string())?;
        for notification in scheduled {
            if notification
                .Id()
                .map_err(|error| error.to_string())?
                .to_string()
                == expected
            {
                notifier
                    .RemoveFromSchedule(&notification)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
    fn add(db: &Connection, notifier: &ToastNotifier, id: &str) -> Result<bool, String> {
        let task = db.query_row("SELECT title, unixepoch(due_at) FROM tasks WHERE id=?1 AND status != 'completed' AND unixepoch(due_at)>unixepoch('now')", params![id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)));
        let (title, due) = match task {
            Ok(task) => task,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        let escaped = title
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;");
        let xml = XmlDocument::new().map_err(|error| error.to_string())?;
        xml.LoadXml(&HSTRING::from(format!("<toast><visual><binding template=\"ToastGeneric\"><text>Deadline Panel</text><text>{escaped}</text></binding></visual></toast>"))).map_err(|error| error.to_string())?;
        let time = due
            .checked_add(11644473600)
            .and_then(|value| value.checked_mul(10000000))
            .ok_or("invalid_notification_time")?;
        let notification = ScheduledToastNotification::CreateScheduledToastNotification(
            &xml,
            DateTime {
                UniversalTime: time,
            },
        )
        .map_err(|error| error.to_string())?;
        notification
            .SetId(&HSTRING::from(notification_id(id)))
            .map_err(|error| error.to_string())?;
        notifier
            .AddToSchedule(&notification)
            .map_err(|error| error.to_string())?;
        Ok(true)
    }
    pub fn refresh(db: &Connection, identifier: &str) -> Result<usize, String> {
        let value = super::super::get_app_setting_value(db, "scheduled_notification_tasks")
            .map_err(|error| error.to_string())?;
        let Some(value) = value else {
            return Ok(0);
        };
        let ids: Vec<String> = serde_json::from_str(&value).map_err(|error| error.to_string())?;
        if ids.is_empty() {
            return Ok(0);
        }
        let _apartment = Apartment::initialize()?;
        let notifier = notifier(identifier)?;
        let mut kept = Vec::new();
        for id in ids {
            remove(&notifier, &id)?;
            if add(db, &notifier, &id)? {
                kept.push(id);
            }
        }
        super::super::set_app_setting_value(
            db,
            "scheduled_notification_tasks",
            &serde_json::to_string(&kept).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        Ok(kept.len())
    }
    pub fn schedule(db: &Connection, identifier: &str) -> Result<usize, String> {
        let notifier = notifier(identifier)?;
        let mut stmt = db.prepare("SELECT id FROM tasks WHERE status!='completed' AND unixepoch(due_at)>unixepoch('now') ORDER BY unixepoch(due_at),id LIMIT 10").map_err(|error| error.to_string())?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let old = super::super::get_app_setting_value(db, "scheduled_notification_tasks")
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| "[]".into());
        let old: Vec<String> = serde_json::from_str(&old).map_err(|error| error.to_string())?;
        for id in old {
            remove(&notifier, &id)?;
        }
        // Keep the selected IDs before scheduling so a partially failed attempt remains cancellable.
        super::super::set_app_setting_value(
            db,
            "scheduled_notification_tasks",
            &serde_json::to_string(&ids).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        refresh(db, identifier)
    }
}
#[cfg(windows)]
pub use platform::{refresh, schedule};
#[cfg(not(windows))]
pub fn schedule(_: &Connection, _: &str) -> Result<usize, String> {
    Err("Windows scheduled notifications unavailable".into())
}
#[cfg(not(windows))]
pub fn refresh(_: &Connection, _: &str) -> Result<usize, String> {
    Ok(0)
}
