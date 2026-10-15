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
  /** False when the command exited cleanly but removed nothing. */
  effective: boolean;
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

/** Storage detail: one node of the live tree (Rust `tree::View`). */
export type NodeState = "pending" | "measuring" | "done";

export interface BreakdownNode {
  id: number;
  name: string;
  /** The whole subtree, in bytes of allocated blocks. */
  bytes: number;
  own_bytes: number;
  state: NodeState;
  /** Biggest first, capped; the rest is summed in other_*. */
  children: BreakdownNode[];
  other_count: number;
  other_bytes: number;
  /** All of its children, shown or not. */
  child_count: number;
  /** Opening it shows rows (children, or folded folders measured on demand). */
  openable: boolean;
}

/** What each `breakdown` event carries (Rust `breakdown::Payload`). */
export interface BreakdownPayload {
  /** The category rows, one level deep. */
  root: BreakdownNode;
  /** The opened row with its children, when the user drilled in. */
  focus: BreakdownNode | null;
  files: number;
  unreadable: number;
  running: boolean;
  /** Unix seconds. */
  started: number;
  finished: number | null;
}
