import { PlusIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { CheckboxField, NumberField, SelectField, SwitchField, TextField } from "@/components/common";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { queryKeys, useApiMutation, useSettings, useSystemInfo } from "@/lib/api";
import { ACTION_TYPES, emptyFlow, flowUrl, moveItem, newAction, slugify, toInput } from "@/lib/flow";
import { notifyError } from "@/lib/notify";
import type { Flow, FlowInput } from "@/lib/types";

import { ActionEditor } from "./ActionEditor";

export function FlowEditor({ flow, onClose }: { flow: Flow | null; onClose: () => void }) {
  const { t } = useTranslation();
  const system = useSystemInfo();
  const settings = useSettings();
  const [form, setForm] = useState<FlowInput>(() => (flow ? toInput(flow) : emptyFlow()));
  const [pathTouched, setPathTouched] = useState(!!flow);
  const save = useApiMutation<Flow>([queryKeys.flows, queryKeys.groups]);

  const set = <K extends keyof FlowInput>(key: K, value: FlowInput[K]) => setForm((f) => ({ ...f, [key]: value }));
  const setMeta = <K extends keyof FlowInput["client_meta"]>(key: K, value: FlowInput["client_meta"][K]) =>
    setForm((f) => ({ ...f, client_meta: { ...f.client_meta, [key]: value } }));
  const shells = system.data?.shells ?? [];
  const prefix = settings.data?.server.flow_prefix ?? "/flow/";

  const submit = () => {
    save.mutate(flow ? { op: "flow_update", id: flow.id, flow: form } : { op: "flow_create", flow: form }, {
      onSuccess: onClose,
      onError: notifyError,
    });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="flex max-h-[90vh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{flow ? t("flows.editTitle") : t("flows.newTitle")}</DialogTitle>
        </DialogHeader>
        <Tabs defaultValue="general" className="min-h-0 flex-1">
          <TabsList>
            <TabsTrigger value="general">{t("flows.tabs.general")}</TabsTrigger>
            <TabsTrigger value="actions">
              {t("flows.tabs.actions")} ({form.actions.length})
            </TabsTrigger>
            <TabsTrigger value="execution">{t("flows.tabs.execution")}</TabsTrigger>
            <TabsTrigger value="client">{t("flows.tabs.client")}</TabsTrigger>
          </TabsList>

          <div className="min-h-0 overflow-y-auto pt-4 pr-1" style={{ maxHeight: "60vh" }}>
            <TabsContent value="general" className="grid gap-4">
              <TextField
                label={t("common.name")}
                value={form.name}
                onChange={(name) => setForm((f) => ({ ...f, name, path: pathTouched ? f.path : slugify(name) }))}
              />
              <TextField
                label={t("flows.path")}
                value={form.path}
                className="font-mono"
                hint={t("flows.pathHint", { url: flowUrl(prefix, form.path) })}
                onChange={(path) => {
                  setPathTouched(true);
                  set("path", path);
                }}
              />
              <TextField label={t("common.description")} value={form.description} onChange={(v) => set("description", v)} />
              <SwitchField label={t("common.enabled")} checked={form.enabled} onChange={(v) => set("enabled", v)} />
            </TabsContent>

            <TabsContent value="actions" className="grid gap-3">
              {form.actions.length === 0 && <p className="text-sm text-muted-foreground">{t("flows.noActions")}</p>}
              {form.actions.map((a, i) => (
                <ActionEditor
                  key={i}
                  action={a}
                  index={i}
                  count={form.actions.length}
                  shells={shells}
                  onChange={(next) =>
                    set(
                      "actions",
                      form.actions.map((x, j) => (j === i ? next : x)),
                    )
                  }
                  onMove={(to) => set("actions", moveItem(form.actions, i, to))}
                  onRemove={() =>
                    set(
                      "actions",
                      form.actions.filter((_, j) => j !== i),
                    )
                  }
                />
              ))}
              <div>
                <DropdownMenu>
                  <DropdownMenuTrigger render={<Button variant="outline" />}>
                    <PlusIcon />
                    {t("flows.addAction")}
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    {ACTION_TYPES.map((type) => (
                      <DropdownMenuItem key={type} onClick={() => set("actions", [...form.actions, newAction(type, shells[0]?.id)])}>
                        {t(`actions.types.${type}`)}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </TabsContent>

            <TabsContent value="execution" className="grid gap-4">
              <SelectField
                label={t("flows.responseMode")}
                value={form.response_mode}
                options={[
                  { value: "sync", label: t("flows.modes.sync") },
                  { value: "async", label: t("flows.modes.async") },
                ]}
                onChange={(v) => set("response_mode", v as FlowInput["response_mode"])}
              />
              <SelectField
                label={t("flows.concurrency")}
                value={form.concurrency}
                options={(["reject_if_running", "queue", "allow"] as const).map((c) => ({
                  value: c,
                  label: t(`flows.concurrencyModes.${c}`),
                }))}
                onChange={(v) => set("concurrency", v as FlowInput["concurrency"])}
              />
              <div className="grid grid-cols-3 gap-3">
                <NumberField label={t("flows.timeout")} min={1} value={form.timeout_sec} onChange={(v) => set("timeout_sec", v ?? 1)} />
                <NumberField label={t("flows.cooldown")} value={form.cooldown_sec} onChange={(v) => set("cooldown_sec", v ?? 0)} />
                <NumberField label={t("flows.startDelay")} value={form.start_delay_sec} onChange={(v) => set("start_delay_sec", v ?? 0)} />
              </div>
              <p className="text-xs text-muted-foreground">{t("flows.startDelayHint")}</p>
              <CheckboxField label={t("flows.cancelable")} checked={form.cancelable} onChange={(v) => set("cancelable", v)} />
            </TabsContent>

            <TabsContent value="client" className="grid gap-4">
              <p className="text-xs text-muted-foreground">{t("flows.clientHint")}</p>
              <div className="grid grid-cols-3 gap-3">
                <TextField label={t("flows.clientIcon")} value={form.client_meta.icon ?? ""} onChange={(v) => setMeta("icon", v || null)} />
                <TextField
                  label={t("flows.clientColor")}
                  value={form.client_meta.color ?? ""}
                  onChange={(v) => setMeta("color", v || null)}
                />
                <TextField
                  label={t("flows.clientCategory")}
                  value={form.client_meta.category ?? ""}
                  onChange={(v) => setMeta("category", v || null)}
                />
              </div>
              <TextField
                label={t("flows.clientTags")}
                value={form.client_meta.tags.join(", ")}
                onChange={(v) =>
                  setMeta(
                    "tags",
                    v
                      .split(",")
                      .map((s) => s.trim())
                      .filter(Boolean),
                  )
                }
              />
              <CheckboxField
                label={t("flows.clientConfirm")}
                checked={form.client_meta.confirm_on_client}
                onChange={(v) => setMeta("confirm_on_client", v)}
              />
            </TabsContent>
          </div>
        </Tabs>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={submit} disabled={!form.name.trim() || form.actions.length === 0 || save.isPending}>
            {flow ? t("common.save") : t("common.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
