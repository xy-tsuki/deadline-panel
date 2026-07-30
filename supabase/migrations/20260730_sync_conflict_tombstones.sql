create table if not exists public.deadline_sync_tombstones (
  sync_code_hash text not null,
  task_id text not null,
  deleted_at timestamptz not null default now(),
  primary key (sync_code_hash, task_id)
);

create index if not exists idx_deadline_sync_tombstones_hash_deleted
on public.deadline_sync_tombstones (sync_code_hash, deleted_at desc);

alter table public.deadline_sync_tombstones enable row level security;
revoke all on public.deadline_sync_tombstones from anon, authenticated;

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
    and p_sync_code_hash ~ '^[a-f0-9]{64}$'
  order by due_at asc;
$$;

create or replace function public.deadline_sync_pull_deleted(p_sync_code_hash text)
returns table (task_id text)
language sql
security definer
set search_path = public
as $$
  select task_id
  from public.deadline_sync_tombstones
  where sync_code_hash = p_sync_code_hash
    and p_sync_code_hash ~ '^[a-f0-9]{64}$';
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
    and not exists (
      select 1
      from public.deadline_sync_tombstones tombstone
      where tombstone.sync_code_hash = p_sync_code_hash
        and tombstone.task_id = task->>'task_id'
    )
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
    completed_at = excluded.completed_at
  where excluded.updated_at > public.deadline_sync_tasks.updated_at;
end;
$$;

create or replace function public.deadline_sync_delete(p_sync_code_hash text, p_task_id text)
returns void
language plpgsql
security definer
set search_path = public
as $$
begin
  if p_sync_code_hash !~ '^[a-f0-9]{64}$' or length(p_task_id) = 0 then
    return;
  end if;

  insert into public.deadline_sync_tombstones (sync_code_hash, task_id, deleted_at)
  values (p_sync_code_hash, p_task_id, now())
  on conflict (sync_code_hash, task_id) do update
  set deleted_at = greatest(
    public.deadline_sync_tombstones.deleted_at,
    excluded.deleted_at
  );

  delete from public.deadline_sync_tasks
  where sync_code_hash = p_sync_code_hash
    and task_id = p_task_id;
end;
$$;

revoke execute on function public.deadline_sync_pull(text) from public;
revoke execute on function public.deadline_sync_pull_deleted(text) from public;
revoke execute on function public.deadline_sync_upsert(text, jsonb) from public;
revoke execute on function public.deadline_sync_delete(text, text) from public;

grant execute on function public.deadline_sync_pull(text) to anon, authenticated;
grant execute on function public.deadline_sync_pull_deleted(text) to anon, authenticated;
grant execute on function public.deadline_sync_upsert(text, jsonb) to anon, authenticated;
grant execute on function public.deadline_sync_delete(text, text) to anon, authenticated;
