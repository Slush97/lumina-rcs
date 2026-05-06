// Single global store. The app is small enough that one reactive object
// beats Pinia's ceremony. Two pieces of state:
//
//   conversations  — Map<conv_id, Conversation>, populated by loadInbox()
//                    and patched in-place by `conversation_updated` events.
//   threads        — Map<conv_id, ThreadState>, lazily filled the first
//                    time a thread is opened, then kept fresh by
//                    `message_upserted` events.
//
// initStore() wires the global event subscriptions exactly once, so all
// screens read the same data without each one re-subscribing.

import {
  computed,
  reactive,
  toValue,
  type ComputedRef,
  type MaybeRefOrGetter,
} from "vue";
import {
  bridge,
  on,
  type Conversation,
  type Message,
  type MessageCursor,
} from "./bridge";

type ThreadState = {
  messages: Map<string, Message>;
  /** Cursor returned with the oldest page; pass back to fetch older. */
  cursor: MessageCursor | null;
  /** False once the bridge returns no cursor (we've reached the start). */
  hasMore: boolean;
  /** True once the first page has been fetched. */
  loaded: boolean;
};

const state = reactive({
  conversations: new Map<string, Conversation>(),
  threads: new Map<string, ThreadState>(),
});

let unlisteners: (() => void)[] = [];
let initialized = false;

function ensureThread(id: string): ThreadState {
  let t = state.threads.get(id);
  if (!t) {
    t = reactive({
      messages: new Map<string, Message>(),
      cursor: null,
      hasMore: true,
      loaded: false,
    }) as ThreadState;
    state.threads.set(id, t);
  }
  return t;
}

function bumpConversationFromMessage(msg: Message) {
  const conv = state.conversations.get(msg.conversation_id);
  if (!conv || msg.timestamp <= conv.timestamp) return;
  const text = msg.parts.find((p) => p.kind === "text");
  state.conversations.set(msg.conversation_id, {
    ...conv,
    timestamp: msg.timestamp,
    snippet: text?.text ?? conv.snippet,
    snippet_self: msg.from_me,
    snippet_from: msg.sender_name,
    unread: msg.from_me ? false : true,
  });
}

export async function initStore() {
  if (initialized) return;
  initialized = true;

  unlisteners.push(
    await on<Conversation>("conversation_updated", (conv) => {
      state.conversations.set(conv.id, conv);
    }),
  );
  unlisteners.push(
    await on<Message>("message_upserted", (msg) => {
      const t = ensureThread(msg.conversation_id);
      // Outgoing message reconciliation: a message we sent first appears
      // with id == tmp_id, then again with the server-assigned id. Drop
      // the temp entry the moment we see the real one.
      if (msg.tmp_id && msg.id !== msg.tmp_id && t.messages.has(msg.tmp_id)) {
        t.messages.delete(msg.tmp_id);
      }
      t.messages.set(msg.id, msg);
      bumpConversationFromMessage(msg);
    }),
  );
}

export function teardownStore() {
  unlisteners.forEach((u) => u());
  unlisteners = [];
  initialized = false;
}

export const conversations: ComputedRef<Conversation[]> = computed(() => {
  const arr = Array.from(state.conversations.values());
  arr.sort((a, b) => b.timestamp - a.timestamp);
  return arr;
});

export function conversation(
  idRef: MaybeRefOrGetter<string>,
): ComputedRef<Conversation | undefined> {
  return computed(() => state.conversations.get(toValue(idRef)));
}

export async function loadInbox(count = 50) {
  const list = await bridge.listConversations(count);
  for (const c of list) state.conversations.set(c.id, c);
}

export function thread(
  idRef: MaybeRefOrGetter<string>,
): ComputedRef<Message[]> {
  return computed(() => {
    const t = state.threads.get(toValue(idRef));
    if (!t) return [];
    const arr = Array.from(t.messages.values());
    arr.sort((a, b) => a.timestamp - b.timestamp);
    return arr;
  });
}

export function threadStatus(
  idRef: MaybeRefOrGetter<string>,
): ComputedRef<{ loaded: boolean; hasMore: boolean }> {
  return computed(() => {
    const t = state.threads.get(toValue(idRef));
    return {
      loaded: t?.loaded ?? false,
      hasMore: t?.hasMore ?? true,
    };
  });
}

export async function loadThread(convId: string) {
  const t = ensureThread(convId);
  if (t.loaded) return;
  const res = await bridge.fetchMessages(convId, 50, null);
  for (const m of res.messages) t.messages.set(m.id, m);
  t.cursor = res.cursor;
  t.hasMore = res.cursor !== null;
  t.loaded = true;
}

/** Build a URL the webview can load for media bytes. The custom protocol
 *  is registered in Rust and proxies through bridge.fetch_media. Browser
 *  caches the response per id; no JS-side cache needed. */
export function mediaUrl(mediaId: string): string {
  return `lumina-media://localhost/${encodeURIComponent(mediaId)}`;
}

export async function loadOlder(convId: string) {
  const t = ensureThread(convId);
  if (!t.cursor || !t.hasMore) return;
  const res = await bridge.fetchMessages(convId, 50, t.cursor);
  for (const m of res.messages) t.messages.set(m.id, m);
  t.cursor = res.cursor;
  t.hasMore = res.cursor !== null;
}
