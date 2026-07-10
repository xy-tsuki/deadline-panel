use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl TaskPriority {
    pub fn weight(self) -> u8 {
        match self {
            Self::Urgent => 4,
            Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "urgent" => Some(Self::Urgent),
            _ => None,
        }
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeadlineStatus {
    Active,
    Completed,
    Postponed,
}

impl DeadlineStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Postponed => "postponed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "postponed" => Some(Self::Postponed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeadlineSource {
    Manual,
    Command,
    Codex,
    Seed,
}

impl DeadlineSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Command => "command",
            Self::Codex => "codex",
            Self::Seed => "seed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "manual" => Some(Self::Manual),
            "command" => Some(Self::Command),
            "codex" => Some(Self::Codex),
            "seed" => Some(Self::Seed),
            _ => None,
        }
    }
}

impl Default for DeadlineSource {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineTask {
    pub id: String,
    pub title: String,
    pub due_at: String,
    pub priority: TaskPriority,
    pub status: DeadlineStatus,
    pub notes: String,
    pub source: DeadlineSource,
    pub is_current: bool,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTaskInput {
    pub title: String,
    pub due_at: String,
    pub priority: TaskPriority,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub source: Option<DeadlineSource>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskInput {
    pub title: Option<String>,
    pub due_at: Option<String>,
    pub priority: Option<TaskPriority>,
    pub notes: Option<String>,
    pub status: Option<DeadlineStatus>,
    pub source: Option<DeadlineSource>,
    pub is_current: Option<bool>,
    pub completed_at: Option<Option<String>>,
}
