import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { Request, Settings } from "@/lib/types";
import { mockApi, renderWithProviders, systemInfo } from "@/test/utils";

import { SettingsPage } from "./SettingsPage";

const settings: Settings = {
  server: {
    enabled: true,
    bind_address: "0.0.0.0",
    port: 8765,
    allowed_ips: [],
    allowed_hosts: [],
    trusted_proxies: [],
    flow_prefix: "/flow/",
    allowed_path: "/get-allowed/",
    status_path: "/status/",
    public_status: true,
    hide_forbidden_flows: false,
    brute_force: { enabled: true, max_failures: 10, window_sec: 300, ban_sec: 900 },
  },
  cli: { enabled: true },
  app: { close_to_tray: true, language: "system", run_history_limit: 200 },
};

function setup() {
  return mockApi({
    settings_get: () => settings,
    system_info: () => systemInfo,
    server_status: () => ({ state: "running", address: "0.0.0.0:8765", error: null }),
    bans_list: () => [{ ip: "10.0.0.9", seconds_left: 120 }],
    settings_update: (r) => (r as Extract<Request, { op: "settings_update" }>).settings,
    ban_remove: () => ({}),
  });
}

describe("SettingsPage", () => {
  it("saves edited settings including list fields", async () => {
    const user = userEvent.setup();
    const calls = setup();
    renderWithProviders(<SettingsPage />);

    const port = await screen.findByLabelText("Port");
    await user.tripleClick(port);
    await user.keyboard("9000");
    await user.type(screen.getByLabelText("Allowed client addresses"), "192.168.1.0/24{Enter}10.0.0.5");
    await user.click(screen.getByRole("switch", { name: /Allow control from the command line/ }));

    expect(screen.getByText("There are unsaved changes")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(calls.some((c) => c.op === "settings_update")).toBe(true));
    const req = calls.find((c) => c.op === "settings_update") as Extract<Request, { op: "settings_update" }>;
    expect(req.settings.server.port).toBe(9000);
    expect(req.settings.server.allowed_ips).toEqual(["192.168.1.0/24", "10.0.0.5"]);
    expect(req.settings.cli.enabled).toBe(false);
  });

  it("controls the server and lists bans", async () => {
    const user = userEvent.setup();
    const calls = setup();
    renderWithProviders(<SettingsPage />);
    expect(await screen.findByText("10.0.0.9")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Unblock" }));
    await waitFor(() => expect(calls).toContainEqual({ op: "ban_remove", ip: "10.0.0.9" }));
    expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
  });
});
