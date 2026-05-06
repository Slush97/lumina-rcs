<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  conversation,
  loadOlder,
  loadThread,
  thread,
  threadStatus,
} from "../store";
import type { MessagePart } from "../bridge";
import MediaPart from "../components/MediaPart.vue";

const props = defineProps<{
  conversationId: string;
  sidebarOpen: boolean;
}>();
defineEmits<{ (e: "toggleSidebar"): void }>();

const convIdRef = computed(() => props.conversationId);
const conv = conversation(convIdRef);
const messages = thread(convIdRef);
const status = threadStatus(convIdRef);

const errorMsg = ref<string>("");
const loadingOlder = ref(false);
const scrollEl = ref<HTMLElement | null>(null);
const lightboxUrl = ref<string | null>(null);
// True while the user is scrolled near the bottom — drives auto-follow on
// new messages. Updated on every scroll event.
const followingTail = ref(true);
const TAIL_THRESHOLD_PX = 80;

function isMedia(p: MessagePart): boolean {
  return p.kind === "media";
}

function openLightbox(url: string) {
  lightboxUrl.value = url;
}
function closeLightbox() {
  lightboxUrl.value = null;
}

// ESC closes the lightbox; body scroll lock while it's open.
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && lightboxUrl.value) closeLightbox();
}
watch(lightboxUrl, (u) => {
  document.body.style.overflow = u ? "hidden" : "";
});

function fmtTime(microsOrMillis: number): string {
  if (!microsOrMillis) return "";
  const ms = microsOrMillis > 1e14 ? microsOrMillis / 1000 : microsOrMillis;
  return new Date(ms).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

function outgoingTick(statusCode: number): string {
  if (statusCode === 11) return "Read";
  if (statusCode === 2) return "Delivered";
  if (statusCode === 1) return "Sent";
  if (statusCode === 5 || statusCode === 6) return "Sending…";
  if (statusCode === 4 || statusCode === 10) return "Queued";
  if (statusCode >= 8 && statusCode !== 11) return "Failed";
  return "";
}

function scrollToBottom() {
  const el = scrollEl.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

async function ensureLoaded() {
  errorMsg.value = "";
  try {
    await loadThread(props.conversationId);
    await nextTick();
    scrollToBottom();
    followingTail.value = true;
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function onLoadOlder() {
  const el = scrollEl.value;
  if (!el || loadingOlder.value) return;
  loadingOlder.value = true;
  // Anchor the visual position by the distance from the bottom of the
  // scroll content. Restore after layout commits.
  const fromBottom = el.scrollHeight - el.scrollTop;
  try {
    await loadOlder(props.conversationId);
    // Use rAF over nextTick so the browser has actually laid out the
    // newly-prepended bubbles before we restore scrollTop.
    requestAnimationFrame(() => {
      el.scrollTop = el.scrollHeight - fromBottom;
    });
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loadingOlder.value = false;
  }
}

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  followingTail.value =
    el.scrollHeight - el.scrollTop - el.clientHeight <= TAIL_THRESHOLD_PX;
}

// Auto-follow tail: when new messages arrive and the user is at the
// bottom, stick to the bottom. If they've scrolled up to read older
// stuff, leave them alone.
watch(
  () => messages.value.length,
  async () => {
    if (!followingTail.value) return;
    await nextTick();
    scrollToBottom();
  },
);

onMounted(() => {
  ensureLoaded();
  window.addEventListener("keydown", onKeydown);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  document.body.style.overflow = "";
});
watch(() => props.conversationId, ensureLoaded);
</script>

<template>
  <div class="w-full h-full flex flex-col">
    <header class="border-b border-ink-300/20 dark:border-ink-300/10 bg-paper px-3 py-3 flex items-center gap-3">
      <button
        class="text-ink-500 dark:text-ink-300 hover:text-accent-700 dark:hover:text-accent-300 px-2 text-lg leading-none"
        :title="sidebarOpen ? 'Hide conversations' : 'Show conversations'"
        @click="$emit('toggleSidebar')"
      >
        ☰
      </button>
      <div
        class="w-9 h-9 rounded-full flex items-center justify-center text-sm font-medium text-surface-0"
        :style="{ backgroundColor: conv?.avatar_color || '#a05a2b' }"
      >
        {{ (conv?.name || "?").charAt(0).toUpperCase() }}
      </div>
      <div class="flex-1 min-w-0">
        <p class="font-medium text-ink-700 dark:text-ink-100 truncate">
          {{ conv?.name || "Conversation" }}
        </p>
        <p v-if="conv?.is_group" class="text-xs text-ink-500 dark:text-ink-300">
          Group
        </p>
      </div>
    </header>

    <div
      ref="scrollEl"
      class="flex-1 min-h-0 overflow-y-auto p-4 space-y-2"
      style="overflow-anchor: none"
      @scroll.passive="onScroll"
    >
      <div
        v-if="!status.loaded"
        class="text-center text-ink-500 dark:text-ink-300 mt-10"
      >
        Loading messages…
      </div>
      <div v-else-if="errorMsg" class="text-center text-red-700 dark:text-red-400">
        {{ errorMsg }}
      </div>
      <template v-else>
        <div v-if="status.hasMore" class="text-center">
          <button
            :disabled="loadingOlder"
            class="text-xs px-3 py-1 rounded-lg text-ink-500 dark:text-ink-300
                   hover:text-accent-700 dark:hover:text-accent-300
                   disabled:opacity-40 disabled:cursor-not-allowed"
            @click="onLoadOlder"
          >
            {{ loadingOlder ? "Loading…" : "Load older messages" }}
          </button>
        </div>

        <div
          v-for="m in messages"
          :key="m.id"
          class="flex"
          :class="m.from_me ? 'justify-end' : 'justify-start'"
        >
          <div
            class="max-w-[75%] flex flex-col gap-1"
            :class="m.from_me ? 'items-end' : 'items-start'"
          >
            <span
              v-if="!m.from_me && conv?.is_group"
              class="text-xs text-ink-500 dark:text-ink-300 ml-2"
            >
              {{ m.sender_name }}
            </span>
            <div
              class="rounded-2xl px-3.5 py-2 text-sm whitespace-pre-wrap break-words flex flex-col gap-2"
              :class="m.from_me
                ? 'bg-accent-600 text-surface-0 rounded-br-sm'
                : 'paper-card text-ink-700 dark:text-ink-100 rounded-bl-sm'"
            >
              <template v-for="(p, i) in m.parts" :key="i">
                <MediaPart
                  v-if="isMedia(p)"
                  :media-id="(p as any).media_id"
                  :width="(p as any).width"
                  :height="(p as any).height"
                  :mime="(p as any).mime"
                  :name="(p as any).name"
                  @open-image="openLightbox"
                />
                <p v-else>{{ (p as any).text }}</p>
              </template>
              <p v-if="m.parts.length === 0" class="italic opacity-70">
                (empty)
              </p>
            </div>
            <span
              class="text-[10px] text-ink-500 dark:text-ink-300"
              :class="m.from_me ? 'mr-2' : 'ml-2'"
            >
              {{ fmtTime(m.timestamp) }}
              <template v-if="m.from_me">
                · {{ outgoingTick(m.status) || m.status_label }}
              </template>
            </span>
          </div>
        </div>

        <div
          v-if="messages.length === 0"
          class="text-center text-ink-500 dark:text-ink-300 mt-10"
        >
          No messages.
        </div>
      </template>
    </div>

    <Teleport to="body">
      <div
        v-if="lightboxUrl"
        class="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-6
               cursor-zoom-out"
        @click.self="closeLightbox"
      >
        <button
          type="button"
          class="absolute top-4 right-4 text-white/80 hover:text-white text-2xl leading-none px-2"
          aria-label="Close"
          @click="closeLightbox"
        >
          ×
        </button>
        <img
          :src="lightboxUrl"
          alt=""
          class="max-w-full max-h-full object-contain"
          draggable="false"
        />
      </div>
    </Teleport>
  </div>
</template>
