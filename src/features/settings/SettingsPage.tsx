import type { ReactNode } from "react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { Field, NumberField, PageHeader, SelectField, SwitchField, TextField } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { queryKeys, useApiMutation, useBans, useServerStatus, useSettings, useSystemInfo } from "@/lib/api";
import { linesToList, listToLines } from "@/lib/flow";
import { notifyError, notifySuccess } from "@/lib/notify";
import type { Settings } from "@/lib/types";

function Section({ title, children, action }: { title: string; children: ReactNode; action?: ReactNode }) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>{title}</CardTitle>
        {action}
      </CardHeader>
      <CardContent className="grid gap-4">{children}</CardContent>
    </Card>
  );
}

function LinesField({ label, hint, value, onChange }: { label: string; hint: string; value: string[]; onChange: (v: string[]) => void }) {
  const id = useId();
  const [text, setText] = useState(listToLines(value));
  const [prev, setPrev] = useState(value);
  // Sync from outside (reset/refetch) without eating the newline the user is typing.
  if (prev !== value) {
    setPrev(value);
    if (listToLines(linesToList(text)) !== listToLines(value)) setText(listToLines(value));
  }
  return (
    <Field label={label} hint={hint} htmlFor={id}>
      <Textarea
        id={id}
        rows={3}
        className="font-mono text-xs"
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          onChange(linesToList(e.target.value));
        }}
      />
    </Field>
  );
}

export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useSettings();
  const system = useSystemInfo();
  const server = useServerStatus();
  const bans = useBans();
  const [form, setForm] = useState<Settings | null>(null);
  const [loaded, setLoaded] = useState<Settings | undefined>(undefined);
  const save = useApiMutation<Settings>([queryKeys.settings, queryKeys.server]);
  const control = useApiMutation<unknown>([queryKeys.settings, queryKeys.server]);
  const unban = useApiMutation<unknown>([queryKeys.bans]);

  if (settings.data && settings.data !== loaded) {
    setLoaded(settings.data);
    setForm(settings.data);
  }

  if (!form) return <p className="text-sm text-muted-foreground">{t("common.loading")}</p>;

  const dirty = JSON.stringify(form) !== JSON.stringify(settings.data);
  const setServer = <K extends keyof Settings["server"]>(k: K, v: Settings["server"][K]) =>
    setForm({ ...form, server: { ...form.server, [k]: v } });
  const setBf = <K extends keyof Settings["server"]["brute_force"]>(k: K, v: Settings["server"]["brute_force"][K]) =>
    setServer("brute_force", { ...form.server.brute_force, [k]: v });
  const setApp = <K extends keyof Settings["app"]>(k: K, v: Settings["app"][K]) => setForm({ ...form, app: { ...form.app, [k]: v } });

  const bindOptions = [
    { value: "0.0.0.0", label: t("settings.allInterfaces") },
    { value: "127.0.0.1", label: t("settings.localhost") },
    ...(system.data?.interfaces ?? []).filter((i) => !i.loopback).map((i) => ({ value: i.ip, label: `${i.ip} (${i.name})` })),
  ];
  if (!bindOptions.some((o) => o.value === form.server.bind_address)) {
    bindOptions.push({ value: form.server.bind_address, label: form.server.bind_address });
  }
  const running = server.data?.state === "running";

  return (
    <div className="pb-20">
      <PageHeader title={t("settings.title")} />
      <div className="grid gap-6">
        <Section
          title={t("settings.server")}
          action={
            <Button
              variant="outline"
              size="sm"
              onClick={() => control.mutate({ op: running ? "server_stop" : "server_start" }, { onError: notifyError })}
            >
              {running ? t("settings.stop") : t("settings.start")}
            </Button>
          }
        >
          {server.data?.error && <p className="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{server.data.error}</p>}
          <SwitchField label={t("settings.serverEnabled")} checked={form.server.enabled} onChange={(v) => setServer("enabled", v)} />
          <div className="grid grid-cols-3 gap-4">
            <div className="col-span-2">
              <SelectField
                label={t("settings.bindAddress")}
                value={form.server.bind_address}
                options={bindOptions}
                onChange={(v) => setServer("bind_address", v)}
              />
            </div>
            <NumberField label={t("settings.port")} min={1} value={form.server.port} onChange={(v) => setServer("port", v ?? 1)} />
          </div>
          <div className="grid grid-cols-3 gap-4">
            <TextField
              label={t("settings.flowPrefix")}
              className="font-mono"
              value={form.server.flow_prefix}
              onChange={(v) => setServer("flow_prefix", v)}
            />
            <TextField
              label={t("settings.allowedPath")}
              className="font-mono"
              value={form.server.allowed_path}
              onChange={(v) => setServer("allowed_path", v)}
            />
            <TextField
              label={t("settings.statusPath")}
              className="font-mono"
              value={form.server.status_path}
              onChange={(v) => setServer("status_path", v)}
            />
          </div>
          <SwitchField
            label={t("settings.publicStatus")}
            checked={form.server.public_status}
            onChange={(v) => setServer("public_status", v)}
          />
          <SwitchField
            label={t("settings.hideForbidden")}
            checked={form.server.hide_forbidden_flows}
            onChange={(v) => setServer("hide_forbidden_flows", v)}
          />
        </Section>

        <Section title={t("settings.network")}>
          <LinesField
            label={t("settings.allowedIps")}
            hint={t("settings.allowedIpsHint")}
            value={form.server.allowed_ips}
            onChange={(v) => setServer("allowed_ips", v)}
          />
          <LinesField
            label={t("settings.allowedHosts")}
            hint={t("settings.allowedHostsHint")}
            value={form.server.allowed_hosts}
            onChange={(v) => setServer("allowed_hosts", v)}
          />
          <LinesField
            label={t("settings.trustedProxies")}
            hint={t("settings.trustedProxiesHint")}
            value={form.server.trusted_proxies}
            onChange={(v) => setServer("trusted_proxies", v)}
          />
          <SwitchField label={t("settings.bruteForce")} checked={form.server.brute_force.enabled} onChange={(v) => setBf("enabled", v)} />
          {form.server.brute_force.enabled && (
            <div className="grid grid-cols-3 gap-4">
              <NumberField
                label={t("settings.maxFailures")}
                min={1}
                value={form.server.brute_force.max_failures}
                onChange={(v) => setBf("max_failures", v ?? 1)}
              />
              <NumberField
                label={t("settings.windowSec")}
                min={1}
                value={form.server.brute_force.window_sec}
                onChange={(v) => setBf("window_sec", v ?? 1)}
              />
              <NumberField
                label={t("settings.banSec")}
                value={form.server.brute_force.ban_sec}
                onChange={(v) => setBf("ban_sec", v ?? 0)}
              />
            </div>
          )}
          <Field label={t("settings.bans")}>
            {bans.data && bans.data.length > 0 ? (
              <div className="grid gap-2">
                {bans.data.map((b) => (
                  <div key={b.ip} className="flex items-center justify-between rounded-md border px-3 py-2 text-sm">
                    <span className="font-mono">{b.ip}</span>
                    <span className="text-muted-foreground">
                      {b.seconds_left} {t("common.seconds")}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => unban.mutate({ op: "ban_remove", ip: b.ip }, { onError: notifyError })}
                    >
                      {t("settings.unban")}
                    </Button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">{t("settings.noBans")}</p>
            )}
          </Field>
        </Section>

        <Section title={t("settings.cli")}>
          <SwitchField
            label={t("settings.cliEnabled")}
            hint={t("settings.cliHint")}
            checked={form.cli.enabled}
            onChange={(v) => setForm({ ...form, cli: { enabled: v } })}
          />
        </Section>

        <Section title={t("settings.app")}>
          <SwitchField label={t("settings.closeToTray")} checked={form.app.close_to_tray} onChange={(v) => setApp("close_to_tray", v)} />
          <div className="grid grid-cols-2 gap-4">
            <SelectField
              label={t("settings.language")}
              value={form.app.language}
              options={(["system", "ru", "en"] as const).map((l) => ({ value: l, label: t(`settings.languages.${l}`) }))}
              onChange={(v) => setApp("language", v as Settings["app"]["language"])}
            />
            <NumberField
              label={t("settings.historyLimit")}
              min={1}
              value={form.app.run_history_limit}
              onChange={(v) => setApp("run_history_limit", v ?? 1)}
            />
          </div>
        </Section>

        {system.data && (
          <Section title={t("settings.about")}>
            <div className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-sm">
              <span className="text-muted-foreground">{t("settings.version")}</span>
              <span>
                {system.data.version} ({system.data.build})
              </span>
              <span className="text-muted-foreground">{t("settings.dataDir")}</span>
              <span className="font-mono text-xs break-all">{system.data.paths.data_dir}</span>
            </div>
          </Section>
        )}
      </div>

      {dirty && (
        <div className="fixed right-6 bottom-6 flex items-center gap-3 rounded-lg border bg-background p-3 shadow-lg">
          <span className="text-sm text-muted-foreground">{t("settings.unsaved")}</span>
          <Button variant="outline" onClick={() => setForm(settings.data ?? null)}>
            {t("common.reset")}
          </Button>
          <Button
            onClick={() =>
              save.mutate(
                { op: "settings_update", settings: form },
                { onSuccess: () => notifySuccess(t("common.saved")), onError: notifyError },
              )
            }
            disabled={save.isPending}
          >
            {t("common.save")}
          </Button>
        </div>
      )}
    </div>
  );
}
