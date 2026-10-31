import { describe, expect, it } from "vitest";

import { invokeMock, mockApi } from "@/test/utils";

import { ApiError, call, toApiError } from "./api";

describe("api client", () => {
  it("sends requests through the api command", async () => {
    mockApi({ keys_list: () => [] });
    await expect(call({ op: "keys_list" })).resolves.toEqual([]);
    expect(invokeMock).toHaveBeenCalledWith("api", { request: { op: "keys_list" } });
  });

  it("normalises backend errors", async () => {
    mockApi({ key_delete: () => Promise.reject({ code: "NOT_FOUND", message: "API key 5 not found" }) });
    const err = (await call({ op: "key_delete", id: 5 }).catch((e: unknown) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("NOT_FOUND");
    expect(err.message).toBe("API key 5 not found");
  });

  it("wraps unknown errors", () => {
    expect(toApiError(new Error("boom"))).toMatchObject({ code: "INTERNAL", message: "boom" });
    expect(toApiError("text")).toMatchObject({ code: "INTERNAL", message: "text" });
  });
});
