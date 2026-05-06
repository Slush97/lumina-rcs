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

export type MessagePart =
  | { kind: "text"; text: string }
  | {
      kind: "media";
      media_id: string;
      name: string;
      mime: string;
      size: number;
      width: number;
      height: number;
    };

export type Message = {
  id: string;
  /** Client-assigned id present on outgoing messages until the server
   *  confirms; lets the UI reconcile optimistic inserts. */
  tmp_id: string;
  conversation_id: string;
  /** libgm timestamp; microseconds for recent messages, ms for old ones. */
  timestamp: number;
  from_me: boolean;
  sender_id: string;
  sender_name: string;
  /** MessageStatusType numeric code (0=unknown, 1xx=incoming, otherwise
   *  outgoing state machine: SENDING/SENT/DELIVERED/DISPLAYED/FAILED…). */
  status: number;
  status_label: string;
  parts: MessagePart[];
  reply_to?: string;
};

export type MessageCursor = {
  last_item_id: string;
  last_item_timestamp: number;
};

export type FetchMessagesResult = {
  messages: Message[];
  cursor: MessageCursor | null;
  total: number;
};

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
  fetchMessages: (
    conversationId: string,
    count = 50,
    cursor: MessageCursor | null = null,
  ) =>
    invoke<FetchMessagesResult>("bridge_fetch_messages", {
      conversationId,
      count,
      cursor,
    }),
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
  | "conversation_updated"
  | "message_upserted";

export function on<T = unknown>(
  event: LuminaEvent,
  cb: (payload: T) => void
): Promise<UnlistenFn> {
  return listen<T>(`lumina://${event}`, (e) => cb(e.payload));
}
