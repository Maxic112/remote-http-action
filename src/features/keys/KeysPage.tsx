import { CopyIcon, MoreHorizontalIcon, PlusIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { CheckList, ConfirmDialog, EmptyState, Field, PageHeader, TextField } from "@/components/common";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { queryKeys, useApiMutation, useGroups, useKeys } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { notifyError } from "@/lib/notify";
import type { ApiKey, Id, IssuedKey } from "@/lib/types";

const invalidate = [queryKeys.keys, queryKeys.groups];

function KeyDialog({
  apiKey,
  open,
  onOpenChange,
  onIssued,
}: {
  apiKey: ApiKey | null;
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onIssued: (k: IssuedKey) => void;
}) {
  const { t } = useTranslation();
  const groups = useGroups();
  const [name, setName] = useState(apiKey?.name ?? "");
  const [groupIds, setGroupIds] = useState<Id[]>(apiKey?.group_ids ?? []);
  const create = useApiMutation<IssuedKey>(invalidate);
  const update = useApiMutation<ApiKey>(invalidate);

  const save = () => {
    if (apiKey) {
      update.mutate(
        { op: "key_update", id: apiKey.id, name, group_ids: groupIds },
        { onSuccess: () => onOpenChange(false), onError: notifyError },
      );
    } else {
      create.mutate(
        { op: "key_create", name, group_ids: groupIds },
        {
          onSuccess: (issued) => {
            onOpenChange(false);
            onIssued(issued);
          },
          onError: notifyError,
        },
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{apiKey ? t("keys.editTitle") : t("keys.create")}</DialogTitle>
        </DialogHeader>
        <div className="grid gap-4">
          <TextField label={t("common.name")} value={name} onChange={setName} />
          <Field label={t("keys.groups")}>
            <CheckList
              items={(groups.data ?? []).map((g) => ({ id: g.id, label: g.name }))}
              selected={groupIds}
              onChange={setGroupIds}
              empty={t("keys.noGroups")}
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={save} disabled={!name.trim() || create.isPending || update.isPending}>
            {apiKey ? t("common.save") : t("common.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function SecretDialog({ issued, onClose }: { issued: IssuedKey | null; onClose: () => void }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  if (!issued) return null;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("keys.secretTitle", { name: issued.key.name })}</DialogTitle>
        </DialogHeader>
        <p className="rounded-md bg-amber-50 p-3 text-sm text-amber-900">{t("keys.secretWarning")}</p>
        <div className="flex items-center gap-2">
          <code className="flex-1 rounded-md border bg-muted px-3 py-2 font-mono text-sm break-all" data-testid="secret">
            {issued.secret}
          </code>
          <Button
            variant="outline"
            onClick={() => {
              void navigator.clipboard?.writeText(issued.secret);
              setCopied(true);
            }}
          >
            <CopyIcon />
            {copied ? t("common.copied") : t("common.copy")}
          </Button>
        </div>
        <DialogFooter>
          <Button onClick={onClose}>{t("common.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function KeysPage() {
  const { t } = useTranslation();
  const keys = useKeys();
  const groups = useGroups();
  const [editing, setEditing] = useState<ApiKey | null | undefined>(undefined);
  const [issued, setIssued] = useState<IssuedKey | null>(null);
  const [rotating, setRotating] = useState<ApiKey | null>(null);
  const [deleting, setDeleting] = useState<ApiKey | null>(null);
  const mutate = useApiMutation<unknown>(invalidate);
  const rotate = useApiMutation<IssuedKey>(invalidate);

  const groupName = (id: Id) => groups.data?.find((g) => g.id === id)?.name ?? `#${id}`;

  return (
    <div>
      <PageHeader
        title={t("keys.title")}
        subtitle={t("keys.subtitle")}
        action={
          <Button onClick={() => setEditing(null)}>
            <PlusIcon />
            {t("keys.create")}
          </Button>
        }
      />
      {keys.data && keys.data.length === 0 ? (
        <EmptyState />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("keys.prefix")}</TableHead>
              <TableHead>{t("keys.groups")}</TableHead>
              <TableHead>{t("keys.lastUsed")}</TableHead>
              <TableHead className="w-16">{t("common.enabled")}</TableHead>
              <TableHead className="w-10" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {(keys.data ?? []).map((k) => (
              <TableRow key={k.id}>
                <TableCell className="font-medium">{k.name}</TableCell>
                <TableCell className="font-mono text-xs text-muted-foreground">{k.key_prefix}…</TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {k.group_ids.map((g) => (
                      <Badge key={g} variant="secondary">
                        {groupName(g)}
                      </Badge>
                    ))}
                  </div>
                </TableCell>
                <TableCell className="text-muted-foreground">{k.last_used_at ? formatDateTime(k.last_used_at) : t("keys.never")}</TableCell>
                <TableCell>
                  <Switch
                    checked={k.enabled}
                    aria-label={t("common.enabled")}
                    onCheckedChange={(enabled) => mutate.mutate({ op: "key_update", id: k.id, enabled }, { onError: notifyError })}
                  />
                </TableCell>
                <TableCell>
                  <DropdownMenu>
                    <DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" aria-label={t("common.actions")} />}>
                      <MoreHorizontalIcon />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={() => setEditing(k)}>{t("common.edit")}</DropdownMenuItem>
                      <DropdownMenuItem onClick={() => setRotating(k)}>{t("keys.rotate")}</DropdownMenuItem>
                      <DropdownMenuItem variant="destructive" onClick={() => setDeleting(k)}>
                        {t("common.delete")}
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {editing !== undefined && <KeyDialog apiKey={editing} open onOpenChange={(o) => !o && setEditing(undefined)} onIssued={setIssued} />}
      <SecretDialog issued={issued} onClose={() => setIssued(null)} />
      <ConfirmDialog
        open={!!rotating}
        onOpenChange={(o) => !o && setRotating(null)}
        title={t("keys.rotateTitle", { name: rotating?.name })}
        description={t("keys.rotateText")}
        confirmLabel={t("keys.rotate")}
        onConfirm={() => rotating && rotate.mutate({ op: "key_rotate", id: rotating.id }, { onSuccess: setIssued, onError: notifyError })}
      />
      <ConfirmDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        title={t("common.confirmDelete", { name: deleting?.name })}
        description={t("common.confirmDeleteText")}
        confirmLabel={t("common.delete")}
        destructive
        onConfirm={() => deleting && mutate.mutate({ op: "key_delete", id: deleting.id }, { onError: notifyError })}
      />
    </div>
  );
}
