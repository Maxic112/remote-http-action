import { CheckCircle2Icon, DownloadIcon, RefreshCwIcon, RotateCcwIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { PageHeader, SwitchField, TextField } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { queryKeys, useApiMutation, useUpdateStatus } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { notifyError } from "@/lib/notify";
import type { UpdateStatus } from "@/lib/types";

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <span className="text-muted-foreground">{label}</span>
      <span>{children}</span>
    </>
  );
}

function formatSize(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function Progress({ downloaded, total }: { downloaded: number; total: number | null }) {
  const { t } = useTranslation();
  const percent = total ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
  return (
    <div className="grid gap-1.5" data-testid="update-progress">
      <div className="h-2 overflow-hidden rounded-full bg-muted">
        <div className="h-full bg-primary transition-all" style={{ width: `${percent ?? 30}%` }} />
      </div>
      <span className="text-xs text-muted-foreground">
        {t("update.downloading")} {formatSize(downloaded)}
        {total ? ` / ${formatSize(total)}` : ""}
      </span>
    </div>
  );
}

/** Download and install actions for the newer version, depending on the installation type. */
function UpdateActions({ status }: { status: UpdateStatus }) {
  const { t } = useTranslation();
  const download = useApiMutation<UpdateStatus>([queryKeys.update]);
  const install = useApiMutation<unknown>([queryKeys.update]);
  const phase = status.phase;
  const msi = status.install_kind === "msi";

  if (status.install_kind === "development") {
    return <p className="text-sm text-muted-foreground">{t("update.devBuild")}</p>;
  }
  if (!status.asset) {
    return <p className="text-sm text-destructive">{t("update.noAsset")}</p>;
  }
  if (phase.state === "installing") {
    return <p className="text-sm text-muted-foreground">{t(msi ? "update.installingMsi" : "update.installingPortable")}</p>;
  }
  if (phase.state === "downloading") {
    return <Progress downloaded={phase.downloaded} total={phase.total} />;
  }

  const runInstall = () => install.mutate({ op: "update_install" }, { onError: notifyError });
  if (phase.state === "ready") {
    return (
      <div className="grid gap-2">
        <p className="text-sm text-muted-foreground">{t(msi ? "update.readyMsi" : "update.readyPortable")}</p>
        <div>
          <Button onClick={runInstall} disabled={install.isPending}>
            <RotateCcwIcon />
            {t(msi ? "update.install" : "update.restart")}
          </Button>
        </div>
      </div>
    );
  }
  return (
    <div className="grid gap-2">
      <p className="text-sm text-muted-foreground">
        {t(msi ? "update.hintMsi" : "update.hintPortable", { size: formatSize(status.asset.size) })}
      </p>
      <div>
        <Button
          disabled={download.isPending}
          onClick={() =>
            download.mutate(
              { op: "update_download" },
              {
                // The installer wizard follows the download at once; the portable version waits for a restart.
                onSuccess: () => msi && runInstall(),
                onError: notifyError,
              },
            )
          }
        >
          <DownloadIcon />
          {t(msi ? "update.downloadAndInstall" : "update.download")}
        </Button>
      </div>
    </div>
  );
}

export function UpdatePage() {
  const { t } = useTranslation();
  const { data: status } = useUpdateStatus();
  const check = useApiMutation<UpdateStatus>([queryKeys.update]);
  const setSetting = useApiMutation<unknown>([queryKeys.update, queryKeys.settings]);
  const [feed, setFeed] = useState<string | null>(null);

  if (!status) return <p className="text-sm text-muted-foreground">{t("common.loading")}</p>;
  const checking = check.isPending || status.phase.state === "checking";
  const latest = status.latest;

  return (
    <div>
      <PageHeader
        title={t("update.title")}
        subtitle={t("update.subtitle")}
        action={
          <Button variant="outline" disabled={checking} onClick={() => check.mutate({ op: "update_check" }, { onError: notifyError })}>
            <RefreshCwIcon className={checking ? "animate-spin" : undefined} />
            {t("update.checkNow")}
          </Button>
        }
      />
      <div className="grid gap-6">
        <Card>
          <CardHeader>
            <CardTitle>{t("update.current")}</CardTitle>
          </CardHeader>
          <CardContent className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
            <Row label={t("update.version")}>
              <span data-testid="current-version">{status.current_version}</span>
            </Row>
            <Row label={t("update.kind")}>{t(`update.kinds.${status.install_kind}`)}</Row>
            <Row label={t("update.lastCheck")}>{status.last_check_at ? formatDateTime(status.last_check_at) : t("update.never")}</Row>
          </CardContent>
        </Card>

        {status.error && <p className="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{status.error}</p>}

        {status.available && latest ? (
          <Card className="border-orange-300">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <span className="size-2.5 rounded-full bg-orange-500" />
                {t("update.available", { version: latest.version })}
              </CardTitle>
            </CardHeader>
            <CardContent className="grid gap-4">
              <div className="text-sm text-muted-foreground">
                {latest.name}
                {latest.published_at && ` · ${formatDateTime(latest.published_at)}`}
              </div>
              {latest.notes && (
                <pre
                  className="max-h-64 overflow-auto rounded-md bg-muted p-3 font-sans text-sm whitespace-pre-wrap"
                  data-testid="release-notes"
                >
                  {latest.notes}
                </pre>
              )}
              {latest.html_url && <span className="text-xs break-all text-muted-foreground">{latest.html_url}</span>}
              <UpdateActions status={status} />
            </CardContent>
          </Card>
        ) : (
          status.last_check_at &&
          !status.error && (
            <div className="flex items-center gap-2 text-sm text-emerald-700" data-testid="up-to-date">
              <CheckCircle2Icon className="size-4" />
              {t("update.upToDate")}
            </div>
          )
        )}

        <Card>
          <CardHeader>
            <CardTitle>{t("update.settings")}</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4">
            <SwitchField
              label={t("update.autoCheck")}
              checked={status.auto_check}
              onChange={(v) => setSetting.mutate({ op: "settings_set", path: "updates.auto_check", value: v }, { onError: notifyError })}
            />
            <div className="flex items-end gap-2">
              <div className="flex-1">
                <TextField
                  label={t("update.feedUrl")}
                  hint={t("update.feedUrlHint")}
                  className="font-mono text-xs"
                  value={feed ?? status.feed_url}
                  onChange={setFeed}
                />
              </div>
              {feed !== null && feed !== status.feed_url && (
                <Button
                  className="mb-5"
                  onClick={() =>
                    setSetting.mutate(
                      { op: "settings_set", path: "updates.feed_url", value: feed },
                      { onSuccess: () => setFeed(null), onError: notifyError },
                    )
                  }
                >
                  {t("common.save")}
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
