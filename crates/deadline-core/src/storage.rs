use crate::model::{
    DeadlineSource, DeadlineStatus, DeadlineTask, NewTaskInput, TaskPriority, UpdateTaskInput,
};
use crate::repository::RepositoryError;
use crate::sorting::sort_deadline_tasks;
use rusqlite::{params, Connection, Row};
use std::path::Path;
use uuid::Uuid;

pub const LEGACY_TAURI_DATABASE_FILE: &str = "deadline-panel.sqlite3";

pub const SQLITE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  due_at TEXT NOT NULL,
  priority TEXT NOT NULL CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
  status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'postponed')),
  notes TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT 'manual',
  is_current INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status_due_at
ON tasks (status, due_at);

CREATE INDEX IF NOT EXISTS idx_tasks_due_priority
ON tasks (due_at, priority);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#;

#[derive(Debug)]
pub enum StorageError {
    Sqlite(String),
    InvalidField(String),
    Repository(RepositoryError),
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error.to_string())
    }
}

impl From<RepositoryError> for StorageError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

pub struct SqliteDeadlineRepository {
    db: Connection,
}

impl SqliteDeadlineRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db = Connection::open(path)?;
        configure_database(&db)?;
        initialize_database(&db)?;
        Ok(Self { db })
    }

    pub fn list_deadlines(&self) -> Result<Vec<DeadlineTask>, StorageError> {
        let mut statement = self.db.prepare(
            "SELECT id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
             FROM tasks",
        )?;
        let rows = statement.query_map([], task_from_row)?;
        let tasks = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(sort_deadline_tasks(&tasks))
    }

    pub fn replace_deadlines(&mut self, tasks: Vec<DeadlineTask>) -> Result<(), StorageError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM tasks", [])?;
        for task in tasks {
            insert_task(&tx, &task)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn merge_deadlines(&mut self, tasks: Vec<DeadlineTask>) -> Result<usize, StorageError> {
        let tx = self.db.transaction()?;
        let count = tasks.len();
        for task in tasks {
            insert_task(&tx, &task)?;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn create_deadline(
        &mut self,
        input: NewTaskInput,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        self.create_deadline_with_id(input, Uuid::new_v4().to_string(), now)
    }

    pub fn create_deadline_with_id(
        &mut self,
        input: NewTaskInput,
        id: impl Into<String>,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        let task = DeadlineTask {
            id: id.into(),
            title: input.title.trim().to_string(),
            due_at: input.due_at,
            priority: input.priority,
            status: DeadlineStatus::Active,
            notes: input.notes.unwrap_or_default().trim().to_string(),
            source: input.source.unwrap_or(DeadlineSource::Manual),
            is_current: false,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            completed_at: None,
        };
        insert_task(&self.db, &task)?;
        Ok(task)
    }

    pub fn update_deadline(
        &mut self,
        id: &str,
        fields: UpdateTaskInput,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        let mut task = self
            .get_deadline(id)?
            .ok_or(RepositoryError::NotFound)?;

        if let Some(title) = fields.title {
            task.title = title.trim().to_string();
        }
        if let Some(due_at) = fields.due_at {
            task.due_at = due_at;
        }
        if let Some(priority) = fields.priority {
            task.priority = priority;
        }
        if let Some(notes) = fields.notes {
            task.notes = notes;
        }
        if let Some(status) = fields.status {
            task.status = status;
        }
        if let Some(source) = fields.source {
            task.source = source;
        }
        if let Some(is_current) = fields.is_current {
            task.is_current = is_current;
        }
        if let Some(completed_at) = fields.completed_at {
            task.completed_at = completed_at;
        }
        task.updated_at = now.to_string();
        insert_task(&self.db, &task)?;
        Ok(task)
    }

    pub fn delete_deadline(&mut self, id: &str) -> Result<bool, StorageError> {
        let changed = self
            .db
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    pub fn complete_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        self.update_deadline(
            id,
            UpdateTaskInput {
                status: Some(DeadlineStatus::Completed),
                is_current: Some(false),
                completed_at: Some(Some(now.to_string())),
                ..Default::default()
            },
            now,
        )
    }

    pub fn restore_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        self.update_deadline(
            id,
            UpdateTaskInput {
                status: Some(DeadlineStatus::Active),
                completed_at: Some(None),
                ..Default::default()
            },
            now,
        )
    }

    pub fn toggle_current_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<DeadlineTask, StorageError> {
        let Some(current) = self.get_deadline(id)? else {
            return Err(RepositoryError::NotFound.into());
        };
        if current.status == DeadlineStatus::Completed {
            return Ok(current);
        }
        if !current.is_current && self.active_current_count()? >= 2 {
            return Err(RepositoryError::CurrentTaskLimitReached.into());
        }
        self.update_deadline(
            id,
            UpdateTaskInput {
                is_current: Some(!current.is_current),
                ..Default::default()
            },
            now,
        )
    }

    fn get_deadline(&self, id: &str) -> Result<Option<DeadlineTask>, StorageError> {
        let mut statement = self.db.prepare(
            "SELECT id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
             FROM tasks
             WHERE id = ?1",
        )?;
        let mut rows = statement.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(task_from_row(row)?))
    }

    fn active_current_count(&self) -> Result<usize, StorageError> {
        let count: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status != 'completed' AND is_current = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

pub fn read_tasks_from_database(path: impl AsRef<Path>) -> Result<Vec<DeadlineTask>, StorageError> {
    let db = Connection::open(path)?;
    let mut statement = db.prepare(
        "SELECT id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
         FROM tasks",
    )?;
    let rows = statement.query_map([], task_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
}

fn configure_database(db: &Connection) -> Result<(), StorageError> {
    db.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;
        ",
    )?;
    Ok(())
}

fn initialize_database(db: &Connection) -> Result<(), StorageError> {
    db.execute_batch(SQLITE_SCHEMA)?;
    ensure_task_columns(db)
}

fn ensure_task_columns(db: &Connection) -> Result<(), StorageError> {
    let mut statement = db.prepare("PRAGMA table_info(tasks)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut has_is_current = false;

    for row in rows {
        if row? == "is_current" {
            has_is_current = true;
            break;
        }
    }

    if !has_is_current {
        db.execute(
            "ALTER TABLE tasks ADD COLUMN is_current INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    Ok(())
}

fn insert_task(db: &Connection, task: &DeadlineTask) -> Result<(), StorageError> {
    db.execute(
        "INSERT INTO tasks (
            id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            due_at = excluded.due_at,
            priority = excluded.priority,
            status = excluded.status,
            notes = excluded.notes,
            source = excluded.source,
            is_current = excluded.is_current,
            updated_at = excluded.updated_at,
            completed_at = excluded.completed_at",
        params![
            &task.id,
            &task.title,
            &task.due_at,
            task.priority.as_str(),
            task.status.as_str(),
            &task.notes,
            task.source.as_str(),
            if task.is_current { 1 } else { 0 },
            &task.created_at,
            &task.updated_at,
            &task.completed_at,
        ],
    )?;
    Ok(())
}

fn task_from_row(row: &Row<'_>) -> Result<DeadlineTask, rusqlite::Error> {
    let priority_text: String = row.get(3)?;
    let status_text: String = row.get(4)?;
    let source_text: String = row.get(6)?;

    let priority = TaskPriority::parse(&priority_text)
        .ok_or_else(|| rusqlite::Error::InvalidParameterName("priority".into()))?;
    let status = DeadlineStatus::parse(&status_text)
        .ok_or_else(|| rusqlite::Error::InvalidParameterName("status".into()))?;
    let source = DeadlineSource::parse(&source_text)
        .ok_or_else(|| rusqlite::Error::InvalidParameterName("source".into()))?;

    Ok(DeadlineTask {
        id: row.get(0)?,
        title: row.get(1)?,
        due_at: row.get(2)?,
        priority,
        status,
        notes: row.get(5)?,
        source,
        is_current: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_repository_persists_deadlines() {
        let path = std::env::temp_dir().join(format!(
            "deadline-core-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));

        {
            let mut repository = SqliteDeadlineRepository::open(&path).expect("open sqlite");
            let task = repository
                .create_deadline_with_id(
                    NewTaskInput {
                        title: " Persist me ".into(),
                        due_at: "2026-07-10T23:59:00Z".into(),
                        priority: TaskPriority::High,
                        notes: Some("note".into()),
                        source: None,
                    },
                    "persisted",
                    "2026-07-09T00:00:00Z",
                )
                .expect("create");
            assert_eq!(task.title, "Persist me");
        }

        {
            let repository = SqliteDeadlineRepository::open(&path).expect("reopen sqlite");
            let tasks = repository.list_deadlines().expect("list");
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].id, "persisted");
            assert_eq!(tasks[0].priority, TaskPriority::High);
        }

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn reads_legacy_database_tasks() {
        let path = std::env::temp_dir().join(format!(
            "deadline-core-legacy-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));

        let mut repository = SqliteDeadlineRepository::open(&path).expect("open sqlite");
        repository
            .create_deadline_with_id(
                NewTaskInput {
                    title: "Legacy".into(),
                    due_at: "2026-07-10T23:59:00Z".into(),
                    priority: TaskPriority::Medium,
                    notes: None,
                    source: Some(DeadlineSource::Manual),
                },
                "legacy-id",
                "2026-07-09T00:00:00Z",
            )
            .expect("create");
        drop(repository);

        let tasks = read_tasks_from_database(&path).expect("read legacy");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "legacy-id");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }
}
