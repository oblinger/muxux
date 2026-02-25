import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ---------------------------------------------------------------------------
// IPC response shape (mirrors Rust IpcResponse)
// ---------------------------------------------------------------------------
interface IpcResponse {
  ok: boolean;
  data: string;
}

// ---------------------------------------------------------------------------
// Zone definitions
// ---------------------------------------------------------------------------
interface ZoneItem {
  text: string;
  action: string;
  param?: string;
  children?: ZoneItem[];
  icon?: string;  // Layout icon type: "2-col", "3-col", "2-row", "dashboard"
}

interface ZoneConfig {
  direction: "up" | "down" | "left" | "right";
  label: string;
  items: ZoneItem[];
}

const BRICK_THRESHOLD = 6;

const ZONE_LABELS: Record<string, string> = {
  s: "Sessions",
  l: "Layout",
  c: "Catalog",
  m: "Modify",
};

// ---------------------------------------------------------------------------
// Filtering state
// ---------------------------------------------------------------------------
let activeZoneLabel: string | null = null;
let filterText = "";

// ---------------------------------------------------------------------------
// Spotlight search types and state
// ---------------------------------------------------------------------------
interface SearchableItem {
  text: string;
  action: string;
  param?: string;
  category: string;
  element?: HTMLDivElement;
}

const SEARCH_MAX_ROWS = 10;

let spotlightDropdown: HTMLDivElement | null = null;
let spotlightItems: SearchableItem[] = [];
let spotlightSelectedIndex = -1;

const STATIC_ZONES: ZoneConfig[] = [
  {
    direction: "right",
    label: "Layout",
    items: [
      { text: "+Row", action: "layout.row" },
      { text: "+Col", action: "layout.column" },
      { text: "Resize", action: "unimplemented" },
      { text: "Even Out", action: "unimplemented" },
    ],
  },
  {
    direction: "down",
    label: "Catalog",
    items: [
      {
        text: "Projects \u25B8",
        action: "submenu",
        children: [
          { text: "New Project", action: "unimplemented" },
          { text: "Import", action: "unimplemented" },
        ],
      },
      {
        text: "Recent \u25B8",
        action: "submenu",
        children: [
          { text: "(no recent items)", action: "unimplemented" },
        ],
      },
      {
        text: "Templates \u25B8",
        action: "submenu",
        children: [
          { text: "2-Col", action: "unimplemented", icon: "2-col" },
          { text: "3-Col", action: "unimplemented", icon: "3-col" },
          { text: "2-Row", action: "unimplemented", icon: "2-row" },
          { text: "Dashboard", action: "unimplemented", icon: "dashboard" },
        ],
      },
    ],
  },
  {
    direction: "left",
    label: "Modify",
    items: [
      { text: "Delete", action: "unimplemented" },
      { text: "Merge", action: "layout.merge" },
      { text: "Swap", action: "unimplemented" },
      { text: "Detach", action: "unimplemented" },
    ],
  },
];

// ---------------------------------------------------------------------------
// Layout icon helpers
// ---------------------------------------------------------------------------
function getIconPaneCount(icon: string): number {
  switch (icon) {
    case "2-col": return 2;
    case "3-col": return 3;
    case "2-row": return 2;
    case "dashboard": return 4;
    default: return 1;
  }
}

function prependLayoutIcon(container: HTMLElement, icon: string): void {
  const iconEl = document.createElement("span");
  iconEl.className = `layout-icon layout-icon-${icon}`;
  const paneCount = getIconPaneCount(icon);
  for (let i = 0; i < paneCount; i++) {
    const pane = document.createElement("span");
    pane.className = "layout-icon-pane";
    iconEl.appendChild(pane);
  }
  container.prepend(iconEl);
  container.classList.add("has-icon");
}

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------
function createZoneElement(config: ZoneConfig): HTMLDivElement {
  const zone = document.createElement("div");
  zone.className = `zone zone-${config.direction}`;

  // For Up and Left zones, label goes at the end (nearest center).
  // For Down and Right zones, label goes at the start (nearest center).
  const labelEl = document.createElement("div");
  labelEl.className = "zone-label";
  labelEl.textContent = config.label;

  const itemEls: HTMLDivElement[] = config.items.map((zi) => {
    const item = document.createElement("div");
    item.className = "zone-item";
    item.textContent = zi.text;
    item.dataset.action = zi.action;
    if (zi.param !== undefined) {
      item.dataset.param = zi.param;
    }

    if (zi.icon) {
      // textContent was set above; we need to clear and re-add with icon
      item.textContent = "";
      prependLayoutIcon(item, zi.icon);
      const textSpan = document.createElement("span");
      textSpan.textContent = zi.text;
      item.appendChild(textSpan);
    }

    if (zi.children && zi.children.length > 0) {
      item.classList.add("has-submenu");

      const subPanel = document.createElement("div");
      subPanel.className = `sub-panel sub-panel-${config.direction}`;

      zi.children.forEach((child) => {
        const childEl = document.createElement("div");
        childEl.className = "zone-item sub-item";
        childEl.textContent = child.text;
        childEl.dataset.action = child.action;
        if (child.param !== undefined) {
          childEl.dataset.param = child.param;
        }
        if (child.icon) {
          childEl.textContent = "";
          prependLayoutIcon(childEl, child.icon);
          const textSpan = document.createElement("span");
          textSpan.textContent = child.text;
          childEl.appendChild(textSpan);
        }
        subPanel.appendChild(childEl);
      });

      item.appendChild(subPanel);
    }

    return item;
  });

  if (config.direction === "up" || config.direction === "left") {
    // Items first, then label at the bottom (nearest center)
    itemEls.forEach((el) => zone.appendChild(el));
    zone.appendChild(labelEl);
  } else {
    // Label first (nearest center), then items
    zone.appendChild(labelEl);
    itemEls.forEach((el) => zone.appendChild(el));
  }

  // Auto-switch to multi-column brick layout for zones with many items
  if (config.items.length > BRICK_THRESHOLD) {
    zone.classList.add("zone-brick");
  }

  return zone;
}

// ---------------------------------------------------------------------------
// Flash toast for unimplemented items
// ---------------------------------------------------------------------------
function showFlashToast(message: string): void {
  // Remove any existing toast
  const existing = document.querySelector(".flash-toast");
  if (existing) existing.remove();

  const toast = document.createElement("div");
  toast.className = "flash-toast";
  toast.textContent = message;

  const app = document.querySelector<HTMLDivElement>("#app")!;
  app.appendChild(toast);

  // Force reflow so the animation triggers
  void toast.offsetWidth;
  toast.classList.add("flash-toast-visible");

  setTimeout(() => {
    toast.classList.remove("flash-toast-visible");
    toast.classList.add("flash-toast-fading");
    // Remove from DOM after fade-out completes
    setTimeout(() => toast.remove(), 400);
  }, 1500);

  // Refocus the center input after toast
  const centerInput = document.getElementById("center-input") as HTMLInputElement | null;
  if (centerInput) centerInput.focus();
}

// ---------------------------------------------------------------------------
// Execute zone item action
// ---------------------------------------------------------------------------
async function executeAction(action: string, param?: string): Promise<void> {
  switch (action) {
    case "session":
      await invoke("mux_layout_session", { name: param ?? "main" });
      await invoke("mux_hide_overlay");
      break;

    case "layout.row":
      await invoke("mux_layout_row", { session: "current" });
      await invoke("mux_hide_overlay");
      break;

    case "layout.column":
      await invoke("mux_layout_column", { session: "current" });
      await invoke("mux_hide_overlay");
      break;

    case "layout.merge":
      await invoke("mux_layout_merge", { session: "current" });
      await invoke("mux_hide_overlay");
      break;

    case "submenu":
      // Sub-menu items handle their own clicks via children
      // Parent item click does nothing
      break;

    case "unimplemented":
      showFlashToast("Not yet implemented");
      // Do NOT dismiss — let the user try another item
      break;

    default:
      showFlashToast("Unknown action");
      break;
  }
}

// ---------------------------------------------------------------------------
// Handle zone item click with confirmation flash
// ---------------------------------------------------------------------------
function handleItemClick(item: HTMLDivElement): void {
  const action = item.dataset.action;
  if (!action) return;

  const param = item.dataset.param;

  // Add confirmation pulse class
  item.classList.add("zone-item-confirm");

  if (action === "unimplemented") {
    // No delay needed for unimplemented — show toast immediately
    executeAction(action, param);
    // Remove the pulse class after animation completes
    setTimeout(() => item.classList.remove("zone-item-confirm"), 200);
  } else {
    // Brief delay so user sees the confirmation flash before overlay dismisses
    setTimeout(() => {
      executeAction(action, param);
    }, 120);
  }
}

// ---------------------------------------------------------------------------
// Fetch session names from IPC
// ---------------------------------------------------------------------------
async function fetchSessionNames(): Promise<string[]> {
  try {
    const resp: IpcResponse = await invoke("mux_status");
    if (resp.ok && resp.data) {
      // mux_status returns session info as text.
      // Try to extract session names — look for lines that could be session names.
      // Common formats: bare names, "name: ...", JSON with name fields, etc.
      const lines = resp.data.split("\n").filter((l) => l.trim().length > 0);

      // Try JSON parse first
      try {
        const parsed: unknown = JSON.parse(resp.data);
        if (Array.isArray(parsed)) {
          const names = parsed
            .map((entry: unknown) => {
              if (typeof entry === "string") return entry;
              if (
                typeof entry === "object" &&
                entry !== null &&
                "name" in entry
              ) {
                return String((entry as { name: unknown }).name);
              }
              return null;
            })
            .filter((n): n is string => n !== null);
          if (names.length > 0) return names;
        }
      } catch {
        // Not JSON — fall through to line-based parsing
      }

      // Use non-empty lines as session names (trim whitespace)
      if (lines.length > 0) {
        return lines.map((l) => l.trim()).slice(0, 8);
      }
    }
  } catch {
    // IPC not available (e.g., running outside Tauri)
  }
  return [];
}

// ---------------------------------------------------------------------------
// Spotlight search helpers
// ---------------------------------------------------------------------------
function collectSearchableItems(): SearchableItem[] {
  const app = document.querySelector<HTMLDivElement>("#app")!;
  const zones = app.querySelectorAll<HTMLDivElement>(".zone");
  const items: SearchableItem[] = [];

  zones.forEach((zone) => {
    const label = zone.querySelector<HTMLDivElement>(".zone-label");
    const category = label?.textContent?.trim() || "";

    // Collect top-level items (not sub-items)
    const zoneItems = zone.querySelectorAll<HTMLDivElement>(".zone-item:not(.sub-item)");
    zoneItems.forEach((el) => {
      items.push({
        text: el.textContent?.trim() || "",
        action: el.dataset.action || "",
        param: el.dataset.param,
        category,
        element: el,
      });
    });

    // Also collect sub-items
    const subItems = zone.querySelectorAll<HTMLDivElement>(".sub-item");
    subItems.forEach((el) => {
      items.push({
        text: el.textContent?.trim() || "",
        action: el.dataset.action || "",
        param: el.dataset.param,
        category,
        element: el,
      });
    });
  });

  return items;
}

function showSpotlightDropdown(query: string): void {
  const app = document.querySelector<HTMLDivElement>("#app")!;

  // Create dropdown if it doesn't exist
  if (!spotlightDropdown) {
    spotlightDropdown = document.createElement("div");
    spotlightDropdown.className = "spotlight-dropdown";
    app.appendChild(spotlightDropdown);
  }

  // Filter all searchable items
  const allItems = collectSearchableItems();
  const lowerQuery = query.toLowerCase();
  spotlightItems = allItems.filter((item) =>
    item.text.toLowerCase().includes(lowerQuery) && item.action !== "submenu"
  );

  // Limit to max rows
  const displayItems = spotlightItems.slice(0, SEARCH_MAX_ROWS);

  // Build dropdown content
  spotlightDropdown.innerHTML = "";
  spotlightSelectedIndex = -1;

  if (displayItems.length === 0) {
    spotlightDropdown.style.display = "none";
    return;
  }

  spotlightDropdown.style.display = "flex";

  displayItems.forEach((item, index) => {
    const row = document.createElement("div");
    row.className = "spotlight-row";
    row.dataset.index = String(index);

    const nameSpan = document.createElement("span");
    nameSpan.className = "spotlight-name";
    nameSpan.textContent = item.text;

    const catSpan = document.createElement("span");
    catSpan.className = "spotlight-category";
    catSpan.textContent = item.category;

    row.appendChild(nameSpan);
    row.appendChild(catSpan);

    // Click to select
    row.addEventListener("click", (e) => {
      e.stopPropagation();
      selectSpotlightItem(index);
    });

    // Hover highlights
    row.addEventListener("mouseenter", () => {
      setSpotlightSelection(index);
    });

    spotlightDropdown!.appendChild(row);
  });
}

function hideSpotlightDropdown(): void {
  if (spotlightDropdown) {
    spotlightDropdown.style.display = "none";
    spotlightDropdown.innerHTML = "";
  }
  spotlightItems = [];
  spotlightSelectedIndex = -1;
}

function setSpotlightSelection(index: number): void {
  if (!spotlightDropdown) return;

  // Remove previous selection
  const rows = spotlightDropdown.querySelectorAll<HTMLDivElement>(".spotlight-row");
  rows.forEach((row) => row.classList.remove("spotlight-selected"));

  // Set new selection
  if (index >= 0 && index < rows.length) {
    rows[index].classList.add("spotlight-selected");
    spotlightSelectedIndex = index;
  }
}

function selectSpotlightItem(index: number): void {
  if (index < 0 || index >= spotlightItems.length) return;

  const item = spotlightItems[index];
  if (item.element) {
    handleItemClick(item.element);
  } else {
    executeAction(item.action, item.param);
  }
}

// ---------------------------------------------------------------------------
// Zone-letter activation and item filtering
// ---------------------------------------------------------------------------
function applyFilter(inputValue: string): void {
  const app = document.querySelector<HTMLDivElement>("#app")!;
  const zones = app.querySelectorAll<HTMLDivElement>(".zone");

  if (inputValue.length === 0) {
    // No input — show all zones at full opacity, show all items
    activeZoneLabel = null;
    filterText = "";
    zones.forEach((zone) => {
      zone.style.opacity = "1";
      zone.style.display = "";
      const items = zone.querySelectorAll<HTMLDivElement>(".zone-item");
      items.forEach((item) => {
        item.style.display = "";
      });
    });
    hideSpotlightDropdown();
    return;
  }

  const firstChar = inputValue[0].toLowerCase();
  const matchedLabel = ZONE_LABELS[firstChar];

  if (!matchedLabel) {
    // First char doesn't match any zone — dim all zones, show spotlight
    activeZoneLabel = null;
    filterText = inputValue;
    zones.forEach((zone) => {
      zone.style.opacity = "0.15";
    });
    showSpotlightDropdown(inputValue);
    return;
  }

  // First char matches a zone — activate it, dim others
  activeZoneLabel = matchedLabel;
  filterText = inputValue.slice(1);

  zones.forEach((zone) => {
    const label = zone.querySelector<HTMLDivElement>(".zone-label");
    const zoneName = label?.textContent?.trim() || "";

    if (zoneName === matchedLabel) {
      // This is the active zone — show it, filter its items
      zone.style.opacity = "1";

      const items = zone.querySelectorAll<HTMLDivElement>(".zone-item:not(.sub-item)");
      let visibleCount = 0;

      items.forEach((item) => {
        const itemText = item.textContent?.toLowerCase() || "";

        if (filterText.length === 0 || itemText.includes(filterText.toLowerCase())) {
          item.style.display = "";
          visibleCount++;
        } else {
          item.style.display = "none";
        }
      });

      // If all items in the active zone are hidden, hide the zone too
      if (visibleCount === 0) {
        zone.style.display = "none";
        // Zone emptied — show spotlight with remaining filter text
        showSpotlightDropdown(filterText);
      } else {
        zone.style.display = "";
        hideSpotlightDropdown();
      }
    } else {
      // Not the active zone — dim it
      zone.style.opacity = "0.15";
    }
  });
}

// ---------------------------------------------------------------------------
// Build overlay UI
// ---------------------------------------------------------------------------
async function buildOverlay(): Promise<void> {
  activeZoneLabel = null;
  filterText = "";

  const app = document.querySelector<HTMLDivElement>("#app")!;
  app.innerHTML = "";

  // Get target pane context (non-blocking, we don't depend on it yet)
  invoke("mux_get_target_pane").catch(() => {
    // ignore — we don't use the result in Phase 1
  });

  // Center input box (replaces center marker)
  const input = document.createElement("input");
  input.type = "text";
  input.className = "center-input";
  input.id = "center-input";
  input.autocomplete = "off";
  input.spellcheck = false;
  input.placeholder = "";
  app.appendChild(input);

  // Auto-focus the input
  input.focus();

  input.addEventListener("input", () => {
    applyFilter(input.value);
  });

  // Arrow key navigation and Enter for spotlight dropdown
  input.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (spotlightDropdown && spotlightDropdown.style.display !== "none") {
        const maxIndex = Math.min(spotlightItems.length, SEARCH_MAX_ROWS) - 1;
        setSpotlightSelection(Math.min(spotlightSelectedIndex + 1, maxIndex));
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (spotlightDropdown && spotlightDropdown.style.display !== "none") {
        setSpotlightSelection(Math.max(spotlightSelectedIndex - 1, 0));
      }
    } else if (e.key === "Enter") {
      if (spotlightSelectedIndex >= 0) {
        e.preventDefault();
        selectSpotlightItem(spotlightSelectedIndex);
      }
    }
  });

  // Fetch sessions for the Up zone
  let sessionNames = await fetchSessionNames();
  if (sessionNames.length === 0) {
    sessionNames = ["main", "dev"];
  }

  const sessionsZone: ZoneConfig = {
    direction: "up",
    label: "Sessions",
    items: sessionNames.map((name) => ({
      text: name,
      action: "session",
      param: name,
    })),
  };

  // Build all four zones
  app.appendChild(createZoneElement(sessionsZone));
  for (const zone of STATIC_ZONES) {
    app.appendChild(createZoneElement(zone));
  }

  // Attach click handlers to all zone items
  const allItems = app.querySelectorAll<HTMLDivElement>(".zone-item");
  allItems.forEach((item) => {
    item.addEventListener("click", (e: MouseEvent) => {
      e.stopPropagation();
      handleItemClick(item);
    });
  });
}

// ---------------------------------------------------------------------------
// Dismiss handlers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
buildOverlay();
