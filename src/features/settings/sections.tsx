import { FolderOpenIcon, ShieldAlertIcon, ShieldCheckIcon } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Field, NumberField, SelectField, SwitchField } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { call, queryKeys, useApiMutation, useAutostart, useSystemInfo, useTlsInfo } from "@/lib/api";
import { notifyError, notifySuccess } from "@/lib/notify";
import { pickFile } from "@/lib/pickFile";
import type { LogCategory, Settings, TlsInfo } from "@/lib/types";

export function Section({ title, children, action }: { title: string; children: ReactNode; action?: ReactNode }) {
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

type Tls = Settings["server"]["tls"];

function PemField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const { t } = useTranslation();
  return (
    <Field label={label}>
      <div className="flex gap-2">
        <Input aria-label={label} className="font-mono text-xs" value={value} onChange={(e) => onChange(e.target.value)} />
        <Button
          variant="outline"
          onClick={() =>
            pickFile([{ name: "PEM", extensions: ["pem", "crt", "cer", "key"] }])
              .then((p) => p && onChange(p))
              .catch(notifyError)
          }
        >
          <FolderOpenIcon />
          {t("actions.browse")}
        </Button>
      </div>
    </Field>
  );
}

export function TlsSection({ tls, onChange, saved }: { tls: Tls; onChange: (tls: Tls) => void; saved: Tls }) {
  const { t } = useTranslation();
  const info = useTlsInfo();
  const regenerate = useApiMutation<TlsInfo>([queryKeys.tls, queryKeys.server]);
  const set = <K extends keyof Tls>(k: K, v: Tls[K]) => onChange({ ...tls, [k]: v });

  return (
    <Section title={t("settings.tls")}>
      <SwitchField
        label={t("settings.tlsEnabled")}
        hint={t("settings.tlsHint")}
        checked={tls.enabled}
        onChange={(v) => set("enabled", v)}
      />
      {tls.enabled && (
        <>
          <SelectField
            label={t("settings.tlsMode")}
            value={tls.mode}
            options={[
              { value: "self_signed", label: t("settings.tlsModes.self_signed") },
              { value: "custom", label: t("settings.tlsModes.custom") },
            ]}
            onChange={(v) => set("mode", v as Tls["mode"])}
          />
          {tls.mode === "custom" && (
            <>
              <PemField label={t("settings.tlsCert")} value={tls.cert_path} onChange={(v) => set("cert_path", v)} />
              <PemField label={t("settings.tlsKey")} value={tls.key_path} onChange={(v) => set("key_path", v)} />
            </>
          )}
        </>
      )}
      {saved.enabled && info.data?.fingerprint && (
        <div className="grid gap-1 rounded-md bg-muted p-3 text-xs">
          <span className="text-muted-foreground">{t("settings.tlsFingerprint")}</span>
          <code className="font-mono break-all" data-testid="tls-fingerprint">
            {info.data.fingerprint}
          </code>
          {info.data.not_after && (
            <span className="text-muted-foreground">
              {t("settings.tlsValidUntil")}: {info.data.not_after}
            </span>
          )}
        </div>
      )}
      {saved.enabled && saved.mode === "self_signed" && (
        <div>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              regenerate.mutate(
                { op: "tls_regenerate" },
                { onSuccess: () => notifySuccess(t("settings.tlsRegenerated")), onError: notifyError },
              )
            }
          >
            {t("settings.tlsRegenerate")}
          </Button>
        </div>
      )}
    </Section>
  );
}

export function AutostartSection() {
  const { t } = useTranslation();
  const status = useAutostart();
  const system = useSystemInfo();
  const mutate = useApiMutation<unknown>([queryKeys.autostart]);
  const s = status.data;
  if (!s || !s.supported) return null;

  const state = !s.installed ? "off" : s.matches_current ? "on" : "stale";
  return (
    <Section title={t("settings.autostart")}>
      <p className="text-sm text-muted-foreground">{t("settings.autostartHint")}</p>
      <div className="flex items-center gap-2 text-sm" data-testid="autostart-state">
        {state === "on" ? <ShieldCheckIcon className="size-4 text-emerald-600" /> : <ShieldAlertIcon className="size-4 text-amber-600" />}
        {t(`settings.autostartStates.${state}`)}
      </div>
      {state === "stale" && s.command && <code className="font-mono text-xs break-all text-muted-foreground">{s.command}</code>}
      {!system.data?.elevated && !s.installed && <p className="text-xs text-amber-700">{t("settings.autostartNeedsAdmin")}</p>}
      <div className="flex gap-2">
        <Button size="sm" onClick={() => mutate.mutate({ op: "autostart_install" }, { onError: notifyError })}>
          {s.installed ? t("settings.autostartReinstall") : t("settings.autostartInstall")}
        </Button>
        {s.installed && (
          <Button variant="outline" size="sm" onClick={() => mutate.mutate({ op: "autostart_remove" }, { onError: notifyError })}>
            {t("settings.autostartRemove")}
          </Button>
        )}
      </div>
    </Section>
  );
}

type Logging = Settings["logging"];
const CATEGORIES: LogCategory[] = ["http", "cli", "flow"];

export function LoggingSection({ logging, onChange }: { logging: Logging; onChange: (l: Logging) => void }) {
  const { t } = useTranslation();
  return (
    <Section
      title={t("settings.logging")}
      action={
        <Button variant="outline" size="sm" onClick={() => call({ op: "logs_open_folder" }).catch(notifyError)}>
          <FolderOpenIcon />
          {t("logs.openFolder")}
        </Button>
      }
    >
      {CATEGORIES.map((c) => (
        <div key={c} className="grid gap-3 rounded-lg border p-3">
          <SwitchField
            label={t(`logs.categories.${c}`)}
            hint={t(`logs.categoryHints.${c}`)}
            checked={logging[c].enabled}
            onChange={(v) => onChange({ ...logging, [c]: { ...logging[c], enabled: v } })}
          />
          {logging[c].enabled && (
            <div className="grid grid-cols-2 gap-3">
              <NumberField
                label={t("settings.logMaxFileMb")}
                min={1}
                value={logging[c].max_file_mb}
                onChange={(v) => onChange({ ...logging, [c]: { ...logging[c], max_file_mb: v ?? 1 } })}
              />
              <NumberField
                label={t("settings.logMaxFiles")}
                min={1}
                value={logging[c].max_files}
                onChange={(v) => onChange({ ...logging, [c]: { ...logging[c], max_files: v ?? 1 } })}
              />
            </div>
          )}
        </div>
      ))}
      <NumberField
        label={t("settings.logOutputLimit")}
        min={0}
        value={logging.output_limit}
        onChange={(v) => onChange({ ...logging, output_limit: v ?? 0 })}
      />
    </Section>
  );
}
