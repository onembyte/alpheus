import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BreakdownPayload,
  Card,
  DiskUsage,
  DryRun,
  ExecResult,
  HistoryEntry,
} from "./types";

export const diskUsage = () => invoke<DiskUsage>("disk_usage");
export const scan = () => invoke<Card[]>("scan");
export const dryRun = (id: string) => invoke<DryRun>("dry_run", { id });
export const execute = (id: string) => invoke<ExecResult>("execute", { id });
export const getHistory = () => invoke<HistoryEntry[]>("history");
export const getSettings = () => invoke<AppSettings>("get_settings");
export const setSettings = (newSettings: AppSettings) =>
  invoke<void>("set_settings", { newSettings });

// Storage detail: the walk runs in the backend; payloads arrive as
// `breakdown` events, and each command also returns one at once.
export const breakdownStart = () => invoke<BreakdownPayload>("breakdown_start");
export const breakdownFocus = (id: number | null) =>
  invoke<BreakdownPayload | null>("breakdown_focus", { id });
export const breakdownCancel = () => invoke<void>("breakdown_cancel");
export const breakdownCached = () => invoke<BreakdownPayload | null>("breakdown_cached");
