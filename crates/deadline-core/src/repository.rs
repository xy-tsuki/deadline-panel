use crate::model::{
    DeadlineSource, DeadlineStatus, DeadlineTask, NewTaskInput, UpdateTaskInput,
};
use crate::sorting::sort_deadline_tasks;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    NotFound,
    CurrentTaskLimitReached,
}

pub trait DeadlineRepository {
    fn list_deadlines(&self) -> Vec<DeadlineTask>;
    fn replace_deadlines(&mut self, tasks: Vec<DeadlineTask>);
    fn create_deadline(&mut self, input: NewTaskInput, now: &str) -> DeadlineTask;
    fn update_deadline(
        &mut self,
        id: &str,
        fields: UpdateTaskInput,
        now: &str,
    ) -> Result<DeadlineTask, RepositoryError>;
    fn delete_deadline(&mut self, id: &str) -> bool;
    fn complete_deadline(&mut self, id: &str, now: &str) -> Result<DeadlineTask, RepositoryError>;
    fn restore_deadline(&mut self, id: &str, now: &str) -> Result<DeadlineTask, RepositoryError>;
    fn toggle_current_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<DeadlineTask, RepositoryError>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryDeadlineRepository {
    tasks: Vec<DeadlineTask>,
}

impl InMemoryDeadlineRepository {
    pub fn new(tasks: Vec<DeadlineTask>) -> Self {
        Self { tasks }
    }

    pub fn create_deadline_with_id(
        &mut self,
        input: NewTaskInput,
        id: impl Into<String>,
        now: &str,
    ) -> DeadlineTask {
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
        self.tasks.push(task.clone());
        task
    }
}

impl DeadlineRepository for InMemoryDeadlineRepository {
    fn list_deadlines(&self) -> Vec<DeadlineTask> {
        sort_deadline_tasks(&self.tasks)
    }

    fn replace_deadlines(&mut self, tasks: Vec<DeadlineTask>) {
        self.tasks = tasks;
    }

    fn create_deadline(&mut self, input: NewTaskInput, now: &str) -> DeadlineTask {
        self.create_deadline_with_id(input, Uuid::new_v4().to_string(), now)
    }

    fn update_deadline(
        &mut self,
        id: &str,
        fields: UpdateTaskInput,
        now: &str,
    ) -> Result<DeadlineTask, RepositoryError> {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return Err(RepositoryError::NotFound);
        };

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

        Ok(task.clone())
    }

    fn delete_deadline(&mut self, id: &str) -> bool {
        let before = self.tasks.len();
        self.tasks.retain(|task| task.id != id);
        before != self.tasks.len()
    }

    fn complete_deadline(&mut self, id: &str, now: &str) -> Result<DeadlineTask, RepositoryError> {
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

    fn restore_deadline(&mut self, id: &str, now: &str) -> Result<DeadlineTask, RepositoryError> {
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

    fn toggle_current_deadline(
        &mut self,
        id: &str,
        now: &str,
    ) -> Result<DeadlineTask, RepositoryError> {
        let Some(current) = self.tasks.iter().find(|task| task.id == id) else {
            return Err(RepositoryError::NotFound);
        };
        if current.status == DeadlineStatus::Completed {
            return Ok(current.clone());
        }
        if !current.is_current {
            let active_current_count = self
                .tasks
                .iter()
                .filter(|task| task.status != DeadlineStatus::Completed && task.is_current)
                .count();
            if active_current_count >= 2 {
                return Err(RepositoryError::CurrentTaskLimitReached);
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskPriority;

    #[test]
    fn create_deadline_matches_store_defaults() {
        let mut repo = InMemoryDeadlineRepository::default();
        let task = repo.create_deadline_with_id(
            NewTaskInput {
                title: "  Test  ".into(),
                due_at: "2026-07-10T23:59:00Z".into(),
                priority: TaskPriority::High,
                notes: Some("  note  ".into()),
                source: None,
            },
            "fixed-id",
            "2026-07-09T00:00:00Z",
        );

        assert_eq!(task.id, "fixed-id");
        assert_eq!(task.title, "Test");
        assert_eq!(task.notes, "note");
        assert_eq!(task.source, DeadlineSource::Manual);
        assert_eq!(task.status, DeadlineStatus::Active);
        assert!(!task.is_current);
        assert_eq!(task.completed_at, None);
    }
}
