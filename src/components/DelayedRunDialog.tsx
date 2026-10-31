import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { call } from "@/lib/api";
import { notifyError, notifySuccess } from "@/lib/notify";
import type { AppEvent } from "@/lib/types";
import { useAppEvents } from "@/lib/useAppEvents";

interface Pending {
  runId: string;
  flowName: string;
  deadline: number;
}

/** Countdown shown on the computer before a flow with a start delay runs. */
export function DelayedRunDialog() {
  const { t } = useTranslation();
  const [pending, setPending] = useState<Pending | null>(null);
  const [now, setNow] = useState(() => Date.now());

  const onEvent = useCallback((e: AppEvent) => {
    if (e.type === "run_delayed") {
      setPending({ runId: e.run_id, flowName: e.flow_name, deadline: Date.now() + e.seconds * 1000 });
    } else if (e.type === "run_updated") {
      setPending((p) => (p && p.runId === e.run.id && e.run.status !== "delayed" ? null : p));
    }
  }, []);
  useAppEvents(onEvent);

  useEffect(() => {
    if (!pending) return;
    const timer = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, [pending]);

  if (!pending) return null;
  const seconds = Math.max(0, Math.ceil((pending.deadline - now) / 1000));

  return (
    <Dialog open onOpenChange={(open) => !open && setPending(null)}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("delayed.title")}</DialogTitle>
          <DialogDescription>{t("delayed.text", { name: pending.flowName, seconds })}</DialogDescription>
        </DialogHeader>
        <div className="py-2 text-center text-5xl font-semibold tabular-nums">{seconds}</div>
        <DialogFooter>
          <Button
            variant="destructive"
            onClick={() =>
              call({ op: "run_cancel", id: pending.runId })
                .then(() => {
                  notifySuccess(t("delayed.canceled"));
                  setPending(null);
                })
                .catch(notifyError)
            }
          >
            {t("delayed.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
