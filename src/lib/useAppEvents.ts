import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { queryKeys } from "./api";
import type { AppEvent } from "./types";

const entityKeys: Record<string, readonly string[][]> = {
  keys: [[...queryKeys.keys], [...queryKeys.groups]],
  groups: [[...queryKeys.groups], [...queryKeys.keys]],
  flows: [[...queryKeys.flows], [...queryKeys.groups]],
  settings: [[...queryKeys.settings], [...queryKeys.server]],
};

/** Keeps cached data in sync with changes made from the CLI, HTTP clients or the tray. */
export function useAppEvents(onEvent?: (e: AppEvent) => void) {
  const qc = useQueryClient();
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<AppEvent>("app-event", ({ payload }) => {
      switch (payload.type) {
        case "data_changed":
          for (const key of entityKeys[payload.entity] ?? []) void qc.invalidateQueries({ queryKey: key });
          break;
        case "run_updated":
          void qc.invalidateQueries({ queryKey: queryKeys.runs });
          break;
        case "server_status_changed":
          void qc.invalidateQueries({ queryKey: queryKeys.server });
          void qc.invalidateQueries({ queryKey: queryKeys.settings });
          break;
      }
      onEvent?.(payload);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [qc, onEvent]);
}
