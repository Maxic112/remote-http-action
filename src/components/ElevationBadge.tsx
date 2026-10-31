import { ShieldAlertIcon, ShieldCheckIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmDialog } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { call, useSystemInfo } from "@/lib/api";
import { notifyError } from "@/lib/notify";

/** Always visible indicator of administrator rights; restarts elevated on click. */
export function ElevationBadge() {
  const { t } = useTranslation();
  const { data } = useSystemInfo();
  const [confirm, setConfirm] = useState(false);
  if (!data) return null;

  if (data.elevated) {
    return (
      <Tooltip>
        <TooltipTrigger
          render={
            <div
              className="flex items-center gap-1.5 rounded-md bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700"
              data-testid="elevation-admin"
            >
              <ShieldCheckIcon className="size-4" />
              {t("elevation.admin")}
            </div>
          }
        />
        <TooltipContent>{t("elevation.adminHint")}</TooltipContent>
      </Tooltip>
    );
  }

  return (
    <>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              variant="outline"
              size="sm"
              className="border-amber-300 bg-amber-50 text-amber-800 hover:bg-amber-100"
              onClick={() => setConfirm(true)}
              data-testid="elevation-user"
            >
              <ShieldAlertIcon />
              {t("elevation.user")}
            </Button>
          }
        />
        <TooltipContent className="max-w-xs">{t("elevation.userHint")}</TooltipContent>
      </Tooltip>
      <ConfirmDialog
        open={confirm}
        onOpenChange={setConfirm}
        title={t("elevation.restartTitle")}
        description={t("elevation.restartText")}
        confirmLabel={t("elevation.restart")}
        onConfirm={() => call({ op: "app_restart_elevated" }).catch(notifyError)}
      />
    </>
  );
}
