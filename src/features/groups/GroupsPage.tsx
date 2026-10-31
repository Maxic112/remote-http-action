import { MoreHorizontalIcon, PlusIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { CheckList, ConfirmDialog, EmptyState, Field, PageHeader, TextField } from "@/components/common";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { queryKeys, useApiMutation, useFlows, useGroups, useKeys } from "@/lib/api";
import { notifyError } from "@/lib/notify";
import type { AccessGroup, Id } from "@/lib/types";

const invalidate = [queryKeys.groups, queryKeys.keys];

function GroupDialog({ group, onClose }: { group: AccessGroup | null; onClose: () => void }) {
  const { t } = useTranslation();
  const keys = useKeys();
  const flows = useFlows();
  const [name, setName] = useState(group?.name ?? "");
  const [description, setDescription] = useState(group?.description ?? "");
  const [keyIds, setKeyIds] = useState<Id[]>(group?.key_ids ?? []);
  const [flowIds, setFlowIds] = useState<Id[]>(group?.flow_ids ?? []);
  const save = useApiMutation<AccessGroup>(invalidate);

  const submit = () => {
    const body = { name, description, key_ids: keyIds, flow_ids: flowIds };
    save.mutate(group ? { op: "group_update", id: group.id, ...body } : { op: "group_create", ...body }, {
      onSuccess: onClose,
      onError: notifyError,
    });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{group ? t("groups.editTitle") : t("groups.create")}</DialogTitle>
        </DialogHeader>
        <div className="grid gap-4">
          <TextField label={t("common.name")} value={name} onChange={setName} />
          <TextField label={t("common.description")} value={description} onChange={setDescription} />
          <Field label={t("groups.flows")}>
            <CheckList
              items={(flows.data ?? []).map((f) => ({ id: f.id, label: f.name, hint: `/${f.path}` }))}
              selected={flowIds}
              onChange={setFlowIds}
              empty={t("groups.noFlows")}
            />
          </Field>
          <Field label={t("groups.keys")}>
            <CheckList
              items={(keys.data ?? []).map((k) => ({ id: k.id, label: k.name }))}
              selected={keyIds}
              onChange={setKeyIds}
              empty={t("groups.noKeys")}
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={submit} disabled={!name.trim() || save.isPending}>
            {group ? t("common.save") : t("common.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function GroupsPage() {
  const { t } = useTranslation();
  const groups = useGroups();
  const keys = useKeys();
  const flows = useFlows();
  const [editing, setEditing] = useState<AccessGroup | null | undefined>(undefined);
  const [deleting, setDeleting] = useState<AccessGroup | null>(null);
  const remove = useApiMutation<unknown>(invalidate);

  const keyName = (id: Id) => keys.data?.find((k) => k.id === id)?.name ?? `#${id}`;
  const flowName = (id: Id) => flows.data?.find((f) => f.id === id)?.name ?? `#${id}`;

  return (
    <div>
      <PageHeader
        title={t("groups.title")}
        subtitle={t("groups.subtitle")}
        action={
          <Button onClick={() => setEditing(null)}>
            <PlusIcon />
            {t("groups.create")}
          </Button>
        }
      />
      {groups.data && groups.data.length === 0 ? (
        <EmptyState />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("groups.flows")}</TableHead>
              <TableHead>{t("groups.keys")}</TableHead>
              <TableHead className="w-10" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {(groups.data ?? []).map((g) => (
              <TableRow key={g.id}>
                <TableCell>
                  <div className="font-medium">{g.name}</div>
                  {g.description && <div className="text-xs text-muted-foreground">{g.description}</div>}
                </TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {g.flow_ids.map((id) => (
                      <Badge key={id} variant="outline">
                        {flowName(id)}
                      </Badge>
                    ))}
                  </div>
                </TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {g.key_ids.map((id) => (
                      <Badge key={id} variant="secondary">
                        {keyName(id)}
                      </Badge>
                    ))}
                  </div>
                </TableCell>
                <TableCell>
                  <DropdownMenu>
                    <DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" aria-label={t("common.actions")} />}>
                      <MoreHorizontalIcon />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={() => setEditing(g)}>{t("common.edit")}</DropdownMenuItem>
                      <DropdownMenuItem variant="destructive" onClick={() => setDeleting(g)}>
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
      {editing !== undefined && <GroupDialog group={editing} onClose={() => setEditing(undefined)} />}
      <ConfirmDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        title={t("common.confirmDelete", { name: deleting?.name })}
        description={t("common.confirmDeleteText")}
        confirmLabel={t("common.delete")}
        destructive
        onConfirm={() => deleting && remove.mutate({ op: "group_delete", id: deleting.id }, { onError: notifyError })}
      />
    </div>
  );
}
