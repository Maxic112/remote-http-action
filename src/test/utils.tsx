import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Request } from "@/lib/types";

type Handler = (req: Request) => unknown;

export const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

export const dialogOpenMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...args: unknown[]) => dialogOpenMock(...args) }));

/** Routes `api` calls by `op`; unknown ops reject with NOT_FOUND. Returns the list of received requests. */
export function mockApi(handlers: Partial<Record<Request["op"], Handler>>) {
  const calls: Request[] = [];
  invokeMock.mockImplementation(async (_cmd: string, args: { request: Request }) => {
    calls.push(args.request);
    const h = handlers[args.request.op];
    if (!h) throw { code: "NOT_FOUND", message: `no mock for ${args.request.op}` };
    return h(args.request);
  });
  return calls;
}

export function renderWithProviders(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <TooltipProvider>{ui}</TooltipProvider>
    </QueryClientProvider>,
  );
}

export const systemInfo = {
  version: "0.1.0",
  build: "dev",
  elevated: false,
  pid: 1,
  paths: { data_dir: "C:\\data", db_path: "C:\\data\\data.db", portable: false },
  ipc_endpoint: "\\\\.\\pipe\\x",
  shells: [
    { id: "cmd", name: "Command Prompt (cmd)", path: "cmd.exe" },
    { id: "powershell", name: "Windows PowerShell", path: "powershell.exe" },
  ],
  interfaces: [{ name: "Ethernet", ip: "192.168.1.10", loopback: false }],
};
