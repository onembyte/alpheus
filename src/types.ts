export type Tier = "safe" | "with-care" | "manual";
export type ActionKind = "delete" | "command" | "explain";

export interface Card {
  id: string;
  title: string;
  description: string;
  tier: Tier;
  size_kb: number;
  paths: string[];
  proof: string | null;
  action: ActionKind;
  command_display: string | null;
}

export interface DiskUsage {
  total_kb: number;
  free_kb: number;
}

export interface DryRunEntry {
  path: string;
  size_kb: number;
}

export interface DryRun {
  entries: DryRunEntry[];
  total_kb: number;
  method: "trash" | "delete" | "command";
  command: string | null;
  warning: string | null;
}

export interface ExecResult {
  freed_kb: number;
  method: string;
  message: string;
}

export interface HistoryEntry {
  timestamp: number;
  card_id: string;
  title: string;
  freed_kb: number;
  method: string;
  auto: boolean;
}

export interface AppSettings {
  auto_scan_secs: number;
  notify_below_gb: number;
  auto_clean: boolean;
  auto_clean_ids: string[];
  last_auto_scan_ts: number;
  menu_bar_only: boolean;
}
