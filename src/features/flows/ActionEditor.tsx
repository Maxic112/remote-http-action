import { ArrowDownIcon, ArrowUpIcon, FolderOpenIcon, ShieldIcon, Trash2Icon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { CheckboxField, Field, NumberField, SelectField, TextField } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { linesToList, listToLines } from "@/lib/flow";
import { notifyError } from "@/lib/notify";
import { type FileFilter, pickFile } from "@/lib/pickFile";
import type { ActionDef, ShellInfo } from "@/lib/types";

const SCRIPT_EXTENSIONS: Record<string, string[]> = {
  cmd: ["bat", "cmd"],
  powershell: ["ps1"],
  pwsh: ["ps1"],
  wsl: ["sh"],
  sh: ["sh"],
};

/** Text input with a "Browse…" button that opens the system file picker. */
function PathField({
  label,
  value,
  onChange,
  filters,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  filters: FileFilter[];
}) {
  const { t } = useTranslation();
  return (
    <Field label={label}>
      <div className="flex gap-2">
        <Input aria-label={label} className="font-mono text-xs" value={value} onChange={(e) => onChange(e.target.value)} />
        <Button
          variant="outline"
          onClick={() =>
            pickFile(filters)
              .then((path) => path && onChange(path))
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

function ArgsField({ value, onChange }: { value: unknown; onChange: (v: string[]) => void }) {
  const { t } = useTranslation();
  const list = Array.isArray(value) ? (value as string[]) : [];
  return (
    <Field label={t("actions.args")} hint={t("actions.argsHint")}>
      <Textarea
        aria-label={t("actions.args")}
        className="font-mono text-xs"
        rows={2}
        defaultValue={listToLines(list)}
        onChange={(e) => onChange(linesToList(e.target.value))}
      />
    </Field>
  );
}

type WaitMode = "none" | "process" | "tree" | "name";

/** Older flows store `wait` as a boolean. */
function waitMode(v: unknown): WaitMode {
  if (v === true) return "process";
  if (v === "process" || v === "tree" || v === "name") return v;
  return "none";
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? "";

interface Props {
  action: ActionDef;
  index: number;
  count: number;
  shells: ShellInfo[];
  onChange: (a: ActionDef) => void;
  onMove: (to: number) => void;
  onRemove: () => void;
}

export function ActionEditor({ action, index, count, shells, onChange, onMove, onRemove }: Props) {
  const { t } = useTranslation();
  const p = action.params;
  const setParam = (key: string, value: unknown) => onChange({ ...action, params: { ...p, [key]: value } });

  const shellOptions = shells.length
    ? shells.map((s) => ({ value: s.id, label: s.name }))
    : [{ value: String(p.shell ?? "cmd"), label: String(p.shell ?? "cmd") }];

  return (
    <div className="rounded-lg border p-4" data-testid={`action-${index}`}>
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm font-medium">
          <span className="flex size-6 items-center justify-center rounded-full bg-muted text-xs">{index + 1}</span>
          {t(`actions.types.${action.type}`)}
          {action.run_as_admin && (
            <ShieldIcon className="size-4 text-amber-600" aria-label={t("actions.runAsAdmin")} data-testid="admin-shield" />
          )}
        </div>
        <div className="flex gap-1">
          <Button variant="ghost" size="icon-sm" disabled={index === 0} onClick={() => onMove(index - 1)} aria-label={t("actions.moveUp")}>
            <ArrowUpIcon />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={index === count - 1}
            onClick={() => onMove(index + 1)}
            aria-label={t("actions.moveDown")}
          >
            <ArrowDownIcon />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={onRemove} aria-label={t("actions.remove")}>
            <Trash2Icon />
          </Button>
        </div>
      </div>

      <div className="grid gap-3">
        {action.type === "delay" && (
          <NumberField label={t("actions.seconds")} value={Number(p.seconds ?? 0)} onChange={(v) => setParam("seconds", v ?? 0)} />
        )}
        {(action.type === "shutdown" || action.type === "reboot") && (
          <CheckboxField label={t("actions.force")} checked={Boolean(p.force)} onChange={(v) => setParam("force", v)} />
        )}
        {action.type === "sleep" && (
          <CheckboxField label={t("actions.hibernate")} checked={Boolean(p.hibernate)} onChange={(v) => setParam("hibernate", v)} />
        )}
        {action.type === "cli_command" && (
          <>
            <SelectField
              label={t("actions.shell")}
              value={String(p.shell ?? "cmd")}
              options={shellOptions}
              onChange={(v) => setParam("shell", v)}
            />
            <Field label={t("actions.command")}>
              <Textarea
                aria-label={t("actions.command")}
                className="font-mono text-xs"
                rows={3}
                value={String(p.command ?? "")}
                onChange={(e) => setParam("command", e.target.value)}
              />
            </Field>
            <TextField
              label={t("actions.workingDir")}
              value={String(p.working_dir ?? "")}
              onChange={(v) => setParam("working_dir", v || null)}
            />
          </>
        )}
        {action.type === "run_script" && (
          <>
            <SelectField
              label={t("actions.shell")}
              value={String(p.shell ?? "cmd")}
              options={shellOptions}
              onChange={(v) => setParam("shell", v)}
            />
            <PathField
              label={t("actions.scriptPath")}
              value={String(p.script_path ?? "")}
              onChange={(v) => setParam("script_path", v)}
              filters={[
                { name: t("actions.scriptFiles"), extensions: SCRIPT_EXTENSIONS[String(p.shell)] ?? ["*"] },
                { name: t("actions.allFiles"), extensions: ["*"] },
              ]}
            />
            <ArgsField value={p.args} onChange={(v) => setParam("args", v)} />
            <TextField
              label={t("actions.workingDir")}
              hint={t("actions.workingDirScriptHint")}
              value={String(p.working_dir ?? "")}
              onChange={(v) => setParam("working_dir", v || null)}
            />
          </>
        )}
        {action.type === "launch_app" && (
          <>
            <PathField
              label={t("actions.appPath")}
              value={String(p.path ?? "")}
              onChange={(v) => setParam("path", v)}
              filters={[{ name: t("actions.programs"), extensions: ["exe", "bat", "cmd"] }]}
            />
            <ArgsField value={p.args} onChange={(v) => setParam("args", v)} />
            <TextField
              label={t("actions.workingDir")}
              hint={t("actions.workingDirAppHint")}
              value={String(p.working_dir ?? "")}
              onChange={(v) => setParam("working_dir", v || null)}
            />
            <SelectField
              label={t("actions.wait")}
              value={waitMode(p.wait)}
              options={(["none", "process", "tree", "name"] as const).map((m) => ({ value: m, label: t(`actions.waitModes.${m}`) }))}
              onChange={(v) => setParam("wait", v)}
              hint={t(`actions.waitHints.${waitMode(p.wait)}`)}
            />
            {waitMode(p.wait) === "name" && (
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label={t("actions.waitProcessName")}
                  placeholder={fileName(String(p.path ?? ""))}
                  value={String(p.wait_process_name ?? "")}
                  onChange={(v) => setParam("wait_process_name", v || null)}
                />
                <NumberField
                  label={t("actions.waitStartTimeout")}
                  min={1}
                  nullable
                  placeholder="60"
                  value={(p.wait_start_timeout_sec as number | null | undefined) ?? null}
                  onChange={(v) => setParam("wait_start_timeout_sec", v)}
                />
              </div>
            )}
          </>
        )}

        <div className="grid grid-cols-2 gap-3">
          <SelectField
            label={t("actions.onError")}
            value={action.on_error}
            options={[
              { value: "stop", label: t("actions.onErrorModes.stop") },
              { value: "continue", label: t("actions.onErrorModes.continue") },
            ]}
            onChange={(v) => onChange({ ...action, on_error: v as ActionDef["on_error"] })}
          />
          <NumberField
            label={t("actions.timeout")}
            value={action.timeout_sec}
            min={1}
            nullable
            placeholder={t("actions.timeoutPlaceholder")}
            onChange={(v) => onChange({ ...action, timeout_sec: v })}
          />
        </div>
        <div className="flex flex-wrap gap-x-6 gap-y-2">
          <CheckboxField
            label={t("actions.runAsAdmin")}
            icon={<ShieldIcon className="size-4 text-amber-600" />}
            checked={action.run_as_admin}
            onChange={(v) => onChange({ ...action, run_as_admin: v })}
          />
          {(action.type === "cli_command" || action.type === "run_script") && (
            <CheckboxField
              label={t("actions.captureOutput")}
              checked={action.capture_output}
              onChange={(v) => onChange({ ...action, capture_output: v })}
            />
          )}
        </div>
      </div>
    </div>
  );
}
