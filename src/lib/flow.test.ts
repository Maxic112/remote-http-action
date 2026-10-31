import { describe, expect, it } from "vitest";

import { emptyFlow, flowUrl, hasAdminSteps, linesToList, listToLines, moveItem, newAction, slugify, toInput } from "./flow";
import type { Flow } from "./types";

describe("flow helpers", () => {
  it("creates actions with sensible defaults", () => {
    expect(newAction("delay").params).toEqual({ seconds: 5 });
    expect(newAction("cli_command", "pwsh").params).toEqual({ shell: "pwsh", command: "", working_dir: null });
    expect(newAction("shutdown")).toMatchObject({ on_error: "stop", run_as_admin: false, timeout_sec: null });
  });

  it("matches backend defaults for new flows", () => {
    expect(emptyFlow()).toMatchObject({
      enabled: true,
      response_mode: "sync",
      timeout_sec: 60,
      concurrency: "reject_if_running",
      cancelable: true,
    });
  });

  it("slugifies names into paths", () => {
    expect(slugify("Sleep PC")).toBe("sleep-pc");
    expect(slugify("  --Reboot!! now ")).toBe("reboot-now");
    expect(slugify("Выключить")).toBe("");
  });

  it("detects admin steps", () => {
    const a = newAction("reboot");
    expect(hasAdminSteps({ actions: [a] })).toBe(false);
    expect(hasAdminSteps({ actions: [a, { ...a, run_as_admin: true }] })).toBe(true);
  });

  it("moves items within bounds", () => {
    expect(moveItem([1, 2, 3], 0, 1)).toEqual([2, 1, 3]);
    expect(moveItem([1, 2, 3], 2, 3)).toEqual([1, 2, 3]);
    expect(moveItem([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
  });

  it("converts textarea lines to lists", () => {
    expect(linesToList(" 10.0.0.0/8 \n\n192.168.1.5\r\n")).toEqual(["10.0.0.0/8", "192.168.1.5"]);
    expect(listToLines(["a", "b"])).toBe("a\nb");
  });

  it("builds flow URLs", () => {
    expect(flowUrl("/flow/", "sleep")).toBe("/flow/sleep/");
    expect(flowUrl("/flow/", "/pc/off/")).toBe("/flow/pc/off/");
    expect(flowUrl("/flow/", "")).toBe("/flow/");
  });

  it("strips server fields", () => {
    const flow: Flow = { ...emptyFlow(), id: 1, created_at: "a", updated_at: "b" };
    const input = toInput(flow);
    expect(input).not.toHaveProperty("id");
    expect(input).not.toHaveProperty("created_at");
  });
});
