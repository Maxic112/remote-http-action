import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { emptyFlow, newAction } from "@/lib/flow";
import type { Flow, FlowInput, Request } from "@/lib/types";
import { mockApi, renderWithProviders, systemInfo } from "@/test/utils";

import { FlowEditor } from "./FlowEditor";

const settings = { server: { flow_prefix: "/flow/" } };

describe("FlowEditor", () => {
  it("creates a flow with a generated path and actions", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const calls = mockApi({ system_info: () => systemInfo, settings_get: () => settings, flow_create: () => ({}) });
    renderWithProviders(<FlowEditor flow={null} onClose={onClose} />);

    await user.type(screen.getByLabelText("Name"), "Sleep PC");
    expect(screen.getByLabelText("Path")).toHaveValue("sleep-pc");
    expect(screen.getByText("Call URL: POST /flow/sleep-pc/")).toBeInTheDocument();

    const save = screen.getByRole("button", { name: "Create" });
    expect(save).toBeDisabled();

    await user.click(screen.getByRole("tab", { name: /Actions/ }));
    await user.click(screen.getByRole("button", { name: /Add action/ }));
    await user.click(await screen.findByRole("menuitem", { name: "Sleep" }));
    expect(screen.getByTestId("action-0")).toHaveTextContent("Sleep");

    await user.click(screen.getByRole("button", { name: /Add action/ }));
    await user.click(await screen.findByRole("menuitem", { name: "Run command" }));
    await user.type(screen.getByLabelText("Command"), "echo hi");

    await user.click(save);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const req = calls.find((c) => c.op === "flow_create") as Extract<Request, { op: "flow_create" }>;
    expect(req.flow.path).toBe("sleep-pc");
    expect(req.flow.actions.map((a) => a.type)).toEqual(["sleep", "cli_command"]);
    expect(req.flow.actions[1].params).toMatchObject({ shell: "cmd", command: "echo hi" });
  });

  it("marks admin steps with a shield and keeps existing values on edit", async () => {
    const user = userEvent.setup();
    const input: FlowInput = { ...emptyFlow(), name: "Reboot", path: "reboot", actions: [newAction("reboot")] };
    const flow: Flow = { ...input, id: 3, created_at: "", updated_at: "" };
    const calls = mockApi({ system_info: () => systemInfo, settings_get: () => settings, flow_update: () => flow });
    renderWithProviders(<FlowEditor flow={flow} onClose={() => {}} />);

    expect(screen.getByLabelText("Path")).toHaveValue("reboot");
    await user.click(screen.getByRole("tab", { name: /Actions/ }));
    expect(screen.queryByTestId("admin-shield")).not.toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: /Requires administrator rights/ }));
    expect(screen.getByTestId("admin-shield")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(calls.some((c) => c.op === "flow_update")).toBe(true));
    const req = calls.find((c) => c.op === "flow_update") as Extract<Request, { op: "flow_update" }>;
    expect(req.id).toBe(3);
    expect(req.flow.actions[0].run_as_admin).toBe(true);
    expect(req.flow).not.toHaveProperty("id");
  });
});
