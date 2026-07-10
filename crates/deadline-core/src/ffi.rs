use crate::model::{NewTaskInput, UpdateTaskInput};
use crate::parser::{parse_quick_add, QuickAddParseResult};
use crate::repository::{DeadlineRepository, InMemoryDeadlineRepository, RepositoryError};
use crate::storage::{read_tasks_from_database, SqliteDeadlineRepository, StorageError};
use crate::DEADLINE_CORE_VERSION;
use chrono::{SecondsFormat, Utc};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::json;
use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;

static REPOSITORY: Lazy<Mutex<CoreRepository>> =
    Lazy::new(|| Mutex::new(CoreRepository::Memory(InMemoryDeadlineRepository::default())));

enum CoreRepository {
    Memory(InMemoryDeadlineRepository),
    Sqlite(SqliteDeadlineRepository),
}

impl CoreRepository {
    fn list_deadlines(&self) -> Result<Vec<crate::DeadlineTask>, StorageError> {
        match self {
            Self::Memory(repository) => Ok(repository.list_deadlines()),
            Self::Sqlite(repository) => repository.list_deadlines(),
        }
    }

    fn create_deadline(
        &mut self,
        input: NewTaskInput,
        now: &str,
    ) -> Result<crate::DeadlineTask, StorageError> {
        match self {
            Self::Memory(repository) => Ok(repository.create_deadline(input, now)),
            Self::Sqlite(repository) => repository.create_deadline(input, now),
        }
    }

    fn update_deadline(
        &mut self,
        id: &str,
        fields: UpdateTaskInput,
        now: &str,
    ) -> Result<crate::DeadlineTask, StorageError> {
        match self {
            Self::Memory(repository) => repository
                .update_deadline(id, fields, now)
                .map_err(StorageError::from),
            Self::Sqlite(repository) => repository.update_deadline(id, fields, now),
        }
    }

    fn delete_deadline(&mut self, id: &str) -> Result<bool, StorageError> {
        match self {
            Self::Memory(repository) => Ok(repository.delete_deadline(id)),
            Self::Sqlite(repository) => repository.delete_deadline(id),
        }
    }

    fn complete_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<crate::DeadlineTask, StorageError> {
        match self {
            Self::Memory(repository) => repository
                .complete_deadline(id, now)
                .map_err(StorageError::from),
            Self::Sqlite(repository) => repository.complete_deadline(id, now),
        }
    }

    fn restore_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<crate::DeadlineTask, StorageError> {
        match self {
            Self::Memory(repository) => repository
                .restore_deadline(id, now)
                .map_err(StorageError::from),
            Self::Sqlite(repository) => repository.restore_deadline(id, now),
        }
    }

    fn toggle_current_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<crate::DeadlineTask, StorageError> {
        match self {
            Self::Memory(repository) => repository
                .toggle_current_deadline(id, now)
                .map_err(StorageError::from),
            Self::Sqlite(repository) => repository.toggle_current_deadline(id, now),
        }
    }

    fn merge_deadlines(&mut self, tasks: Vec<crate::DeadlineTask>) -> Result<usize, StorageError> {
        match self {
            Self::Memory(repository) => {
                let count = tasks.len();
                let mut merged = repository.list_deadlines();
                for task in tasks {
                    if let Some(index) = merged.iter().position(|existing| existing.id == task.id) {
                        merged[index] = task;
                    } else {
                        merged.push(task);
                    }
                }
                repository.replace_deadlines(merged);
                Ok(count)
            }
            Self::Sqlite(repository) => repository.merge_deadlines(tasks),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDeadlineRequest {
    id: String,
    fields: UpdateTaskInput,
}

#[no_mangle]
pub extern "C" fn deadline_core_version_json() -> *mut c_char {
    ok_json(json!({ "version": DEADLINE_CORE_VERSION }))
}

#[no_mangle]
pub unsafe extern "C" fn deadline_initialize_json(database_path: *const c_char) -> *mut c_char {
    let Ok(database_path) = c_string_to_string(database_path) else {
        return error_json("invalid-input");
    };
    match SqliteDeadlineRepository::open(database_path) {
        Ok(repository) => {
            let Ok(mut current) = REPOSITORY.lock() else {
                return error_json("repository-lock-failed");
            };
            *current = CoreRepository::Sqlite(repository);
            ok_json(json!({ "storage": "sqlite" }))
        }
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_migrate_legacy_database_json(
    legacy_database_path: *const c_char,
) -> *mut c_char {
    let Ok(legacy_database_path) = c_string_to_string(legacy_database_path) else {
        return error_json("invalid-input");
    };
    let tasks = match read_tasks_from_database(&legacy_database_path) {
        Ok(tasks) => tasks,
        Err(error) => return storage_error_json(error),
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.merge_deadlines(tasks) {
        Ok(count) => ok_json(json!({ "imported": count })),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_import_tasks_json(input: *const c_char) -> *mut c_char {
    let Ok(tasks) = parse_input::<Vec<crate::DeadlineTask>>(input) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.merge_deadlines(tasks) {
        Ok(count) => ok_json(json!({ "imported": count })),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub extern "C" fn deadline_list_json() -> *mut c_char {
    let Ok(repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.list_deadlines() {
        Ok(tasks) => ok_json(tasks),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_create_json(input: *const c_char) -> *mut c_char {
    let Ok(input) = parse_input::<NewTaskInput>(input) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.create_deadline(input, &now_iso()) {
        Ok(task) => ok_json(task),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_update_json(input: *const c_char) -> *mut c_char {
    let Ok(request) = parse_input::<UpdateDeadlineRequest>(input) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.update_deadline(&request.id, request.fields, &now_iso()) {
        Ok(task) => ok_json(task),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_delete(id: *const c_char) -> bool {
    let Ok(id) = c_string_to_string(id) else {
        return false;
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return false;
    };
    repository.delete_deadline(&id).unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "C" fn deadline_complete_json(id: *const c_char) -> *mut c_char {
    let Ok(id) = c_string_to_string(id) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.complete_deadline(&id, &now_iso()) {
        Ok(task) => ok_json(task),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_restore_json(id: *const c_char) -> *mut c_char {
    let Ok(id) = c_string_to_string(id) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.restore_deadline(&id, &now_iso()) {
        Ok(task) => ok_json(task),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_toggle_current_json(id: *const c_char) -> *mut c_char {
    let Ok(id) = c_string_to_string(id) else {
        return error_json("invalid-input");
    };
    let Ok(mut repository) = REPOSITORY.lock() else {
        return error_json("repository-lock-failed");
    };
    match repository.toggle_current_deadline(&id, &now_iso()) {
        Ok(task) => ok_json(task),
        Err(error) => storage_error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_parse_quick_add_json(input: *const c_char) -> *mut c_char {
    let Ok(text) = c_string_to_string(input) else {
        return error_json("invalid-input");
    };
    match parse_quick_add(&text) {
        QuickAddParseResult::Ok(input) => ok_json(input),
        QuickAddParseResult::Error(error) => error_json(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn deadline_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

fn parse_input<T: for<'de> Deserialize<'de>>(ptr: *const c_char) -> Result<T, ()> {
    let text = c_string_to_string(ptr)?;
    serde_json::from_str(&text).map_err(|_| ())
}

fn c_string_to_string(ptr: *const c_char) -> Result<String, ()> {
    if ptr.is_null() {
        return Err(());
    }
    let value = unsafe { CStr::from_ptr(ptr) };
    value.to_str().map(|value| value.to_string()).map_err(|_| ())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn repository_error_json(error: RepositoryError) -> *mut c_char {
    match error {
        RepositoryError::NotFound => error_json("not-found"),
        RepositoryError::CurrentTaskLimitReached => error_json("current-task-limit-reached"),
    }
}

fn storage_error_json(error: StorageError) -> *mut c_char {
    match error {
        StorageError::Repository(error) => repository_error_json(error),
        StorageError::Sqlite(error) => error_json(&format!("sqlite:{error}")),
        StorageError::InvalidField(field) => error_json(&format!("invalid-field:{field}")),
    }
}

fn ok_json(value: impl serde::Serialize) -> *mut c_char {
    string_to_c_ptr(json!({ "ok": true, "data": value }).to_string())
}

fn error_json(error: &str) -> *mut c_char {
    string_to_c_ptr(json!({ "ok": false, "error": error }).to_string())
}

fn string_to_c_ptr(value: String) -> *mut c_char {
    CString::new(value)
        .expect("JSON output should not contain interior NUL bytes")
        .into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_response_is_json_envelope() {
        let ptr = deadline_core_version_json();
        let text = unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() };
        unsafe { deadline_free_string(ptr) };

        let value: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["version"], DEADLINE_CORE_VERSION);
    }
}
