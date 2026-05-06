<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import PairScreen from "./screens/PairScreen.vue";
import ConversationListScreen from "./screens/ConversationListScreen.vue";
import { bridge, on } from "./bridge";

type Phase = "loading" | "pairing" | "ready";
const phase = ref<Phase>("loading");

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

onMounted(async () => {
  unlisten.push(await on<void>("ready", () => (phase.value = "ready")));
  await checkStatus();
});

onBeforeUnmount(() => unlisten.forEach((fn) => fn()));
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
    <ConversationListScreen
      v-else
      class="flex-1"
      @unpaired="phase = 'pairing'"
    />
  </div>
</template>
