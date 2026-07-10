use crate::model::{DeadlineStatus, DeadlineTask};

pub fn sort_deadline_tasks(tasks: &[DeadlineTask]) -> Vec<DeadlineTask> {
    let mut sorted = tasks.to_vec();
    sorted.sort_by(|a, b| {
        a.due_at
            .cmp(&b.due_at)
            .then_with(|| b.priority.weight().cmp(&a.priority.weight()))
            .then_with(|| status_rank(a.status).cmp(&status_rank(b.status)))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    sorted
}

pub fn get_actionable_tasks(tasks: &[DeadlineTask]) -> Vec<DeadlineTask> {
    let actionable: Vec<DeadlineTask> = tasks
        .iter()
        .filter(|task| task.status != DeadlineStatus::Completed)
        .cloned()
        .collect();
    sort_deadline_tasks(&actionable)
}

pub fn get_today_focus(tasks: &[DeadlineTask], limit: usize) -> Vec<DeadlineTask> {
    get_actionable_tasks(tasks).into_iter().take(limit).collect()
}

pub fn get_current_tasks(tasks: &[DeadlineTask]) -> Vec<DeadlineTask> {
    let current: Vec<DeadlineTask> = tasks
        .iter()
        .filter(|task| task.status != DeadlineStatus::Completed && task.is_current)
        .cloned()
        .collect();
    sort_deadline_tasks(&current).into_iter().take(2).collect()
}

pub fn get_nearest_deadline(tasks: &[DeadlineTask]) -> Option<DeadlineTask> {
    get_actionable_tasks(tasks).into_iter().next()
}

fn status_rank(status: DeadlineStatus) -> u8 {
    match status {
        DeadlineStatus::Active => 0,
        DeadlineStatus::Postponed => 1,
        DeadlineStatus::Completed => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeadlineSource, TaskPriority};

    fn task(
        id: &str,
        due_at: &str,
        priority: TaskPriority,
        status: DeadlineStatus,
        created_at: &str,
    ) -> DeadlineTask {
        DeadlineTask {
            id: id.into(),
            title: id.into(),
            due_at: due_at.into(),
            priority,
            status,
            notes: String::new(),
            source: DeadlineSource::Manual,
            is_current: false,
            created_at: created_at.into(),
            updated_at: created_at.into(),
            completed_at: None,
        }
    }

    #[test]
    fn sorts_like_current_typescript_logic() {
        let tasks = vec![
            task(
                "later",
                "2026-07-11T23:59:00Z",
                TaskPriority::Urgent,
                DeadlineStatus::Active,
                "2026-07-01T00:00:00Z",
            ),
            task(
                "low",
                "2026-07-10T23:59:00Z",
                TaskPriority::Low,
                DeadlineStatus::Active,
                "2026-07-01T00:00:00Z",
            ),
            task(
                "high",
                "2026-07-10T23:59:00Z",
                TaskPriority::High,
                DeadlineStatus::Active,
                "2026-07-02T00:00:00Z",
            ),
        ];

        let ids: Vec<String> = sort_deadline_tasks(&tasks)
            .into_iter()
            .map(|task| task.id)
            .collect();
        assert_eq!(ids, vec!["high", "low", "later"]);
    }
}
