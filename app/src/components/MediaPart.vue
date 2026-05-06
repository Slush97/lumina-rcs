<script setup lang="ts">
import { computed, ref } from "vue";
import { mediaUrl } from "../store";

const props = defineProps<{
  mediaId: string;
  /** From the gmessages proto. 0 means unknown — we fall back to a
   *  fixed aspect ratio so the bubble doesn't reflow when the media
   *  finally decodes. */
  width: number;
  height: number;
  mime: string;
  name: string;
}>();

const emit = defineEmits<{ (e: "openImage", url: string): void }>();

// Bumped to force the browser to refetch on retry (cache-busting query).
const reloadKey = ref(0);
const errored = ref(false);

const url = computed(() => {
  const base = mediaUrl(props.mediaId);
  return reloadKey.value === 0 ? base : `${base}?retry=${reloadKey.value}`;
});

const family = computed<"image" | "video" | "audio" | "other">(() => {
  if (props.mime.startsWith("image/")) return "image";
  if (props.mime.startsWith("video/")) return "video";
  if (props.mime.startsWith("audio/")) return "audio";
  return "other";
});

const aspectStyle = computed<Record<string, string>>(() => {
  const w = props.width;
  const h = props.height;
  if (w > 0 && h > 0) return { aspectRatio: `${w} / ${h}` };
  return { aspectRatio: "4 / 3" };
});

function onError(e: Event) {
  console.warn("[MediaPart] failed to load", {
    mediaId: props.mediaId,
    mime: props.mime,
    url: url.value,
    target: e.target,
  });
  errored.value = true;
}
function retry() {
  errored.value = false;
  reloadKey.value += 1;
}
function openImage() {
  emit("openImage", url.value);
}
</script>

<template>
  <!-- Image -->
  <div
    v-if="family === 'image'"
    class="rounded-lg overflow-hidden bg-ink-300/20 dark:bg-ink-300/10
           max-w-[280px] w-full relative"
    :style="aspectStyle"
  >
    <img
      v-if="!errored"
      :key="reloadKey"
      :src="url"
      :alt="name || 'image'"
      loading="lazy"
      decoding="async"
      draggable="false"
      class="w-full h-full object-cover cursor-zoom-in"
      @click="openImage"
      @error="onError"
    />
    <button
      v-else
      type="button"
      class="absolute inset-0 flex items-center justify-center text-xs
             text-red-700 dark:text-red-400 bg-paper/40
             hover:bg-paper/60 transition-colors"
      @click="retry"
    >
      Couldn't load — tap to retry
    </button>
  </div>

  <!-- Video -->
  <div
    v-else-if="family === 'video'"
    class="rounded-lg overflow-hidden bg-black/20 max-w-[320px] w-full relative"
    :style="aspectStyle"
  >
    <video
      v-if="!errored"
      :key="reloadKey"
      :src="url"
      controls
      preload="none"
      class="w-full h-full object-contain bg-black"
      @error="onError"
    />
    <button
      v-else
      type="button"
      class="absolute inset-0 flex items-center justify-center text-xs
             text-red-700 dark:text-red-400 bg-paper/40
             hover:bg-paper/60 transition-colors"
      @click="retry"
    >
      Couldn't load — tap to retry
    </button>
  </div>

  <!-- Audio -->
  <div v-else-if="family === 'audio'" class="max-w-[320px]">
    <audio
      v-if="!errored"
      :key="reloadKey"
      :src="url"
      controls
      preload="metadata"
      class="w-full"
      @error="onError"
    />
    <button
      v-else
      type="button"
      class="px-3 py-2 rounded-lg text-xs text-red-700 dark:text-red-400
             border border-red-700/30 hover:bg-red-700/10 transition-colors"
      @click="retry"
    >
      Couldn't load audio — retry
    </button>
    <p class="text-[10px] opacity-60 mt-1 truncate">
      {{ name || mime }}
    </p>
  </div>

  <!-- Other (PDF, vCard, docx, …) -->
  <div
    v-else
    class="px-3 py-2 rounded-lg border border-ink-300/30
           text-xs flex items-center gap-2 max-w-[280px]"
  >
    <span aria-hidden="true">📎</span>
    <span class="truncate">{{ name || mime || "attachment" }}</span>
  </div>
</template>
