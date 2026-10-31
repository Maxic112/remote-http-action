import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { Request } from "@/lib/types";
import { mockApi, renderWithProviders } from "@/test/utils";

import { LogsPage } from "./LogsPage";

const info = {
  dir: "C:\\data\\logs",
  categories: [
    { category: "http", enabled: true, files: 1, size_bytes: 2048 },
    { category: "cli", enabled: false, files: 0, size_bytes: 0 },
    { category: "flow", enabled: true, files: 1, size_bytes: 100 },
  ],
};

describe("LogsPage", () => {
  it("shows entries per category and their details", async () => {
    const user = userEvent.setup();
    const calls = mockApi({
      logs_info: () => info,
      logs_read: (r) => {
        const req = r as Extract<Request, { op: "logs_read" }>;
        if (req.category === "http")
          return [
            { ts: "2026-01-01T10:00:00Z", ip: "10.0.0.2", method: "POST", path: "/flow/sleep/", status: 401, key: null, duration_ms: 3 },
          ];
        return [
          { ts: "2026-01-01T10:00:00Z", event: "finished", flow: "sleep", source: "http", key: "phone", status: "failed", error: "boom" },
        ];
      },
    });
    renderWithProviders(<LogsPage />);
    expect(await screen.findByText("boom")).toBeInTheDocument();
    expect(screen.getByText("http · phone")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "HTTP requests" }));
    expect(await screen.findByText("POST /flow/sleep/")).toBeInTheDocument();
    expect(screen.getByTestId("logs-size")).toHaveTextContent("2.0 KB");

    await user.click(screen.getByText("10.0.0.2"));
    expect(await screen.findByTestId("log-details")).toHaveTextContent('"status": 401');

    expect(calls).toContainEqual({ op: "logs_read", category: "http", limit: 300, query: undefined });
  });

  it("searches and clears a category", async () => {
    const user = userEvent.setup();
    const calls = mockApi({ logs_info: () => info, logs_read: () => [], logs_clear: () => ({}) });
    renderWithProviders(<LogsPage />);
    expect(await screen.findByText("No entries.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Search…"), "phone");
    await waitFor(() => expect(calls).toContainEqual({ op: "logs_read", category: "flow", limit: 300, query: "phone" }));
    await user.click(screen.getByRole("button", { name: "Clear" }));
    await user.click(await screen.findByRole("button", { name: "Clear" }));
    await waitFor(() => expect(calls).toContainEqual({ op: "logs_clear", category: "flow" }));
  });
});
