import { toast } from "sonner";

import i18n from "@/i18n";

import { toApiError } from "./api";

export function notifyError(e: unknown) {
  const err = toApiError(e);
  const key = `errors.${err.code}`;
  const title = i18n.exists(key) ? i18n.t(key) : i18n.t("common.error");
  toast.error(title, { description: err.message });
}

export function notifySuccess(message: string) {
  toast.success(message);
}
