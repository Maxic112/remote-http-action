import { open } from "@tauri-apps/plugin-dialog";

export interface FileFilter {
  name: string;
  extensions: string[];
}

/** Opens the system file picker; resolves to the chosen path or `null` if canceled. */
export async function pickFile(filters: FileFilter[] = []): Promise<string | null> {
  const result = await open({ multiple: false, directory: false, filters });
  return typeof result === "string" ? result : null;
}
