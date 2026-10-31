import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { newAction } from "@/lib/flow";
import type { ActionDef } from "@/lib/types";
import { dialogOpenMock, renderWithProviders, systemInfo } from "@/test/utils";

import { ActionEditor } from "./ActionEditor";

const onUpdate = vi.fn<(a: ActionDef) => void>();
const latest = () => onUpdate.mock.lastCall![0];

function Harness({ initial }: { initial: ActionDef }) {
  const [action, setAction] = useState(initial);
  return (
    <ActionEditor
      action={action}
      index={0}
      count={1}
      shells={systemInfo.shells}
      onChange={(a) => {
        setAction(a);
        onUpdate(a);
      }}
      onMove={() => {}}
      onRemove={() => {}}
    />
  );
}

describe("ActionEditor", () => {
  it("picks a script file and collects arguments", async () => {
    const user = userEvent.setup();
    dialogOpenMock.mockResolvedValue("C:\\Scripts\\backup.ps1");
    renderWithProviders(<Harness initial={newAction("run_script", "powershell")} />);

    await user.click(screen.getByRole("button", { name: /Browse/ }));
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ filters: expect.arrayContaining([{ name: "Scripts", extensions: ["ps1"] }]) }),
    );
    expect(screen.getByLabelText("Script file")).toHaveValue("C:\\Scripts\\backup.ps1");

    await user.type(screen.getByLabelText("Arguments"), "-Target{Enter}D:\\Backup");
    expect(latest().params).toMatchObject({ script_path: "C:\\Scripts\\backup.ps1", args: ["-Target", "D:\\Backup"] });
    expect(screen.getByRole("checkbox", { name: "Return output to the client" })).toBeInTheDocument();
  });

  it("configures an application launch", async () => {
    const user = userEvent.setup();
    dialogOpenMock.mockResolvedValue(null);
    renderWithProviders(<Harness initial={newAction("launch_app")} />);
    await user.click(screen.getByRole("button", { name: /Browse/ }));
    expect(screen.getByLabelText("Program")).toHaveValue("");
    await user.type(screen.getByLabelText("Program"), "C:\\Apps\\player.exe");
    await user.click(screen.getByRole("checkbox", { name: /Wait for it to exit/ }));
    expect(latest().params).toMatchObject({ path: "C:\\Apps\\player.exe", wait: true });
    expect(screen.queryByRole("checkbox", { name: "Return output to the client" })).not.toBeInTheDocument();
  });
});
