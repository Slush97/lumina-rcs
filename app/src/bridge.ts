import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Conversation = {
  id: string;
  name: string;
  snippet: string;
  snippet_from: string;
  snippet_self: boolean;
  timestamp: number;
  unread: boolean;
  is_group: boolean;
  avatar_color: string;
  pinned: boolean;
  read_only: boolean;
};

export type Status = { paired: boolean; connected: boolean };

export type DetectedBrowser = {
  id: string;
  display: string;
  cookie_count: number;
  has_sapisid: boolean;
};

export type ImportedCookies = {
  browser: string;
  cookies: Record<string, string>;
};

export const bridge = {
  status: () => invoke<Status>("bridge_status"),
  pair: () => invoke<{ qr_url: string }>("bridge_pair"),
  connect: () => invoke<{ ok: boolean }>("bridge_connect"),
  unpair: () => invoke<{ ok: boolean }>("bridge_unpair"),
  listConversations: (count = 50) =>
    invoke<Conversation[]>("bridge_list_conversations", { count }),
  startGaiaLogin: () => invoke<void>("start_gaia_login"),
  pairWithCookies: (cookies: Record<string, string>) =>
    invoke<{ started: boolean }>("pair_with_cookies", { cookies }),
  detectBrowsers: () => invoke<DetectedBrowser[]>("detect_browsers"),
  importBrowserCookies: (browser: string) =>
    invoke<ImportedCookies>("import_browser_cookies", { browser }),
};

export type LuminaEvent =
  | "hello"
  | "qr"
  | "gaia_emoji"
  | "paired"
  | "ready"
  | "error"
  | "phone_offline"
  | "phone_online"
  | "conversation_updated";

export function on<T = unknown>(
  event: LuminaEvent,
  cb: (payload: T) => void
): Promise<UnlistenFn> {
  return listen<T>(`lumina://${event}`, (e) => cb(e.payload));
}
