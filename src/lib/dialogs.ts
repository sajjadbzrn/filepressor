import { open, save } from "@tauri-apps/plugin-dialog";
import { norm } from "./utils";

export async function pickFiles(title = "Choose files"): Promise<string[] | null> {
  const res = await open({ multiple: true, directory: false, title });
  if (!res) return null;
  return (Array.isArray(res) ? res : [res]).map(norm);
}

export async function pickFolders(title = "Choose folders"): Promise<string[] | null> {
  const res = await open({ multiple: true, directory: true, title });
  if (!res) return null;
  return (Array.isArray(res) ? res : [res]).map(norm);
}

export async function pickFolder(title = "Choose a folder"): Promise<string | null> {
  const res = await open({ multiple: false, directory: true, title });
  return res ? norm(res) : null;
}

export interface SaveFilter {
  name: string;
  extensions: string[];
}

export async function pickSavePath(
  defaultPath: string,
  filters: SaveFilter[],
): Promise<string | null> {
  const res = await save({ defaultPath: norm(defaultPath), filters });
  return res ? norm(res) : null;
}
