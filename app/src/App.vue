<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import PairScreen from "./screens/PairScreen.vue";
import ConversationListScreen from "./screens/ConversationListScreen.vue";
import ThreadScreen from "./screens/ThreadScreen.vue";
import { bridge, on } from "./bridge";
import { initStore, teardownStore } from "./store";

type Phase = "loading" | "pairing" | "ready";
const phase = ref<Phase>("loading");
const selectedConversationId = ref<string | null>(null);
const sidebarOpen = ref(true);

const unlisten: (() => void)[] = [];

async function checkStatus() {
  const s = await bridge.status();
  if (s.paired && s.connected) {
    phase.value = "ready";
  } else if (s.paired) {
    // Auth on disk; bridge auto-connects on startup, just wait for `ready`.
    phase.value = "loading";
  } else {
    phase.value = "pairing";
  }
}

watch(phase, async (p) => {
  if (p === "ready") {
    await initStore();
  }
});

onMounted(async () => {
  unlisten.push(await on<void>("ready", () => (phase.value = "ready")));
  await checkStatus();
});

onBeforeUnmount(() => {
  unlisten.forEach((fn) => fn());
  teardownStore();
});
</script>

<template>
  <div
    class="w-screen h-screen bg-paper text-ink-700 dark:text-ink-100
           select-none cursor-default flex"
  >
    <div v-if="phase === 'loading'" class="flex-1 flex items-center justify-center">
      <p class="text-ink-500 dark:text-ink-300">Connecting…</p>
    </div>
    <PairScreen
      v-else-if="phase === 'pairing'"
      class="flex-1"
      @paired="phase = 'ready'"
    />
    <template v-else>
      <!-- Sidebar: conversation list. Fixed-width on desktop, hideable. -->
      <aside
        v-if="sidebarOpen"
        class="w-[340px] flex-shrink-0 border-r border-ink-300/20
               dark:border-ink-300/10 flex flex-col"
      >
        <ConversationListScreen
          class="flex-1 min-h-0"
          :selected-id="selectedConversationId"
          @unpaired="phase = 'pairing'; selectedConversationId = null"
          @open="(id) => (selectedConversationId = id)"
        />
      </aside>

      <!-- Main pane: thread or empty state. -->
      <main class="flex-1 min-w-0 flex flex-col">
        <ThreadScreen
          v-if="selectedConversationId"
          :key="selectedConversationId"
          :conversation-id="selectedConversationId"
          :sidebar-open="sidebarOpen"
          class="flex-1 min-h-0"
          @toggle-sidebar="sidebarOpen = !sidebarOpen"
        />
        <div
          v-else
          class="flex-1 flex flex-col items-center justify-center text-ink-500
                 dark:text-ink-300 gap-3"
        >
          <button
            v-if="!sidebarOpen"
            class="px-3 py-1 rounded-lg border border-ink-300/40
                   dark:border-ink-300/30 hover:bg-accent-100/40
                   dark:hover:bg-accent-900/40 text-sm"
            @click="sidebarOpen = true"
          >
            Show conversations
          </button>
          <p class="text-sm">Select a conversation</p>
        </div>
      </main>
    </template>
  </div>
</template>
