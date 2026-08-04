import { invoke } from "@tauri-apps/api/core";
import type { Card, DiskUsage, DryRun, ExecResult, HistoryEntry } from "./types";

export const diskUsage = () => invoke<DiskUsage>("disk_usage");
export const scan = () => invoke<Card[]>("scan");
export const dryRun = (id: string) => invoke<DryRun>("dry_run", { id });
export const execute = (id: string) => invoke<ExecResult>("execute", { id });
export const getHistory = () => invoke<HistoryEntry[]>("history");
