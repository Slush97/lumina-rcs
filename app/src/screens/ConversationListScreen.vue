<script setup lang="ts">
import { onMounted, ref } from "vue";
import { bridge } from "../bridge";
import { conversations, loadInbox } from "../store";

defineProps<{ selectedId?: string | null }>();
const emit = defineEmits<{
  (e: "unpaired"): void;
  (e: "open", id: string): void;
}>();

const loading = ref(true);
const errorMsg = ref<string>("");

async function refresh() {
  try {
    await loadInbox(50);
    loading.value = false;
  } catch (e) {
    errorMsg.value = String(e);
    loading.value = false;
  }
}

function fmtTimestamp(microsOrMillis: number): string {
  if (!microsOrMillis) return "";
  // libgm reports microseconds; collapse to ms.
  const ms = microsOrMillis > 1e14 ? microsOrMillis / 1000 : microsOrMillis;
  const d = new Date(ms);
  const today = new Date();
  if (d.toDateString() === today.toDateString()) {
    return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

async function unpair() {
  if (!confirm("Unpair this phone? Saved auth will be deleted.")) return;
  await bridge.unpair();
  emit("unpaired");
}

onMounted(refresh);
</script>

<template>
  <div class="w-full h-full flex flex-col">
    <header class="paper-card rounded-none border-x-0 border-t-0 px-6 py-3 flex items-center">
      <h1 class="text-xl font-serif text-accent-700 dark:text-accent-300 flex-1">
        Lumina
      </h1>
      <button
        class="text-xs text-ink-500 hover:text-accent-700 dark:text-ink-300 dark:hover:text-accent-300"
        @click="unpair"
      >
        Unpair
      </button>
    </header>

    <div class="flex-1 overflow-y-auto p-4 space-y-2">
      <div v-if="loading" class="text-center text-ink-500 dark:text-ink-300 mt-10">
        Loading conversations…
      </div>
      <div v-else-if="errorMsg" class="text-center text-red-700 dark:text-red-400 mt-10">
        {{ errorMsg }}
      </div>
      <div v-else-if="conversations.length === 0" class="text-center text-ink-500 dark:text-ink-300 mt-10">
        No conversations yet.
      </div>
      <button
        v-for="conv in conversations"
        :key="conv.id"
        type="button"
        class="rounded-lg px-3 py-2 flex items-center gap-3 w-full text-left
               transition-colors"
        :class="conv.id === selectedId
          ? 'bg-accent-100/60 dark:bg-accent-900/40'
          : 'hover:bg-accent-100/30 dark:hover:bg-accent-900/20'"
        @click="emit('open', conv.id)"
      >
        <div
          class="w-10 h-10 rounded-full flex-shrink-0 flex items-center justify-center text-sm font-medium text-surface-0"
          :style="{ backgroundColor: conv.avatar_color || '#a05a2b' }"
        >
          {{ (conv.name || "?").charAt(0).toUpperCase() }}
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-baseline gap-2">
            <p class="font-medium text-ink-700 dark:text-ink-100 truncate">
              {{ conv.name || "(no name)" }}
            </p>
            <span class="text-xs text-ink-500 dark:text-ink-300 ml-auto flex-shrink-0">
              {{ fmtTimestamp(conv.timestamp) }}
            </span>
          </div>
          <p
            class="text-sm truncate"
            :class="conv.unread
              ? 'text-ink-700 dark:text-ink-100 font-medium'
              : 'text-ink-500 dark:text-ink-300'"
          >
            <span v-if="conv.snippet_self" class="text-ink-500 dark:text-ink-300">You: </span>
            {{ conv.snippet || "(no preview)" }}
          </p>
        </div>
      </button>
    </div>
  </div>
</template>
