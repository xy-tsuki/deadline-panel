-- Deadline Panel 0.4.0 Supabase sync-code schema
-- Run this in the Supabase SQL editor for your project.
--
-- Security model:
-- - Users do not need email/password accounts.
-- - A long local sync code is the shared secret.
-- - The app sends only sha256(sync_code) to Supabase.
-- - Direct table access is blocked by RLS; the app uses RPC functions.

create table if not exists public.deadline_sync_tasks (
  sync_code_hash text not null,
  task_id text not null,
  title text not null,
  due_at timestamptz not null,
  priority text not null check (priority in ('low', 'medium', 'high', 'urgent')),
  status text not null check (status in ('active', 'completed', 'postponed')),
  notes text not null default '',
  source text not null check (source in ('manual', 'command', 'codex', 'seed')),
  is_current boolean not null default false,
  created_at timestamptz not null,
  updated_at timestamptz not null,
  completed_at timestamptz,
  primary key (sync_code_hash, task_id)
);

create index if not exists idx_deadline_sync_tasks_hash_status_due
on public.deadline_sync_tasks (sync_code_hash, status, due_at);

create index if not exists idx_deadline_sync_tasks_hash_updated
on public.deadline_sync_tasks (sync_code_hash, updated_at desc);

alter table public.deadline_sync_tasks enable row level security;

revoke all on public.deadline_sync_tasks from anon, authenticated;

create or replace function public.deadline_sync_pull(p_sync_code_hash text)
returns table (
  task_id text,
  title text,
  due_at text,
  priority text,
  status text,
  notes text,
  source text,
  is_current boolean,
  created_at text,
  updated_at text,
  completed_at text
)
language sql
security definer
set search_path = public
as $$
  select
    task_id,
    title,
    due_at::text,
    priority,
    status,
    notes,
    source,
    is_current,
    created_at::text,
    updated_at::text,
    completed_at::text
  from public.deadline_sync_tasks
  where sync_code_hash = p_sync_code_hash
  order by due_at asc;
$$;

create or replace function public.deadline_sync_upsert(p_sync_code_hash text, p_tasks jsonb)
returns void
language plpgsql
security definer
set search_path = public
as $$
begin
  insert into public.deadline_sync_tasks (
    sync_code_hash,
    task_id,
    title,
    due_at,
    priority,
    status,
    notes,
    source,
    is_current,
    created_at,
    updated_at,
    completed_at
  )
  select
    p_sync_code_hash,
    task->>'task_id',
    task->>'title',
    (task->>'due_at')::timestamptz,
    task->>'priority',
    task->>'status',
    coalesce(task->>'notes', ''),
    task->>'source',
    coalesce((task->>'is_current')::boolean, false),
    (task->>'created_at')::timestamptz,
    (task->>'updated_at')::timestamptz,
    nullif(task->>'completed_at', '')::timestamptz
  from jsonb_array_elements(p_tasks) as task
  where
    p_sync_code_hash ~ '^[a-f0-9]{64}$'
    and length(task->>'task_id') > 0
    and length(task->>'title') > 0
  on conflict (sync_code_hash, task_id) do update set
    title = excluded.title,
    due_at = excluded.due_at,
    priority = excluded.priority,
    status = excluded.status,
    notes = excluded.notes,
    source = excluded.source,
    is_current = excluded.is_current,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at,
    completed_at = excluded.completed_at;
end;
$$;

create or replace function public.deadline_sync_delete(p_sync_code_hash text, p_task_id text)
returns void
language sql
security definer
set search_path = public
as $$
  delete from public.deadline_sync_tasks
  where sync_code_hash = p_sync_code_hash
    and task_id = p_task_id;
$$;

grant execute on function public.deadline_sync_pull(text) to anon, authenticated;
grant execute on function public.deadline_sync_upsert(text, jsonb) to anon, authenticated;
grant execute on function public.deadline_sync_delete(text, text) to anon, authenticated;
