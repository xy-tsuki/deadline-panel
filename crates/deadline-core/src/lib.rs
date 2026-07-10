pub mod ffi;
pub mod model;
pub mod parser;
pub mod repository;
pub mod sorting;
pub mod storage;

pub const DEADLINE_CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use model::{
    DeadlineSource, DeadlineStatus, DeadlineTask, NewTaskInput, TaskPriority, UpdateTaskInput,
};
pub use parser::{parse_quick_add, parse_quick_add_with_now, QuickAddParseResult};
pub use repository::{DeadlineRepository, InMemoryDeadlineRepository, RepositoryError};
pub use sorting::{
    get_actionable_tasks, get_current_tasks, get_nearest_deadline, get_today_focus,
    sort_deadline_tasks,
};
