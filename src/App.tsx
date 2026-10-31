import { HistoryIcon, KeyRoundIcon, SettingsIcon, UsersIcon, WorkflowIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { DelayedRunDialog } from "@/components/DelayedRunDialog";
import { ElevationBadge } from "@/components/ElevationBadge";
import { ServerBadge } from "@/components/ServerBadge";
import { FlowsPage } from "@/features/flows/FlowsPage";
import { GroupsPage } from "@/features/groups/GroupsPage";
import { KeysPage } from "@/features/keys/KeysPage";
import { RunsPage } from "@/features/runs/RunsPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import i18n, { resolveLanguage } from "@/i18n";
import { useSettings } from "@/lib/api";
import { useAppEvents } from "@/lib/useAppEvents";
import { cn } from "@/lib/utils";

const PAGES = {
  flows: { icon: WorkflowIcon, component: FlowsPage },
  keys: { icon: KeyRoundIcon, component: KeysPage },
  groups: { icon: UsersIcon, component: GroupsPage },
  runs: { icon: HistoryIcon, component: RunsPage },
  settings: { icon: SettingsIcon, component: SettingsPage },
} as const;

export type PageId = keyof typeof PAGES;

export default function App() {
  const { t } = useTranslation();
  const [page, setPage] = useState<PageId>("flows");
  const settings = useSettings();
  useAppEvents();

  const language = settings.data?.app.language;
  useEffect(() => {
    if (language) void i18n.changeLanguage(resolveLanguage(language));
  }, [language]);

  const Page = PAGES[page].component;

  return (
    <div className="flex h-screen bg-background text-foreground">
      <aside className="flex w-56 shrink-0 flex-col border-r bg-sidebar">
        <div className="px-4 py-4 text-sm font-semibold">{t("app.title")}</div>
        <nav className="grid gap-0.5 px-2">
          {(Object.keys(PAGES) as PageId[]).map((id) => {
            const Icon = PAGES[id].icon;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setPage(id)}
                className={cn(
                  "flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm text-sidebar-foreground transition-colors hover:bg-sidebar-accent",
                  page === id && "bg-sidebar-accent font-medium",
                )}
              >
                <Icon className="size-4" />
                {t(`nav.${id}`)}
              </button>
            );
          })}
        </nav>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center justify-end gap-4 border-b px-6">
          <ServerBadge />
          <ElevationBadge />
        </header>
        <main className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-6xl">
            <Page />
          </div>
        </main>
      </div>
      <DelayedRunDialog />
    </div>
  );
}
