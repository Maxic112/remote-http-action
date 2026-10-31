import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { UpdateStatus } from "@/lib/types";
import { mockApi, renderWithProviders } from "@/test/utils";

import { UpdatePage } from "./UpdatePage";

const base: UpdateStatus = {
  current_version: "1.0.0",
  install_kind: "msi",
  auto_check: true,
  feed_url: "https://api.github.com/repos/o/r/releases/latest",
  last_check_at: "2026-11-01T10:00:00Z",
  available: false,
  latest: null,
  asset: null,
  error: null,
  phase: { state: "idle" },
};

const newer: UpdateStatus = {
  ...base,
  available: true,
  latest: {
    version: "1.1.0",
    tag: "v1.1.0",
    name: "Remote HTTP Action 1.1.0",
    notes: "- Faster startup",
    published_at: "2026-11-02T10:00:00Z",
    html_url: "https://github.com/o/r/releases/tag/v1.1.0",
    assets: [],
  },
  asset: { name: "remote-http-action_1.1.0_x64_ru-RU.msi", url: "u", size: 5 * 1024 * 1024, digest: null },
};

describe("UpdatePage", () => {
  it("shows the current version and that it is up to date", async () => {
    const user = userEvent.setup();
    const calls = mockApi({ update_status: () => base, update_check: () => base });
    renderWithProviders(<UpdatePage />);
    expect(await screen.findByTestId("current-version")).toHaveTextContent("1.0.0");
    expect(screen.getByText("MSI installer")).toBeInTheDocument();
    expect(screen.getByTestId("up-to-date")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Check now/ }));
    await waitFor(() => expect(calls).toContainEqual({ op: "update_check" }));
  });

  it("downloads and starts the installer for the MSI version", async () => {
    const user = userEvent.setup();
    const calls = mockApi({
      update_status: () => newer,
      update_download: () => ({ ...newer, phase: { state: "ready", version: "1.1.0" } }),
      update_install: () => ({ ok: true }),
    });
    renderWithProviders(<UpdatePage />);
    expect(await screen.findByText("Version 1.1.0 is available")).toBeInTheDocument();
    expect(screen.getByTestId("release-notes")).toHaveTextContent("Faster startup");
    await user.click(screen.getByRole("button", { name: /Download and install/ }));
    await waitFor(() => expect(calls.map((c) => c.op)).toEqual(expect.arrayContaining(["update_download", "update_install"])));
  });

  it("waits for a restart in the portable version", async () => {
    const user = userEvent.setup();
    const ready: UpdateStatus = { ...newer, install_kind: "portable", phase: { state: "ready", version: "1.1.0" } };
    const calls = mockApi({ update_status: () => ready, update_install: () => ({ ok: true }) });
    renderWithProviders(<UpdatePage />);
    await user.click(await screen.findByRole("button", { name: /Restart and update/ }));
    await waitFor(() => expect(calls).toContainEqual({ op: "update_install" }));
  });

  it("shows download progress and errors", async () => {
    mockApi({
      update_status: () => ({
        ...newer,
        error: "update check: rate limited",
        phase: { state: "downloading", downloaded: 1024 * 1024, total: 4 * 1024 * 1024 },
      }),
    });
    renderWithProviders(<UpdatePage />);
    expect(await screen.findByTestId("update-progress")).toHaveTextContent("1.0 MB / 4.0 MB");
    expect(screen.getByText("update check: rate limited")).toBeInTheDocument();
  });

  it("does not offer to install in a development build", async () => {
    mockApi({ update_status: () => ({ ...newer, install_kind: "development" }) });
    renderWithProviders(<UpdatePage />);
    expect(await screen.findByText(/Development builds are not updated/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Download/ })).not.toBeInTheDocument();
  });

  it("changes update settings", async () => {
    const user = userEvent.setup();
    const calls = mockApi({ update_status: () => base, settings_set: () => ({}) });
    renderWithProviders(<UpdatePage />);
    await user.click(await screen.findByRole("switch", { name: /Check for updates automatically/ }));
    await waitFor(() => expect(calls).toContainEqual({ op: "settings_set", path: "updates.auto_check", value: false }));
    const feed = screen.getByLabelText("Update source");
    await user.clear(feed);
    await user.type(feed, "http://192.0.2.10:8899/feed");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(calls).toContainEqual({ op: "settings_set", path: "updates.feed_url", value: "http://192.0.2.10:8899/feed" }),
    );
  });
});
