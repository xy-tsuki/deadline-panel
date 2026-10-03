import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
const settings = new Map([["sync_supabase_url", "https://synthetic.invalid"], ["sync_supabase_anon_key", "synthetic-public-key"], ["sync_code", "synthetic-test-code"]]);
const calls = [];
let pending = ["deleted"];
let tombstoneFailure = false;
let remote = [];
globalThis.__syncTest = {
  loadSetting: async (key) => settings.get(key) ?? null,
  saveSetting: async (key, value) => settings.set(key, value),
  loadSettings: async (keys) => Object.fromEntries(keys.map(key => [key, settings.get(key) ?? null])),
  saveSettings: async (values) => { for (const [key, value] of Object.entries(values)) settings.set(key, value); },
  loadPendingDeletes: async () => [...pending],
  acknowledgeDelete: async (_, id) => { pending = pending.filter((item) => item !== id); },
  createClient: () => ({ rpc: async (name, args) => {
    calls.push({ name, args });
    if (name === "deadline_sync_pull_deleted") return tombstoneFailure ? { error: { message: "missing RPC" } } : { data: [{ task_id: "deleted" }] };
    if (name === "deadline_sync_pull") return { data: remote };
    return { error: null };
  } })
};
const { webcrypto } = await import("node:crypto");
globalThis.crypto = webcrypto;
let source = readFileSync(new URL("../src/sync/supabaseSync.ts", import.meta.url), "utf8").replace(/^import .*;\r?\n/gm, "").replaceAll("import.meta.env", "({})");
source = 'const fetchWithTimeout = () => {}, DEFAULT_PUBLIC_SYNC_URL = "", DEFAULT_PUBLIC_SYNC_ANON_KEY = "";\n' + "const { createClient, loadSetting, saveSetting, loadSettings, saveSettings, loadPendingDeletes, acknowledgeDelete } = globalThis.__syncTest;\n" + source;
const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const sync = await import("data:text/javascript;base64," + Buffer.from(compiled).toString("base64"));
const task = (id, updatedAt, title = id) => ({ id, title, dueAt: "2026-11-01T00:00:00Z", priority: "medium", status: "active", notes: "", source: "manual", isCurrent: false, createdAt: "2026-01-01T00:00:00Z", updatedAt, completedAt: null });
const local = task("tie", "2026-10-03T10:00:00+09:00", "local");
const distant = task("tie", "2026-10-03T01:00:00.000Z", "remote");
assert.equal(sync.mergeTasks([local], [distant])[0].title, "remote", "equal instants use remote winner");
assert.equal(sync.mergeTasks([task("tie", "2026-10-03T02:00:00Z", "new")], [distant])[0].title, "new");
assert.throws(() => sync.mergeTasks([task("tie", "invalid")], [distant]), /Invalid task timestamp/);
const prepared = await sync.syncTasksWithSupabase([task("deleted", "2026-10-03T00:00:00Z"), local]);
assert.equal(prepared.tasks.some((item) => item.id === "deleted"), false);
assert.equal(pending.length, 0);
assert.deepEqual(calls.map((call) => call.name), ["deadline_sync_delete", "deadline_sync_pull", "deadline_sync_pull_deleted"]);
await prepared.upload();
assert.equal(calls.at(-1).name, "deadline_sync_upsert");
assert.equal(calls.at(-1).args.p_tasks.some((item) => item.task_id === "deleted"), false);
tombstoneFailure = true;
const before = calls.filter((call) => call.name === "deadline_sync_upsert").length;
await assert.rejects(sync.syncTasksWithSupabase([local]), /tombstone protocol unavailable/);
assert.equal(calls.filter((call) => call.name === "deadline_sync_upsert").length, before);
tombstoneFailure = false;
const switching = await sync.syncTasksWithSupabase([local]);
settings.set("sync_code", "new-synthetic-code");
await assert.rejects(switching.assertCurrent(), /sync_context_changed/);
await assert.rejects(switching.upload(), /sync_context_changed/);
assert.equal(calls.filter(call => call.name === "deadline_sync_upsert").length, before);
console.log("Passed: timestamp tie/timezone, newer local, invalid timestamp, delete retry order, tombstone filtering, missing protocol prevents upload.");
delete globalThis.__syncTest;
