<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, computed } from "vue";
import { bridge, on } from "../bridge";

const emit = defineEmits<{ (e: "paired"): void }>();

type Phase =
  | "choose"
  | "paste"
  | "submitting"
  | "awaiting_emoji"
  | "completing"
  | "error";

const phase = ref<Phase>("choose");
const emoji = ref<string>("");
const errorMsg = ref<string>("");
const phoneId = ref<string>("");
const cookieText = ref<string>("");

const REQUIRED_COOKIES = ["SAPISID"];

/** Parse pasted cookie input. Accepts JSON or `name=value; name=value` */
function parseCookies(input: string): Record<string, string> {
  const trimmed = input.trim();
  if (!trimmed) return {};

  // JSON object
  if (trimmed.startsWith("{")) {
    const obj = JSON.parse(trimmed);
    if (typeof obj !== "object" || obj === null) {
      throw new Error("JSON must be an object of name → value");
    }
    const out: Record<string, string> = {};
    for (const [k, v] of Object.entries(obj)) {
      if (typeof v === "string") out[k] = v;
      else if (v && typeof v === "object" && "value" in v) {
        out[k] = String((v as { value: unknown }).value);
      }
    }
    return out;
  }

  // Semicolon- or newline-delimited `name=value`
  const out: Record<string, string> = {};
  const parts = trimmed.split(/[;\n]+/);
  for (const part of parts) {
    const eq = part.indexOf("=");
    if (eq === -1) continue;
    const name = part.slice(0, eq).trim();
    const value = part.slice(eq + 1).trim();
    if (name) out[name] = value;
  }
  return out;
}

const parsed = computed<Record<string, string>>(() => {
  try {
    return parseCookies(cookieText.value);
  } catch {
    return {};
  }
});
const parseError = computed<string>(() => {
  if (!cookieText.value.trim()) return "";
  try {
    parseCookies(cookieText.value);
    return "";
  } catch (e) {
    return String(e);
  }
});
const missing = computed<string[]>(() =>
  REQUIRED_COOKIES.filter((k) => !(k in parsed.value))
);
const canSubmit = computed(
  () => Object.keys(parsed.value).length > 0 && missing.value.length === 0 && !parseError.value
);

async function submitCookies() {
  phase.value = "submitting";
  errorMsg.value = "";
  try {
    await bridge.pairWithCookies(parsed.value);
    // Bridge will emit gaia_emoji → paired → ready in sequence.
  } catch (e) {
    errorMsg.value = String(e);
    phase.value = "error";
  }
}

const unlisten: (() => void)[] = [];

onMounted(async () => {
  unlisten.push(
    await on<{ emoji: string }>("gaia_emoji", (p) => {
      emoji.value = p.emoji;
      phase.value = "awaiting_emoji";
    })
  );
  unlisten.push(
    await on<{ phone_id: string }>("paired", (p) => {
      phoneId.value = p.phone_id;
      phase.value = "completing";
    })
  );
  unlisten.push(await on<void>("ready", () => emit("paired")));
  unlisten.push(
    await on<{ kind: string; msg: string }>("error", (p) => {
      errorMsg.value = `${p.kind}: ${p.msg}`;
      phase.value = "error";
    })
  );
});

onBeforeUnmount(() => unlisten.forEach((fn) => fn()));
</script>

<template>
  <div class="w-full h-full flex items-center justify-center p-8 overflow-y-auto">
    <div class="paper-card rounded-xl p-10 max-w-xl w-full">
      <h1 class="text-3xl font-serif text-accent-700 dark:text-accent-300 mb-2 text-center">
        Pair your phone
      </h1>

      <!-- Choose: only path that actually works for now is paste. -->
      <div v-if="phase === 'choose'" class="space-y-6 text-center">
        <p class="text-sm text-ink-500 dark:text-ink-300">
          Google blocks sign-in from this app's webview, so we pair using
          cookies copied from a browser where you're already signed in.
        </p>
        <button
          class="px-6 py-3 rounded-lg bg-accent-600 hover:bg-accent-500
                 text-surface-0 font-medium transition-colors"
          @click="phase = 'paste'"
        >
          Paste cookies from browser
        </button>
        <p class="text-xs text-ink-500 dark:text-ink-300">
          Need help finding them?
          <a
            href="https://github.com/Slush97/lumina-rcs/blob/master/docs/COOKIE_PAIRING.md"
            target="_blank"
            class="text-accent-700 dark:text-accent-300 hover:underline"
          >
            See the cookie-pairing guide
          </a>
        </p>
      </div>

      <!-- Paste: textarea + parse feedback. -->
      <div v-else-if="phase === 'paste'" class="space-y-4">
        <p class="text-sm text-ink-500 dark:text-ink-300">
          Paste cookies for <code>google.com</code> below. Either JSON
          (<code>{ "SAPISID": "...", ... }</code>) or
          <code>name=value; name=value</code> as Chrome's DevTools gives
          you.
        </p>
        <textarea
          v-model="cookieText"
          rows="8"
          spellcheck="false"
          autocomplete="off"
          placeholder="SAPISID=...; HSID=...; SSID=...; SID=...; APISID=..."
          class="w-full rounded-lg p-3 paper-card font-mono text-xs
                 text-ink-700 dark:text-ink-100
                 focus:outline-none focus:ring-2 focus:ring-accent-500"
        />
        <div class="flex items-center justify-between text-xs">
          <span v-if="parseError" class="text-red-700 dark:text-red-400">
            {{ parseError }}
          </span>
          <span
            v-else-if="cookieText && missing.length"
            class="text-amber-700 dark:text-amber-400"
          >
            Missing required cookie: {{ missing.join(", ") }}
          </span>
          <span v-else-if="cookieText" class="text-ink-500 dark:text-ink-300">
            Parsed {{ Object.keys(parsed).length }} cookie(s)
          </span>
          <span v-else></span>
          <div class="flex gap-2">
            <button
              class="px-3 py-1 rounded-lg border border-ink-300/40 dark:border-ink-300/30
                     text-ink-700 dark:text-ink-100
                     hover:bg-accent-100/40 dark:hover:bg-accent-900/40 transition-colors"
              @click="phase = 'choose'; cookieText = ''"
            >
              Back
            </button>
            <button
              :disabled="!canSubmit"
              class="px-4 py-1 rounded-lg bg-accent-600 hover:bg-accent-500
                     text-surface-0 font-medium transition-colors
                     disabled:opacity-40 disabled:cursor-not-allowed"
              @click="submitCookies"
            >
              Pair
            </button>
          </div>
        </div>
      </div>

      <!-- Submitting: bridge has accepted, waiting for emoji. -->
      <div
        v-else-if="phase === 'submitting'"
        class="text-ink-500 dark:text-ink-300 text-center"
      >
        Sending cookies to Google…
      </div>

      <!-- Got emoji: prompt user to tap matching one on phone. -->
      <div
        v-else-if="phase === 'awaiting_emoji'"
        class="flex flex-col items-center gap-4"
      >
        <div
          class="text-7xl leading-none p-6 rounded-2xl paper-card"
          style="font-family: 'Apple Color Emoji', 'Segoe UI Emoji',
                 'Noto Color Emoji', sans-serif"
        >
          {{ emoji }}
        </div>
        <p class="text-base text-ink-700 dark:text-ink-100 font-medium">
          Tap this emoji on your phone
        </p>
        <p class="text-xs text-ink-500 dark:text-ink-300">
          Google Messages should be showing three emojis on your phone now.
        </p>
      </div>

      <!-- Pair confirmed; loading initial conversations. -->
      <div
        v-else-if="phase === 'completing'"
        class="flex flex-col items-center gap-2"
      >
        <p class="text-accent-700 dark:text-accent-300 font-medium">
          Phone paired
        </p>
        <p
          v-if="phoneId"
          class="text-xs text-ink-500 dark:text-ink-300 break-all"
        >
          {{ phoneId }}
        </p>
        <p class="text-xs text-ink-500 dark:text-ink-300 mt-2">
          Loading conversations…
        </p>
      </div>

      <!-- Error: show + offer retry. -->
      <div v-else class="flex flex-col items-center gap-4">
        <p class="text-red-700 dark:text-red-400 text-sm">{{ errorMsg }}</p>
        <button
          class="px-4 py-2 rounded-lg border border-ink-300/40 dark:border-ink-300/30
                 text-ink-700 dark:text-ink-100
                 hover:bg-accent-100/40 dark:hover:bg-accent-900/40 transition-colors"
          @click="phase = 'choose'"
        >
          Try again
        </button>
      </div>
    </div>
  </div>
</template>
