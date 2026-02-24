import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const app = document.querySelector<HTMLDivElement>("#app")!;
app.textContent = "MuxUX";

// Dismiss the overlay on Escape key
document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    await invoke("mux_hide_overlay");
  }
});

// Dismiss the overlay when the window loses focus (click outside)
const currentWindow = getCurrentWindow();
currentWindow.onFocusChanged(async ({ payload: focused }) => {
  if (!focused) {
    await invoke("mux_hide_overlay");
  }
});
