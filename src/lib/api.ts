import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  AccessGroup,
  ApiKey,
  AutostartStatus,
  Ban,
  Flow,
  LogsInfo,
  Request,
  Run,
  ServerStatus,
  Settings,
  SystemInfo,
  TlsInfo,
} from "./types";

export class ApiError extends Error {
  code: string;

  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

export function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) return e;
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    const { code, message } = e as { code: unknown; message: unknown };
    return new ApiError(String(code), String(message));
  }
  return new ApiError("INTERNAL", e instanceof Error ? e.message : String(e));
}

/** Sends a request to the core. The same requests are available through the CLI. */
export async function call<T>(request: Request): Promise<T> {
  try {
    return await invoke<T>("api", { request });
  } catch (e) {
    throw toApiError(e);
  }
}

export const queryKeys = {
  keys: ["keys"] as const,
  groups: ["groups"] as const,
  flows: ["flows"] as const,
  runs: ["runs"] as const,
  settings: ["settings"] as const,
  server: ["server"] as const,
  system: ["system"] as const,
  bans: ["bans"] as const,
  autostart: ["autostart"] as const,
  tls: ["tls"] as const,
  logsInfo: ["logs-info"] as const,
};

export const useKeys = () => useQuery({ queryKey: queryKeys.keys, queryFn: () => call<ApiKey[]>({ op: "keys_list" }) });
export const useGroups = () => useQuery({ queryKey: queryKeys.groups, queryFn: () => call<AccessGroup[]>({ op: "groups_list" }) });
export const useFlows = () => useQuery({ queryKey: queryKeys.flows, queryFn: () => call<Flow[]>({ op: "flows_list" }) });
export const useRuns = () => useQuery({ queryKey: queryKeys.runs, queryFn: () => call<Run[]>({ op: "runs_list", limit: 200 }) });
export const useSettings = () => useQuery({ queryKey: queryKeys.settings, queryFn: () => call<Settings>({ op: "settings_get" }) });
export const useServerStatus = () => useQuery({ queryKey: queryKeys.server, queryFn: () => call<ServerStatus>({ op: "server_status" }) });
export const useSystemInfo = () => useQuery({ queryKey: queryKeys.system, queryFn: () => call<SystemInfo>({ op: "system_info" }) });
export const useBans = () =>
  useQuery({ queryKey: queryKeys.bans, queryFn: () => call<Ban[]>({ op: "bans_list" }), refetchInterval: 15_000 });

export const useAutostart = () =>
  useQuery({ queryKey: queryKeys.autostart, queryFn: () => call<AutostartStatus>({ op: "autostart_status" }) });
export const useTlsInfo = () => useQuery({ queryKey: queryKeys.tls, queryFn: () => call<TlsInfo>({ op: "tls_info" }) });
export const useLogsInfo = () => useQuery({ queryKey: queryKeys.logsInfo, queryFn: () => call<LogsInfo>({ op: "logs_info" }) });

/** Mutation that refetches the given queries on success. */
export function useApiMutation<T>(invalidate: readonly (readonly string[])[] = []) {
  const qc = useQueryClient();
  return useMutation<T, ApiError, Request>({
    mutationFn: (req) => call<T>(req),
    onSuccess: () => {
      for (const key of invalidate) void qc.invalidateQueries({ queryKey: key });
    },
  });
}
