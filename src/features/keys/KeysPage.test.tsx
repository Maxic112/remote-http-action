import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { ApiKey } from "@/lib/types";
import { mockApi, renderWithProviders } from "@/test/utils";

import { KeysPage } from "./KeysPage";

const key: ApiKey = {
  id: 1,
  name: "phone",
  key_prefix: "rha_AbCdEfGh",
  enabled: true,
  group_ids: [7],
  created_at: "2026-01-01T00:00:00Z",
  last_used_at: null,
};

describe("KeysPage", () => {
  it("lists keys with prefix and group names, never the secret", async () => {
    mockApi({
      keys_list: () => [key],
      groups_list: () => [{ id: 7, name: "ops", description: "", key_ids: [1], flow_ids: [], created_at: "" }],
    });
    renderWithProviders(<KeysPage />);
    expect(await screen.findByText("phone")).toBeInTheDocument();
    expect(screen.getByText("rha_AbCdEfGh…")).toBeInTheDocument();
    expect(await screen.findByText("ops")).toBeInTheDocument();
    expect(screen.getByText("never")).toBeInTheDocument();
  });

  it("creates a key and shows the secret once", async () => {
    const user = userEvent.setup();
    const calls = mockApi({
      keys_list: () => [],
      groups_list: () => [{ id: 7, name: "ops", description: "", key_ids: [], flow_ids: [], created_at: "" }],
      key_create: () => ({ key: { ...key, name: "tablet" }, secret: "rha_secret_value" }),
    });
    renderWithProviders(<KeysPage />);
    await user.click(await screen.findByRole("button", { name: /create key/i }));
    await user.type(screen.getByLabelText("Name"), "tablet");
    await user.click(await screen.findByRole("checkbox", { name: "ops" }));
    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(await screen.findByTestId("secret")).toHaveTextContent("rha_secret_value");
    expect(calls).toContainEqual({ op: "key_create", name: "tablet", group_ids: [7] });

    await user.click(screen.getAllByRole("button", { name: "Close" }).at(-1)!);
    await waitFor(() => expect(screen.queryByTestId("secret")).not.toBeInTheDocument());
  });

  it("toggles a key", async () => {
    const user = userEvent.setup();
    const calls = mockApi({ keys_list: () => [key], groups_list: () => [], key_update: () => ({ ...key, enabled: false }) });
    renderWithProviders(<KeysPage />);
    await user.click(await screen.findByRole("switch", { name: "Enabled" }));
    await waitFor(() => expect(calls).toContainEqual({ op: "key_update", id: 1, enabled: false }));
  });
});
