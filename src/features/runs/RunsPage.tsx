import { useState } from "react";
import { useTranslation } from "react-i18next";

import { EmptyState, PageHeader } from "@/components/common";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { call, useRuns } from "@/lib/api";
import { formatDateTime, formatDuration } from "@/lib/format";
import { notifyError } from "@/lib/notify";
import type { Run, RunStatus, StepStatus } from "@/lib/types";

const ACTIVE: RunStatus[] = ["pending", "delayed", "running"];

export function StatusBadge({ status }: { status: RunStatus | StepStatus }) {
  const { t } = useTranslation();
  const variant = status === "success" ? "secondary" : status === "failed" || status === "timeout" ? "destructive" : "outline";
  return <Badge variant={variant}>{t(`runs.statuses.${status}`)}</Badge>;
}

function RunDetails({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {t("runs.details")}: {run.flow_name}
          </DialogTitle>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div className="text-muted-foreground">{t("runs.status")}</div>
          <div>
            <StatusBadge status={run.status} />
          </div>
          <div className="text-muted-foreground">{t("runs.source")}</div>
          <div>
            {t(`runs.sources.${run.source}`)}
            {run.api_key_name && ` · ${run.api_key_name}`}
            {run.client_ip && ` · ${run.client_ip}`}
          </div>
          <div className="text-muted-foreground">{t("runs.time")}</div>
          <div>{formatDateTime(run.created_at)}</div>
          <div className="text-muted-foreground">{t("runs.duration")}</div>
          <div>{formatDuration(run.started_at, run.finished_at)}</div>
        </div>
        {run.error && <p className="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{run.error}</p>}
        <div className="grid gap-3">
          {run.steps.map((s) => (
            <div key={s.index} className="rounded-lg border p-3 text-sm">
              <div className="flex items-center justify-between">
                <span className="font-medium">
                  {t("runs.step", { n: s.index + 1 })}: {t(`actions.types.${s.type}`, { defaultValue: s.type })}
                </span>
                <StatusBadge status={s.status} />
              </div>
              {s.exit_code !== undefined && (
                <div className="mt-1 text-xs text-muted-foreground">
                  {t("runs.exitCode")}: {s.exit_code}
                </div>
              )}
              {s.error && <div className="mt-1 text-xs text-destructive">{s.error}</div>}
              {s.stdout && <pre className="mt-2 max-h-40 overflow-auto rounded bg-muted p-2 text-xs whitespace-pre-wrap">{s.stdout}</pre>}
              {s.stderr && (
                <pre className="mt-2 max-h-40 overflow-auto rounded bg-muted p-2 text-xs whitespace-pre-wrap text-destructive">
                  {s.stderr}
                </pre>
              )}
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}

export function RunsPage() {
  const { t } = useTranslation();
  const runs = useRuns();
  const [selected, setSelected] = useState<Run | null>(null);

  return (
    <div>
      <PageHeader title={t("runs.title")} subtitle={t("runs.subtitle")} />
      {runs.data && runs.data.length === 0 ? (
        <EmptyState />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("runs.time")}</TableHead>
              <TableHead>{t("runs.flow")}</TableHead>
              <TableHead>{t("runs.source")}</TableHead>
              <TableHead>{t("runs.status")}</TableHead>
              <TableHead>{t("runs.duration")}</TableHead>
              <TableHead className="w-24" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {(runs.data ?? []).map((r) => (
              <TableRow key={r.id} className="cursor-pointer" onClick={() => setSelected(r)}>
                <TableCell className="whitespace-nowrap">{formatDateTime(r.created_at)}</TableCell>
                <TableCell>
                  <div className="font-medium">{r.flow_name}</div>
                  <div className="font-mono text-xs text-muted-foreground">{r.flow_path}</div>
                </TableCell>
                <TableCell className="text-sm">
                  {t(`runs.sources.${r.source}`)}
                  {r.api_key_name && <div className="text-xs text-muted-foreground">{r.api_key_name}</div>}
                </TableCell>
                <TableCell>
                  <StatusBadge status={r.status} />
                </TableCell>
                <TableCell className="text-muted-foreground">{formatDuration(r.started_at, r.finished_at)}</TableCell>
                <TableCell>
                  {ACTIVE.includes(r.status) && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        call({ op: "run_cancel", id: r.id }).catch(notifyError);
                      }}
                    >
                      {t("runs.cancel")}
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {selected && <RunDetails run={runs.data?.find((r) => r.id === selected.id) ?? selected} onClose={() => setSelected(null)} />}
    </div>
  );
}
