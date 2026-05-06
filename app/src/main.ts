import { createApp } from "vue";
import App from "./App.vue";
import "./styles/main.css";

// Synchronous prefers-color-scheme apply so we don't flash light on dark.
;(() => {
  try {
    if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
      document.documentElement.classList.add("dark");
      document.documentElement.style.colorScheme = "dark";
    }
  } catch { /* noop */ }
})();

createApp(App).mount("#app");
