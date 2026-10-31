import { MoreHorizontalIcon, PlayIcon, PlusIcon, ShieldIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmDialog, EmptyState, PageHeader } from "@/components/common";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { queryKeys, useApiMutation, useFlows, useSettings } from "@/lib/api";
import { flowUrl, hasAdminSteps } from "@/lib/flow";
import { notifyError, notifySuccess } from "@/lib/notify";
import type { Flow, Run } from "@/lib/types";

import { FlowEditor } from "./FlowEditor";

export function FlowsPage() {
  const { t } = useTranslation();
  const flows = useFlows();
  const settings = useSettings();
  const [editing, setEditing] = useState<Flow | null | undefined>(undefined);
  const [deleting, setDeleting] = useState<Flow | null>(null);
  const mutate = useApiMutation<unknown>([queryKeys.flows, queryKeys.groups]);
  const run = useApiMutation<Run>([queryKeys.runs]);
  const prefix = settings.data?.server.flow_prefix ?? "/flow/";

  return (
    <div>
      <PageHeader
        title={t("flows.title")}
        subtitle={t("flows.subtitle")}
        action={
          <Button onClick={() => setEditing(null)}>
            <PlusIcon />
            {t("flows.create")}
          </Button>
        }
      />
      {flows.data && flows.data.length === 0 ? (
        <EmptyState />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("flows.path")}</TableHead>
              <TableHead>{t("common.actions")}</TableHead>
              <TableHead>{t("flows.mode")}</TableHead>
              <TableHead className="w-16">{t("common.enabled")}</TableHead>
              <TableHead className="w-20" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {(flows.data ?? []).map((f) => (
              <TableRow key={f.id}>
                <TableCell>
                  <div className="font-medium">{f.name}</div>
                  {f.description && <div className="max-w-xs truncate text-xs text-muted-foreground">{f.description}</div>}
                </TableCell>
                <TableCell className="font-mono text-xs">POST {flowUrl(prefix, f.path)}</TableCell>
                <TableCell>
                  <div className="flex items-center gap-1.5">
                    <span className="text-sm text-muted-foreground">{t("flows.steps", { count: f.actions.length })}</span>
                    {hasAdminSteps(f) && (
                      <Tooltip>
                        <TooltipTrigger render={<ShieldIcon className="size-4 text-amber-600" data-testid="flow-admin" />} />
                        <TooltipContent>{t("flows.adminSteps")}</TooltipContent>
                      </Tooltip>
                    )}
                  </div>
                </TableCell>
                <TableCell>
                  <Badge variant="outline">{t(`flows.modeShort.${f.response_mode}`)}</Badge>
                </TableCell>
                <TableCell>
                  <Switch
                    checked={f.enabled}
                    aria-label={t("common.enabled")}
                    onCheckedChange={(enabled) =>
                      mutate.mutate({ op: "flow_set_enabled", flow: String(f.id), enabled }, { onError: notifyError })
                    }
                  />
                </TableCell>
                <TableCell>
                  <div className="flex justify-end gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("flows.runNow")}
                      title={t("flows.runNow")}
                      disabled={!f.enabled}
                      onClick={() =>
                        run.mutate(
                          { op: "flow_run", flow: String(f.id), wait: false },
                          { onSuccess: () => notifySuccess(t("flows.started")), onError: notifyError },
                        )
                      }
                    >
                      <PlayIcon />
                    </Button>
                    <DropdownMenu>
                      <DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" aria-label={t("common.actions")} />}>
                        <MoreHorizontalIcon />
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => setEditing(f)}>{t("common.edit")}</DropdownMenuItem>
                        <DropdownMenuItem variant="destructive" onClick={() => setDeleting(f)}>
                          {t("common.delete")}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {editing !== undefined && <FlowEditor flow={editing} onClose={() => setEditing(undefined)} />}
      <ConfirmDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        title={t("common.confirmDelete", { name: deleting?.name })}
        description={t("common.confirmDeleteText")}
        confirmLabel={t("common.delete")}
        destructive
        onConfirm={() => deleting && mutate.mutate({ op: "flow_delete", flow: String(deleting.id) }, { onError: notifyError })}
      />
    </div>
  );
}
