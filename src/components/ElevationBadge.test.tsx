import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { mockApi, renderWithProviders, systemInfo } from "@/test/utils";

import { ElevationBadge } from "./ElevationBadge";

describe("ElevationBadge", () => {
  it("shows administrator state", async () => {
    mockApi({ system_info: () => ({ ...systemInfo, elevated: true }) });
    renderWithProviders(<ElevationBadge />);
    expect(await screen.findByTestId("elevation-admin")).toHaveTextContent("Administrator");
  });

  it("restarts elevated after confirmation", async () => {
    const user = userEvent.setup();
    const calls = mockApi({ system_info: () => systemInfo, app_restart_elevated: () => ({ ok: true }) });
    renderWithProviders(<ElevationBadge />);
    await user.click(await screen.findByTestId("elevation-user"));
    expect(calls.some((c) => c.op === "app_restart_elevated")).toBe(false);
    await user.click(await screen.findByRole("button", { name: "Restart" }));
    await waitFor(() => expect(calls.some((c) => c.op === "app_restart_elevated")).toBe(true));
  });
});
