// Mirrors the Rust types in src-tauri/src/model.rs, settings.rs and core.rs.

export type Id = number;

export interface ApiKey {
  id: Id;
  name: string;
  key_prefix: string;
  enabled: boolean;
  group_ids: Id[];
  created_at: string;
  last_used_at: string | null;
}

export interface IssuedKey {
  key: ApiKey;
  secret: string;
}

export interface AccessGroup {
  id: Id;
  name: string;
  description: string;
  key_ids: Id[];
  flow_ids: Id[];
  created_at: string;
}

export type ResponseMode = "sync" | "async";
export type Concurrency = "allow" | "reject_if_running" | "queue";
export type OnError = "stop" | "continue";

export interface ClientMeta {
  icon: string | null;
  color: string | null;
  category: string | null;
  tags: string[];
  confirm_on_client: boolean;
}

export type ActionType = "delay" | "shutdown" | "reboot" | "sleep" | "cli_command";

export interface ActionDef {
  type: ActionType;
  params: Record<string, unknown>;
  on_error: OnError;
  timeout_sec: number | null;
  run_as_admin: boolean;
  capture_output: boolean;
}

export interface FlowInput {
  name: string;
  description: string;
  path: string;
  enabled: boolean;
  response_mode: ResponseMode;
  timeout_sec: number;
  concurrency: Concurrency;
  cooldown_sec: number;
  start_delay_sec: number;
  cancelable: boolean;
  client_meta: ClientMeta;
  input_schema: unknown | null;
  actions: ActionDef[];
}

export interface Flow extends FlowInput {
  id: Id;
  created_at: string;
  updated_at: string;
}

export type RunStatus = "pending" | "delayed" | "running" | "success" | "failed" | "canceled" | "timeout";
export type StepStatus = "success" | "failed" | "skipped" | "canceled" | "timeout";

export interface StepResult {
  index: number;
  type: string;
  status: StepStatus;
  started_at: string;
  finished_at: string;
  exit_code?: number;
  stdout?: string;
  stderr?: string;
  error?: string;
  capture_output: boolean;
}

export interface Run {
  id: string;
  flow_id: Id | null;
  flow_name: string;
  flow_path: string;
  source: "http" | "cli" | "ui";
  api_key_id: Id | null;
  api_key_name: string | null;
  client_ip: string | null;
  status: RunStatus;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
  steps: StepResult[];
}

export interface Settings {
  server: {
    enabled: boolean;
    bind_address: string;
    port: number;
    allowed_ips: string[];
    allowed_hosts: string[];
    trusted_proxies: string[];
    flow_prefix: string;
    allowed_path: string;
    status_path: string;
    public_status: boolean;
    hide_forbidden_flows: boolean;
    brute_force: { enabled: boolean; max_failures: number; window_sec: number; ban_sec: number };
  };
  cli: { enabled: boolean };
  app: { close_to_tray: boolean; language: "system" | "ru" | "en"; run_history_limit: number };
}

export interface ServerStatus {
  state: "stopped" | "running" | "error";
  address: string | null;
  error: string | null;
}

export interface ShellInfo {
  id: string;
  name: string;
  path: string;
}

export interface SystemInfo {
  version: string;
  build: "dev" | "release";
  elevated: boolean;
  pid: number;
  paths: { data_dir: string; db_path: string; portable: boolean };
  ipc_endpoint: string;
  shells: ShellInfo[];
  interfaces: { name: string; ip: string; loopback: boolean }[];
}

export interface Ban {
  ip: string;
  seconds_left: number;
}

export type Request =
  | { op: "keys_list" }
  | { op: "key_create"; name: string; group_ids: Id[] }
  | { op: "key_update"; id: Id; name?: string; enabled?: boolean; group_ids?: Id[] }
  | { op: "key_rotate"; id: Id }
  | { op: "key_delete"; id: Id }
  | { op: "groups_list" }
  | { op: "group_create"; name: string; description: string; key_ids: Id[]; flow_ids: Id[] }
  | { op: "group_update"; id: Id; name?: string; description?: string; key_ids?: Id[]; flow_ids?: Id[] }
  | { op: "group_delete"; id: Id }
  | { op: "flows_list" }
  | { op: "flow_get"; flow: string }
  | { op: "flow_create"; flow: FlowInput }
  | { op: "flow_update"; id: Id; flow: FlowInput }
  | { op: "flow_set_enabled"; flow: string; enabled: boolean }
  | { op: "flow_delete"; flow: string }
  | { op: "flow_run"; flow: string; wait: boolean }
  | { op: "runs_list"; flow_id?: Id; limit?: number }
  | { op: "run_get"; id: string }
  | { op: "run_cancel"; id: string }
  | { op: "settings_get"; path?: string }
  | { op: "settings_update"; settings: Settings }
  | { op: "settings_set"; path: string; value: unknown }
  | { op: "server_status" }
  | { op: "server_start" }
  | { op: "server_stop" }
  | { op: "bans_list" }
  | { op: "ban_remove"; ip: string }
  | { op: "system_info" }
  | { op: "action_types" }
  | { op: "app_show" }
  | { op: "app_quit" }
  | { op: "app_restart_elevated" };

export type AppEvent =
  | { type: "data_changed"; entity: string }
  | { type: "run_updated"; run: Run }
  | { type: "run_delayed"; run_id: string; flow_name: string; seconds: number; cancelable: boolean }
  | { type: "server_status_changed" };
