import { DEFAULT_PUBLIC_SYNC_URL, DEFAULT_PUBLIC_SYNC_ANON_KEY } from "./defaultConfig";
import { fetchWithTimeout } from "./transport";
import { loadSettings, saveSettings } from "../storage/storage";
import { createClient, SupabaseClient } from "@supabase/supabase-js";
import { DeadlineTask } from "../domain/task";
import { acknowledgeDelete, loadPendingDeletes } from "../storage/storage";

const SUPABASE_URL_KEY = "sync_supabase_url";
const SUPABASE_ANON_KEY = "sync_supabase_anon_key";
const SUPABASE_CODE_KEY = "sync_code";
const DEFAULT_SUPABASE_URL = normalizeSupabaseUrl(import.meta.env.VITE_SUPABASE_URL?.trim() || DEFAULT_PUBLIC_SYNC_URL);
const DEFAULT_SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY?.trim() || DEFAULT_PUBLIC_SYNC_ANON_KEY;

export interface SupabaseSyncSettings {
  url: string;
  anonKey: string;
  syncCode: string;
}

interface DeadlineTaskRow {
  task_id: string;
  title: string;
  due_at: string;
  priority: DeadlineTask["priority"];
  status: DeadlineTask["status"];
  notes: string;
  source: DeadlineTask["source"];
  is_current: boolean;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

let cachedClient: SupabaseClient | null = null;
let cachedKey = "";

export async function loadSupabaseSyncSettings(): Promise<SupabaseSyncSettings> {
  const values = await loadSettings([SUPABASE_URL_KEY, SUPABASE_ANON_KEY, SUPABASE_CODE_KEY]);
  const customUrl = values[SUPABASE_URL_KEY] ?? "";
  const customAnonKey = values[SUPABASE_ANON_KEY] ?? "";
  return {
    url: normalizeSupabaseUrl(customUrl) || DEFAULT_SUPABASE_URL,
    anonKey: customAnonKey || DEFAULT_SUPABASE_ANON_KEY,
    syncCode: values[SUPABASE_CODE_KEY] ?? ""
  };
}

export async function saveSupabaseSyncSettings(settings: SupabaseSyncSettings): Promise<void> {
  await saveSettings({ [SUPABASE_URL_KEY]: normalizeSupabaseUrl(settings.url), [SUPABASE_ANON_KEY]: settings.anonKey.trim(), [SUPABASE_CODE_KEY]: settings.syncCode.trim() });
  clearCachedClient();
}

export function generateSyncCode(): string {
  const bytes = new Uint8Array(48);
  crypto.getRandomValues(bytes);
  return `dp_sync_v1_${base64Url(bytes)}`;
}

export function hasDefaultSupabaseConfig(): boolean {
  return Boolean(DEFAULT_SUPABASE_URL && DEFAULT_SUPABASE_ANON_KEY);
}

export function clearCachedClient(): void {
  cachedClient = null;
  cachedKey = "";
}

export async function syncTasksWithSupabase(localTasks: DeadlineTask[]): Promise<{ tasks: DeadlineTask[]; assertCurrent: () => Promise<void>; upload: () => Promise<void> }> {
  const { client, codeHash, namespace, settings } = await getConfiguredSyncContext();
  const assertCurrent = async () => { const current = await loadSupabaseSyncSettings(); if (current.url !== settings.url || current.anonKey !== settings.anonKey || current.syncCode !== settings.syncCode) throw new Error("sync_context_changed"); };
  const pending = await loadPendingDeletes(namespace);
  for (const id of pending) {
    const { error } = await client.rpc("deadline_sync_delete", { p_sync_code_hash: codeHash, p_task_id: id });
    if (error) throw error;
    await acknowledgeDelete(namespace, id);
  }
  const remoteTasks = await fetchRemoteTasks(client, codeHash);
  const { data: deleted, error: deletedError } = await client.rpc("deadline_sync_pull_deleted", { p_sync_code_hash: codeHash });
  if (deletedError) throw new Error("Cloud tombstone protocol unavailable: " + deletedError.message);
  const deletedIds = new Set(((deleted ?? []) as { task_id: string }[]).map((row) => row.task_id));
  for (const id of pending) deletedIds.add(id);
  const mergedTasks = mergeTasks(localTasks, remoteTasks).filter((task) => !deletedIds.has(task.id));
  return { tasks: mergedTasks, assertCurrent, upload: async () => { await assertCurrent(); await upsertTasks(client, codeHash, mergedTasks); } };
}

export async function pushTaskToSupabase(task: DeadlineTask): Promise<void> {
  const context = await getOptionalSyncContext();
  if (!context) return;
  await upsertTasks(context.client, context.codeHash, [task]);
}

export async function pushTasksToSupabase(tasks: DeadlineTask[]): Promise<void> {
  if (tasks.length === 0) return;
  const context = await getOptionalSyncContext();
  if (!context) return;
  await upsertTasks(context.client, context.codeHash, tasks);
}

export async function deleteTaskFromSupabase(id: string): Promise<void> {
  const context = await getOptionalSyncContext();
  if (!context) return;

  const { error } = await context.client.rpc("deadline_sync_delete", {
    p_sync_code_hash: context.codeHash,
    p_task_id: id
  });
  if (error) throw error;
}

async function getConfiguredSyncContext(): Promise<{ client: SupabaseClient; codeHash: string; namespace: string; settings: SupabaseSyncSettings }> {
  const settings = await loadSupabaseSyncSettings();
  if (!settings.url || !settings.anonKey || !settings.syncCode) {
    throw new Error("Supabase sync is not configured.");
  }
  const codeHash = await hashSyncCode(settings.syncCode);
  return { client: getClient(settings), codeHash, namespace: settings.url + "|" + codeHash, settings };
}

async function getOptionalSyncContext(): Promise<{ client: SupabaseClient; codeHash: string } | null> {
  const settings = await loadSupabaseSyncSettings();
  if (!settings.url || !settings.anonKey || !settings.syncCode) return null;
  return {
    client: getClient(settings),
    codeHash: await hashSyncCode(settings.syncCode)
  };
}

function getClient(settings: SupabaseSyncSettings): SupabaseClient {
  const url = normalizeSupabaseUrl(settings.url);
  const nextKey = `${url}|${settings.anonKey}`;
  if (cachedClient && cachedKey === nextKey) {
    return cachedClient;
  }

  cachedClient = createClient(url, settings.anonKey, {
    global: { fetch: fetchWithTimeout },
    auth: {
      persistSession: false,
      autoRefreshToken: false
    }
  });
  cachedKey = nextKey;
  return cachedClient;
}

async function fetchRemoteTasks(client: SupabaseClient, codeHash: string): Promise<DeadlineTask[]> {
  const { data, error } = await client.rpc("deadline_sync_pull", {
    p_sync_code_hash: codeHash
  });

  if (error) throw error;
  return ((data ?? []) as DeadlineTaskRow[]).map(taskFromRow);
}

async function upsertTasks(client: SupabaseClient, codeHash: string, tasks: DeadlineTask[]): Promise<void> {
  if (tasks.length === 0) return;
  const { error } = await client.rpc("deadline_sync_upsert", {
    p_sync_code_hash: codeHash,
    p_tasks: tasks.map(taskToRow)
  });
  if (error) throw error;
}

export function mergeTasks(localTasks: DeadlineTask[], remoteTasks: DeadlineTask[]): DeadlineTask[] {
  const merged = new Map<string, DeadlineTask>();
  for (const task of remoteTasks) {
    parseDate(task.updatedAt);
    merged.set(task.id, task);
  }

  for (const task of localTasks) {
    parseDate(task.updatedAt);
    const remote = merged.get(task.id);
    if (!remote || compareUpdatedAt(task, remote) > 0) {
      merged.set(task.id, task);
    }
  }

  return [...merged.values()];
}

function compareUpdatedAt(left: DeadlineTask, right: DeadlineTask): number {
  return parseDate(left.updatedAt) - parseDate(right.updatedAt);
}

function parseDate(value: string): number {
  const time = new Date(value).getTime();
  if (!Number.isFinite(time)) throw new Error(`Invalid task timestamp: ${value}`);
  return time;
}

function taskToRow(task: DeadlineTask): DeadlineTaskRow {
  return {
    task_id: task.id,
    title: task.title,
    due_at: task.dueAt,
    priority: task.priority,
    status: task.status,
    notes: task.notes,
    source: task.source,
    is_current: task.isCurrent,
    created_at: task.createdAt,
    updated_at: task.updatedAt,
    completed_at: task.completedAt ?? null
  };
}

function taskFromRow(row: DeadlineTaskRow): DeadlineTask {
  return {
    id: row.task_id,
    title: row.title,
    dueAt: row.due_at,
    priority: row.priority,
    status: row.status,
    notes: row.notes,
    source: row.source,
    isCurrent: row.is_current,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    completedAt: row.completed_at
  };
}

async function hashSyncCode(syncCode: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(syncCode.trim()));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function base64Url(bytes: Uint8Array): string {
  let binary = "";
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary).split("+").join("-").split("/").join("_").split("=").join("");
}

function normalizeSupabaseUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    const url = new URL(trimmed);
    return `${url.protocol}//${url.host}`;
  } catch {
    return trimmed;
  }
}

export async function getSyncNamespace(): Promise<string | undefined> {
  const settings = await loadSupabaseSyncSettings();
  return settings.url && settings.anonKey && settings.syncCode ? settings.url + "|" + await hashSyncCode(settings.syncCode) : undefined;
}
