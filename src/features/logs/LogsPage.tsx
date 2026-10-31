import { useQuery } from "@tanstack/react-query";
import { FolderOpenIcon, RefreshCwIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmDialog, EmptyState, PageHeader } from "@/components/common";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { call, queryKeys, useApiMutation, useLogsInfo } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { notifyError } from "@/lib/notify";
import type { LogCategory, LogEntry } from "@/lib/types";

const LIMIT = 300;

function str(v: unknown): string {
  if (v === null || v === undefined) return "—";
  return typeof v === "string" ? v : JSON.stringify(v);
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** Columns shown per category; the full entry is available in the details dialog. */
function columns(category: LogCategory, t: (k: string) => string): { title: string; value: (e: LogEntry) => React.ReactNode }[] {
  switch (category) {
    case "http":
      return [
        { title: t("logs.columns.ip"), value: (e) => str(e.ip) },
        { title: t("logs.columns.request"), value: (e) => <span className="font-mono text-xs">{`${str(e.method)} ${str(e.path)}`}</span> },
        {
          title: t("logs.columns.status"),
          value: (e) => <Badge variant={Number(e.status) >= 400 ? "destructive" : "secondary"}>{str(e.status)}</Badge>,
        },
        { title: t("logs.columns.key"), value: (e) => str(e.key) },
        { title: t("logs.columns.duration"), value: (e) => `${str(e.duration_ms)} ms` },
      ];
    case "cli":
      return [
        { title: t("logs.columns.command"), value: (e) => <span className="font-mono text-xs">{str(e.op)}</span> },
        {
          title: t("logs.columns.result"),
          value: (e) =>
            e.ok ? <Badge variant="secondary">OK</Badge> : <Badge variant="destructive">{str((e.error as { code?: string })?.code)}</Badge>,
        },
        { title: t("logs.columns.duration"), value: (e) => `${str(e.duration_ms)} ms` },
      ];
    case "flow":
      return [
        { title: t("logs.columns.event"), value: (e) => str(e.event) },
        { title: t("logs.columns.flow"), value: (e) => <span className="font-mono text-xs">{str(e.flow)}</span> },
        { title: t("logs.columns.source"), value: (e) => [str(e.source), e.key ? str(e.key) : null].filter(Boolean).join(" · ") },
        {
          title: t("logs.columns.status"),
          value: (e) => (e.status ? <Badge variant={e.status === "success" ? "secondary" : "destructive"}>{str(e.status)}</Badge> : "—"),
        },
        { title: t("logs.columns.error"), value: (e) => <span className="text-xs text-destructive">{e.error ? str(e.error) : ""}</span> },
      ];
  }
}

export function LogsPage() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<LogCategory>("flow");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<LogEntry | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const info = useLogsInfo();
  const clear = useApiMutation<unknown>([queryKeys.logsInfo, ["logs"]]);
  const entries = useQuery({
    queryKey: ["logs", category, query],
    queryFn: () => call<LogEntry[]>({ op: "logs_read", category, limit: LIMIT, query: query || undefined }),
    refetchInterval: 5000,
  });
  const catInfo = info.data?.categories.find((c) => c.category === category);
  const cols = columns(category, t);

  return (
    <div>
      <PageHeader
        title={t("logs.title")}
        subtitle={t("logs.subtitle")}
        action={
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => void entries.refetch()} aria-label={t("logs.refresh")}>
              <RefreshCwIcon />
            </Button>
            <Button variant="outline" onClick={() => call({ op: "logs_open_folder" }).catch(notifyError)}>
              <FolderOpenIcon />
              {t("logs.openFolder")}
            </Button>
            <Button variant="destructive" onClick={() => setConfirmClear(true)}>
              {t("logs.clear")}
            </Button>
          </div>
        }
      />
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <Tabs value={category} onValueChange={(v) => setCategory(v as LogCategory)}>
          <TabsList>
            {(["flow", "http", "cli"] as LogCategory[]).map((c) => (
              <TabsTrigger key={c} value={c}>
                {t(`logs.categories.${c}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
        <div className="flex items-center gap-3">
          {catInfo && (
            <span className="text-xs text-muted-foreground" data-testid="logs-size">
              {catInfo.enabled ? formatSize(catInfo.size_bytes) : t("logs.disabled")}
            </span>
          )}
          <Input
            className="w-64"
            placeholder={t("logs.search")}
            aria-label={t("logs.search")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      {entries.data && entries.data.length === 0 ? (
        <EmptyState>{t("logs.empty")}</EmptyState>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("logs.columns.time")}</TableHead>
              {cols.map((c) => (
                <TableHead key={c.title}>{c.title}</TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {(entries.data ?? []).map((e, i) => (
              <TableRow key={`${e.ts}-${i}`} className="cursor-pointer" onClick={() => setSelected(e)}>
                <TableCell className="whitespace-nowrap">{formatDateTime(e.ts)}</TableCell>
                {cols.map((c) => (
                  <TableCell key={c.title}>{c.value(e)}</TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {selected && (
        <Dialog open onOpenChange={(o) => !o && setSelected(null)}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
            <DialogHeader>
              <DialogTitle>{formatDateTime(selected.ts)}</DialogTitle>
            </DialogHeader>
            <pre className="overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap" data-testid="log-details">
              {JSON.stringify(selected, null, 2)}
            </pre>
          </DialogContent>
        </Dialog>
      )}
      <ConfirmDialog
        open={confirmClear}
        onOpenChange={setConfirmClear}
        title={t("logs.clearTitle", { category: t(`logs.categories.${category}`) })}
        description={t("common.confirmDeleteText")}
        confirmLabel={t("logs.clear")}
        destructive
        onConfirm={() => clear.mutate({ op: "logs_clear", category }, { onError: notifyError })}
      />
    </div>
  );
}
