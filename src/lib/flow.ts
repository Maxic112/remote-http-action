import type { ActionDef, ActionType, Flow, FlowInput } from "./types";

export const ACTION_TYPES: ActionType[] = ["shutdown", "reboot", "sleep", "delay", "cli_command", "run_script", "launch_app"];

export function defaultParams(type: ActionType, defaultShell = "cmd"): Record<string, unknown> {
  switch (type) {
    case "delay":
      return { seconds: 5 };
    case "shutdown":
    case "reboot":
      return { force: false };
    case "sleep":
      return { hibernate: false };
    case "cli_command":
      return { shell: defaultShell, command: "", working_dir: null };
    case "run_script":
      return { shell: defaultShell, script_path: "", args: [], working_dir: null };
    case "launch_app":
      return { path: "", args: [], working_dir: null, wait: "none" };
  }
}

export function newAction(type: ActionType, defaultShell?: string): ActionDef {
  return {
    type,
    params: defaultParams(type, defaultShell),
    on_error: "stop",
    timeout_sec: null,
    run_as_admin: false,
    capture_output: false,
  };
}

export function emptyFlow(): FlowInput {
  return {
    name: "",
    description: "",
    path: "",
    enabled: true,
    response_mode: "sync",
    timeout_sec: 60,
    concurrency: "reject_if_running",
    cooldown_sec: 0,
    start_delay_sec: 0,
    cancelable: true,
    client_meta: { icon: null, color: null, category: null, tags: [], confirm_on_client: false },
    input_schema: null,
    actions: [],
  };
}

/** Strips server-managed fields so a stored flow can be edited and sent back. */
export function toInput(flow: Flow): FlowInput {
  const { id: _id, created_at: _c, updated_at: _u, ...input } = flow;
  return input;
}

/** Suggests a URL path from a flow name: "Sleep PC" → "sleep-pc". */
export function slugify(name: string): string {
  return name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function hasAdminSteps(flow: Pick<FlowInput, "actions">): boolean {
  return flow.actions.some((a) => a.run_as_admin);
}

export function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (to < 0 || to >= items.length) return items;
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

/** Textarea with one entry per line ⇄ string array. */
export const linesToList = (text: string) =>
  text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
export const listToLines = (list: string[]) => list.join("\n");

export function flowUrl(prefix: string, path: string): string {
  const p = path.trim().replace(/^\/+|\/+$/g, "");
  return `${prefix}${p ? `${p}/` : ""}`;
}
