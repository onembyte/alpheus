import { invoke } from "@tauri-apps/api/core";

/**
 * Thin Drive v3 client. The Rust side owns credentials and hands out
 * short-lived access tokens; the webview calls the API directly (Google's
 * endpoints are CORS-enabled). Every mutation is a move to Drive's own
 * 30-day Trash — the one exception, emptying that Trash, is explicit.
 */
async function g<T>(path: string, init?: RequestInit): Promise<T> {
  const token = await invoke<string>("google_token");
  const res = await fetch(`https://www.googleapis.com/drive/v3/${path}`, {
    ...init,
    headers: { Authorization: `Bearer ${token}`, ...(init?.headers ?? {}) },
  });
  if (!res.ok) throw new Error(`Drive API ${res.status}: ${(await res.text()).slice(0, 300)}`);
  return res.status === 204 ? (undefined as T) : ((await res.json()) as T);
}

export interface DriveQuota {
  limitBytes: number;
  usageBytes: number;
  driveBytes: number;
  trashBytes: number;
  email: string;
}

export async function fetchQuota(): Promise<DriveQuota> {
  const j = await g<{
    storageQuota: { limit?: string; usage?: string; usageInDrive?: string; usageInDriveTrash?: string };
    user?: { emailAddress?: string };
  }>("about?fields=storageQuota,user(emailAddress)");
  const q = j.storageQuota;
  return {
    limitBytes: Number(q.limit ?? 0),
    usageBytes: Number(q.usage ?? 0),
    driveBytes: Number(q.usageInDrive ?? 0),
    trashBytes: Number(q.usageInDriveTrash ?? 0),
    email: j.user?.emailAddress ?? "",
  };
}

export interface DriveFile {
  id: string;
  name: string;
  sizeBytes: number;
  md5: string | undefined;
  mimeType: string;
  modifiedTime: string;
}

/** Pages through every file the user owns (capped at 30k files). */
export async function listOwnFiles(onProgress?: (count: number) => void): Promise<DriveFile[]> {
  const files: DriveFile[] = [];
  let pageToken = "";
  let pages = 0;
  do {
    const j = await g<{
      nextPageToken?: string;
      files?: {
        id: string;
        name: string;
        size?: string;
        md5Checksum?: string;
        mimeType: string;
        modifiedTime: string;
      }[];
    }>(
      `files?q=${encodeURIComponent("trashed=false and 'me' in owners")}&fields=nextPageToken,files(id,name,size,md5Checksum,mimeType,modifiedTime)&pageSize=1000${
        pageToken ? `&pageToken=${encodeURIComponent(pageToken)}` : ""
      }`,
    );
    for (const f of j.files ?? []) {
      files.push({
        id: f.id,
        name: f.name,
        sizeBytes: Number(f.size ?? 0),
        md5: f.md5Checksum,
        mimeType: f.mimeType,
        modifiedTime: f.modifiedTime,
      });
    }
    pageToken = j.nextPageToken ?? "";
    pages++;
    onProgress?.(files.length);
  } while (pageToken && pages < 30);
  return files;
}

export const trashFile = (id: string) =>
  g<void>(`files/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ trashed: true }),
  });

export const emptyTrash = () => g<void>("files/trash", { method: "DELETE" });
