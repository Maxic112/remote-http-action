import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { useServerStatus } from "@/lib/api";

export function ServerBadge() {
  const { t } = useTranslation();
  const { data } = useServerStatus();
  if (!data) return null;
  const label =
    data.state === "running"
      ? t("server.running", { address: data.address ?? "" })
      : data.state === "error"
        ? t("server.error")
        : t("server.stopped");
  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground" title={data.error ?? undefined} data-testid="server-badge">
      <span
        className={cn(
          "size-2 rounded-full",
          data.state === "running" && "bg-emerald-500",
          data.state === "stopped" && "bg-zinc-400",
          data.state === "error" && "bg-red-500",
        )}
      />
      {label}
    </div>
  );
}
