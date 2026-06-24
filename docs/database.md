# Database Design

SQLite is the target local database.

## Tables

### tasks

```sql
CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  due_at TEXT NOT NULL,
  priority TEXT NOT NULL CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
  status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'postponed')),
  notes TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT 'manual',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status_due_at
ON tasks (status, due_at);

CREATE INDEX IF NOT EXISTS idx_tasks_due_priority
ON tasks (due_at, priority);
```

### app_settings

```sql
CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## Status Semantics

- `active`: visible in normal deadline ranking.
- `completed`: hidden from Top 3, retained for history.
- `postponed`: still visible, but after active tasks with the same deadline.

## Priority Weight

```text
urgent = 4
high   = 3
medium = 2
low    = 1
```

## Sorting Contract

Frontend and backend use the same contract:

1. Completed tasks are excluded from focus ranking.
2. Earlier `due_at` wins.
3. Higher priority wins when `due_at` is equal.
4. Active beats postponed when both deadline and priority are equal.
5. Older `created_at` wins as final stable tie breaker.

