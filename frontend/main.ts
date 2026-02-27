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
const LR_PUSH = 80; // must match CSS --lr-push

const ZONE_LABELS: Record<string, string> = {
  s: "Sessions",
  l: "Layout",
  c: "Catalog",
  m: "Modify",
};

// ---------------------------------------------------------------------------
// Filtering state
// ---------------------------------------------------------------------------
// Exported so TypeScript doesn't complain about unused writes.
// These track current filter state for future use (e.g., status display).
export let activeZoneLabel: string | null = null;
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

const DEFAULT_SEARCH_MAX_ROWS = 10;
const DEFAULT_ZONE_MAX_WIDTH = 160;
const DEFAULT_LR_SLIDE_START = 5;
const DEFAULT_LR_SLIDE_FULL = 40;

let searchMaxRows = DEFAULT_SEARCH_MAX_ROWS;
let lrSlideStart = DEFAULT_LR_SLIDE_START;
let lrSlideFull = DEFAULT_LR_SLIDE_FULL;

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
      {
        text: "Resize \u25B8",
        action: "submenu",
        children: [
          { text: "Grow Left", action: "layout.resize", param: "left" },
          { text: "Grow Right", action: "layout.resize", param: "right" },
          { text: "Grow Up", action: "layout.resize", param: "up" },
          { text: "Grow Down", action: "layout.resize", param: "down" },
        ],
      },
      { text: "Even Out", action: "layout.even_out" },
    ],
  },
  {
    direction: "down",
    label: "Catalog",
    items: [
      {
        text: "Agents \u25B8",
        action: "submenu",
        children: [
          { text: "(loading...)", action: "unimplemented" },
        ],
      },
      {
        text: "Compositions \u25B8",
        action: "submenu",
        children: [
          { text: "(loading...)", action: "unimplemented" },
        ],
      },
      {
        text: "Sessions \u25B8",
        action: "submenu",
        children: [
          { text: "(loading...)", action: "unimplemented" },
        ],
      },
      {
        text: "Templates \u25B8",
        action: "submenu",
        children: [
          { text: "2-Col", action: "template.apply", param: "2-col", icon: "2-col" },
          { text: "3-Col", action: "template.apply", param: "3-col", icon: "3-col" },
          { text: "2-Row", action: "template.apply", param: "2-row", icon: "2-row" },
          { text: "Dashboard", action: "template.apply", param: "dashboard", icon: "dashboard" },
        ],
      },
    ],
  },
  {
    direction: "left",
    label: "Modify",
    items: [
      { text: "Delete", action: "layout.kill_pane" },
      { text: "Merge", action: "layout.merge" },
      {
        text: "Swap \u25B8",
        action: "submenu",
        children: [
          { text: "Swap Up", action: "layout.swap_pane", param: "up" },
          { text: "Swap Down", action: "layout.swap_pane", param: "down" },
        ],
      },
      { text: "Detach", action: "layout.break_pane" },
      { text: "Capture", action: "layout.capture_save" },
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

/** Invoke an IPC command, log the result, show errors as a toast, then dismiss. */
async function execAndDismiss(
  ipcName: string,
  args: Record<string, unknown>,
  label: string,
): Promise<void> {
  try {
    const resp: IpcResponse = await invoke(ipcName, args);
    console.log(`[mux-ipc] ${label}: ok=${resp.ok} data=${resp.data}`);
    if (!resp.ok) {
      showFlashToast(resp.data || `${label} failed`);
      // Delay dismiss so user can read the error
      setTimeout(() => invoke("mux_hide_overlay"), 2000);
      return;
    }
  } catch (err) {
    console.error(`[mux-ipc] ${label} exception:`, err);
    showFlashToast(`${label}: ${err}`);
    setTimeout(() => invoke("mux_hide_overlay"), 2000);
    return;
  }
  await invoke("mux_hide_overlay");
}

async function executeAction(action: string, param?: string): Promise<void> {
  switch (action) {
    case "session":
      await execAndDismiss("mux_session_switch", { name: param ?? "main" }, "session_switch");
      break;

    case "layout.row":
      await execAndDismiss("mux_layout_row", { session: "current" }, "layout_row");
      break;

    case "layout.column":
      await execAndDismiss("mux_layout_column", { session: "current" }, "layout_column");
      break;

    case "layout.merge":
      await execAndDismiss("mux_layout_merge", { session: "current" }, "layout_merge");
      break;

    case "layout.resize":
      await execAndDismiss("mux_layout_resize", { direction: param ?? "right" }, "layout_resize");
      break;

    case "layout.even_out":
      await execAndDismiss("mux_layout_even_out", {}, "layout_even_out");
      break;

    case "layout.kill_pane":
      await execAndDismiss("mux_layout_kill_pane", {}, "layout_kill_pane");
      break;

    case "layout.swap_pane":
      await execAndDismiss("mux_layout_swap_pane", { direction: param ?? "down" }, "layout_swap_pane");
      break;

    case "layout.break_pane":
      await execAndDismiss("mux_layout_break_pane", {}, "layout_break_pane");
      break;

    case "template.apply":
      await execAndDismiss("mux_template_apply", { template: param ?? "2-col" }, "template_apply");
      break;

    case "parts.place":
      await execAndDismiss("mux_parts_place", { part: param ?? "" }, "parts_place");
      break;

    case "layout.capture_save": {
      // Prompt for a name, then capture and save
      const name = prompt("Save layout as:");
      if (name) {
        try {
          const resp: IpcResponse = await invoke("mux_layout_capture_save", { name });
          console.log(`[mux-ipc] capture_save: ok=${resp.ok} data=${resp.data}`);
          if (resp.ok) {
            showFlashToast(`Layout '${name}' saved`);
          } else {
            showFlashToast(resp.data || "Capture failed");
          }
        } catch (err) {
          console.error("[mux-ipc] capture_save exception:", err);
          showFlashToast(`Capture error: ${err}`);
        }
      }
      await invoke("mux_hide_overlay");
      break;
    }

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
    const resp: IpcResponse = await invoke("mux_session_list");
    if (resp.ok && resp.data) {
      const parsed: unknown = JSON.parse(resp.data);
      if (Array.isArray(parsed)) {
        const names = parsed
          .map((entry: unknown) => {
            if (typeof entry === "object" && entry !== null && "name" in entry) {
              return String((entry as { name: unknown }).name);
            }
            return null;
          })
          .filter((n): n is string => n !== null);
        if (names.length > 0) return names;
      }
    }
  } catch {
    // IPC not available (e.g., running outside Tauri)
  }
  return [];
}

// ---------------------------------------------------------------------------
// Fetch parts catalog from IPC
// ---------------------------------------------------------------------------
interface PartsCatalog {
  agents: { name: string; role?: string }[];
  compositions: { name: string }[];
  sessions: { name: string }[];
}

async function fetchPartsCatalog(): Promise<PartsCatalog | null> {
  try {
    const resp: IpcResponse = await invoke("mux_parts_list");
    if (resp.ok && resp.data) {
      return JSON.parse(resp.data) as PartsCatalog;
    }
  } catch {
    // IPC not available
  }
  return null;
}

function populateCatalogZone(catalog: PartsCatalog): void {
  const app = document.querySelector<HTMLDivElement>("#app");
  if (!app) return;

  const downZone = app.querySelector<HTMLDivElement>(".zone-down");
  if (!downZone) return;

  // Find the Agents, Compositions, and Sessions sub-menus
  const subMenus = downZone.querySelectorAll<HTMLDivElement>(".zone-item.has-submenu");
  subMenus.forEach((menuItem) => {
    const label = menuItem.childNodes[0]?.textContent?.trim() || "";
    const subPanel = menuItem.querySelector<HTMLDivElement>(".sub-panel");
    if (!subPanel) return;

    let items: { name: string }[] = [];
    if (label.startsWith("Agents")) {
      items = catalog.agents;
    } else if (label.startsWith("Compositions")) {
      items = catalog.compositions;
    } else if (label.startsWith("Sessions")) {
      items = catalog.sessions;
    } else {
      return; // Templates — leave as-is
    }

    // Clear placeholder children
    subPanel.innerHTML = "";

    if (items.length === 0) {
      const empty = document.createElement("div");
      empty.className = "zone-item sub-item";
      empty.textContent = "(none)";
      empty.dataset.action = "unimplemented";
      subPanel.appendChild(empty);
      return;
    }

    items.forEach((part) => {
      const childEl = document.createElement("div");
      childEl.className = "zone-item sub-item";
      childEl.textContent = part.name;
      childEl.dataset.action = "parts.place";
      childEl.dataset.param = part.name;
      childEl.addEventListener("click", (e: MouseEvent) => {
        e.stopPropagation();
        handleItemClick(childEl);
      });
      subPanel.appendChild(childEl);
    });
  });
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
  const displayItems = spotlightItems.slice(0, searchMaxRows);

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
    // No input — hide input, restore star, hand visibility back to mouse tracking
    activeZoneLabel = null;
    filterText = "";
    const input = document.getElementById("center-input") as HTMLInputElement | null;
    const star = document.getElementById("center-star");
    if (input) input.style.display = "none";
    if (star) star.style.display = "";
    zones.forEach((zone) => {
      zone.style.display = "";
      zone.style.opacity = "";
      zone.style.pointerEvents = "auto";
      const items = zone.querySelectorAll<HTMLDivElement>(".zone-item");
      items.forEach((item) => {
        item.style.display = "";
      });
    });
    // Reset to mouse-driven zone visibility
    resetMousePhase();
    setVisibleZone("all");
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

  // Stretch keyboard-selected L/R zone inner edge to overlap position
  const app2 = document.querySelector<HTMLDivElement>("#app");
  if (app2) {
    const leftZone = app2.querySelector<HTMLDivElement>(".zone-left");
    const rightZone = app2.querySelector<HTMLDivElement>(".zone-right");
    if (matchedLabel === "Modify" && leftZone) {
      stretchZone(leftZone, "left", LR_PUSH);
    } else if (leftZone) {
      resetZone(leftZone, "left");
    }
    if (matchedLabel === "Layout" && rightZone) {
      stretchZone(rightZone, "right", LR_PUSH);
    } else if (rightZone) {
      resetZone(rightZone, "right");
    }
  }

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
// Center star indicator (shown before first keystroke)
// ---------------------------------------------------------------------------
function createCenterStar(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "90");
  svg.setAttribute("height", "70");
  svg.setAttribute("viewBox", "-45 -35 90 70");
  svg.classList.add("center-star");
  svg.id = "center-star";

  const fill = "rgba(255,255,255,0.4)";

  // Four triangular spokes pointing outward
  // Right triangle — wide at center, point at edge
  const right = document.createElementNS(ns, "polygon");
  right.setAttribute("points", "10,-6 45,0 10,6");
  right.setAttribute("fill", fill);
  svg.appendChild(right);

  // Left triangle
  const left = document.createElementNS(ns, "polygon");
  left.setAttribute("points", "-10,-6 -45,0 -10,6");
  left.setAttribute("fill", fill);
  svg.appendChild(left);

  // Up triangle
  const up = document.createElementNS(ns, "polygon");
  up.setAttribute("points", "-5,-10 0,-35 5,-10");
  up.setAttribute("fill", fill);
  svg.appendChild(up);

  // Down triangle
  const down = document.createElementNS(ns, "polygon");
  down.setAttribute("points", "-5,10 0,35 5,10");
  down.setAttribute("fill", fill);
  svg.appendChild(down);

  // Center circle (larger)
  const circle = document.createElementNS(ns, "circle");
  circle.setAttribute("cx", "0");
  circle.setAttribute("cy", "0");
  circle.setAttribute("r", "8");
  circle.setAttribute("fill", "rgba(255,255,255,0.45)");
  svg.appendChild(circle);

  return svg;
}

// ---------------------------------------------------------------------------
// Left/right zone stretch helpers
// ---------------------------------------------------------------------------
// Natural border-box widths cached after build (box-sizing: border-box on L/R)
const lrNatural = { left: 0, right: 0 };

function stretchZone(
  zone: HTMLDivElement,
  side: "left" | "right",
  stretch: number,
): void {
  const pushRemaining = LR_PUSH - stretch;
  const sign = side === "left" ? -1 : 1;
  const natural = side === "left" ? lrNatural.left : lrNatural.right;

  zone.style.transform = `translateY(-50%) translateX(${sign * pushRemaining}px)`;
  zone.style.minWidth = `${natural + stretch}px`;
  zone.style.alignItems = stretch > 0 ? "stretch" : "";
}

function resetZone(zone: HTMLDivElement, side: "left" | "right"): void {
  const sign = side === "left" ? -1 : 1;
  zone.style.transform = `translateY(-50%) translateX(${sign * LR_PUSH}px)`;
  zone.style.minWidth = "";
  zone.style.alignItems = "";
}

function updateLRSlide(mx: number, _my: number): void {
  const app = document.querySelector<HTMLDivElement>("#app");
  if (!app) return;

  const dx = mx - CENTER;
  const absDx = Math.abs(dx);

  if (absDx <= lrSlideStart) {
    resetLRSlide();
    return;
  }

  const range = lrSlideFull - lrSlideStart;
  const t = Math.min((absDx - lrSlideStart) / range, 1);
  const stretch = LR_PUSH * t;

  const leftZone = app.querySelector<HTMLDivElement>(".zone-left");
  const rightZone = app.querySelector<HTMLDivElement>(".zone-right");

  if (dx < 0 && leftZone) {
    stretchZone(leftZone, "left", stretch);
  } else if (leftZone) {
    resetZone(leftZone, "left");
  }

  if (dx > 0 && rightZone) {
    stretchZone(rightZone, "right", stretch);
  } else if (rightZone) {
    resetZone(rightZone, "right");
  }
}

function resetLRSlide(): void {
  const app = document.querySelector<HTMLDivElement>("#app");
  if (!app) return;
  const leftZone = app.querySelector<HTMLDivElement>(".zone-left");
  const rightZone = app.querySelector<HTMLDivElement>(".zone-right");
  if (leftZone) resetZone(leftZone, "left");
  if (rightZone) resetZone(rightZone, "right");
}

// ---------------------------------------------------------------------------
// Build overlay UI
// ---------------------------------------------------------------------------
async function buildOverlay(): Promise<void> {
  activeZoneLabel = null;
  filterText = "";
  resetMousePhase();

  // Fetch settings from backend; fall back to defaults on failure
  try {
    const settingsResp: IpcResponse = await invoke("mux_get_settings");
    if (settingsResp.ok && settingsResp.data) {
      const s = JSON.parse(settingsResp.data);
      if (typeof s.search_max_rows === "number") {
        searchMaxRows = s.search_max_rows;
      }
      if (typeof s.zone_max_width === "number") {
        document.documentElement.style.setProperty(
          "--zone-max-width",
          s.zone_max_width + "px",
        );
      }
      if (typeof s.lr_slide_start === "number") {
        lrSlideStart = s.lr_slide_start;
      }
      if (typeof s.lr_slide_full === "number") {
        lrSlideFull = s.lr_slide_full;
      }
    }
  } catch {
    // IPC not available — apply defaults explicitly
    searchMaxRows = DEFAULT_SEARCH_MAX_ROWS;
    lrSlideStart = DEFAULT_LR_SLIDE_START;
    lrSlideFull = DEFAULT_LR_SLIDE_FULL;
    document.documentElement.style.setProperty(
      "--zone-max-width",
      DEFAULT_ZONE_MAX_WIDTH + "px",
    );
  }

  const app = document.querySelector<HTMLDivElement>("#app")!;
  app.innerHTML = "";

  // Get target pane context (non-blocking, we don't depend on it yet)
  invoke("mux_get_target_pane").catch(() => {
    // ignore — we don't use the result in Phase 1
  });

  // Center star (visible initially, replaced by input on first keystroke)
  const star = createCenterStar();
  app.appendChild(star);

  // Center input box (hidden until first keystroke)
  const input = document.createElement("input");
  input.type = "text";
  input.className = "center-input";
  input.id = "center-input";
  input.autocomplete = "off";
  input.spellcheck = false;
  input.placeholder = "";
  input.style.display = "none";
  app.appendChild(input);

  input.addEventListener("input", () => {
    applyFilter(input.value);
  });

  // Arrow key navigation and Enter for spotlight dropdown
  input.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (spotlightDropdown && spotlightDropdown.style.display !== "none") {
        const maxIndex = Math.min(spotlightItems.length, searchMaxRows) - 1;
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

  // Populate catalog zone with parts from backend (non-blocking)
  fetchPartsCatalog().then((catalog) => {
    if (catalog) {
      populateCatalogZone(catalog);
    }
  });

  // Cache natural border-box widths of L/R zones (after layout)
  requestAnimationFrame(() => {
    const leftZone = app.querySelector<HTMLDivElement>(".zone-left");
    const rightZone = app.querySelector<HTMLDivElement>(".zone-right");
    if (leftZone) lrNatural.left = leftZone.offsetWidth;
    if (rightZone) lrNatural.right = rightZone.offsetWidth;

    // Diagnostic: log every zone-item's pixel dimensions
    const allZoneItems = app.querySelectorAll<HTMLDivElement>(".zone-item:not(.sub-item)");
    allZoneItems.forEach((item) => {
      const r = item.getBoundingClientRect();
      const zone = item.closest(".zone");
      const zoneClass = zone ? zone.className.replace("zone ", "") : "?";
      console.log(`[mux-diag] item="${item.textContent?.trim()}" zone=${zoneClass} w=${Math.round(r.width)} h=${Math.round(r.height)}`);
    });
    // Also log zone dimensions
    const allZones = app.querySelectorAll<HTMLDivElement>(".zone");
    allZones.forEach((zone) => {
      const r = zone.getBoundingClientRect();
      console.log(`[mux-diag] zone=${zone.className} w=${Math.round(r.width)} h=${Math.round(r.height)}`);
    });
  });
}

// ---------------------------------------------------------------------------
// Mouse-driven zone visibility
// ---------------------------------------------------------------------------
const APP_SIZE = 700;
const CENTER = APP_SIZE / 2;
const DEADZONE = 30;            // px from center — show all zones
const DISMISS_MARGIN = 50;      // px from nearest content rect → dismiss

type Direction = "up" | "down" | "left" | "right";
type ZoneVisibility = Direction | "all" | "none";
let visibleZone: ZoneVisibility = "all";

// Three-state mouse lifecycle:
//   "pristine"  — overlay just opened, mouse hasn't entered yet, show all zones
//   "tracking"  — mouse is inside the region, directional highlighting active
//   "dismissed" — mouse entered then left, overlay dismissed permanently
let mousePhase: "pristine" | "tracking" | "dismissed" = "pristine";

function resetMousePhase(): void {
  mousePhase = "pristine";
  visibleZone = "all";
  resetLRSlide();
}

/** Minimum pixel distance from (cx, cy) to the nearest visible content rect. */
function distToNearestContent(cx: number, cy: number): number {
  const selectors = ".zone, .sub-panel, .center-input, .spotlight-dropdown, .center-star";
  const els = document.querySelectorAll<HTMLElement>(selectors);
  let minDist = Infinity;

  els.forEach((el) => {
    // Skip hidden elements
    const style = getComputedStyle(el);
    if (style.display === "none" || style.opacity === "0") return;
    // sub-panels are display:none until hovered — skip those too
    if (el.classList.contains("sub-panel") && style.display === "none") return;

    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;

    // Distance from point to axis-aligned rect
    const dx = Math.max(r.left - cx, 0, cx - r.right);
    const dy = Math.max(r.top - cy, 0, cy - r.bottom);
    const d = Math.sqrt(dx * dx + dy * dy);
    if (d < minDist) minDist = d;
  });

  return minDist;
}

function directionalZone(mx: number, my: number): ZoneVisibility {
  const dx = mx - CENTER;
  const dy = my - CENTER;

  if (Math.abs(dx) < DEADZONE && Math.abs(dy) < DEADZONE) return "all";

  if (Math.abs(dx) > Math.abs(dy)) {
    return dx > 0 ? "right" : "left";
  } else {
    return dy > 0 ? "down" : "up";
  }
}

function setVisibleZone(vis: ZoneVisibility): void {
  if (vis === visibleZone) return;
  visibleZone = vis;

  const app = document.querySelector<HTMLDivElement>("#app");
  if (!app) return;

  const zones = app.querySelectorAll<HTMLDivElement>(".zone");
  const centerInput = document.getElementById("center-input") as HTMLElement | null;
  const centerStar = document.getElementById("center-star") as HTMLElement | null;

  if (vis === "none") {
    zones.forEach((zone) => {
      zone.style.opacity = "0";
      zone.style.pointerEvents = "none";
    });
    if (centerInput) centerInput.style.opacity = "0";
    if (centerStar) centerStar.style.opacity = "0";
  } else if (vis === "all") {
    zones.forEach((zone) => {
      zone.style.opacity = "0.6";
      zone.style.pointerEvents = "auto";
    });
    if (centerInput) centerInput.style.opacity = "1";
    if (centerStar) centerStar.style.opacity = "1";
  } else {
    zones.forEach((zone) => {
      const isMatch = zone.classList.contains(`zone-${vis}`);
      zone.style.opacity = isMatch ? "1" : "0";
      zone.style.pointerEvents = isMatch ? "auto" : "none";
    });
    if (centerInput) centerInput.style.opacity = "1";
    if (centerStar) centerStar.style.opacity = "1";
  }
}

document.addEventListener("mousemove", (e: MouseEvent) => {
  // Keyboard filter active — skip mouse logic
  const input = document.getElementById("center-input") as HTMLInputElement | null;
  if (input && input.value.length > 0) return;

  // Already dismissed — nothing to do
  if (mousePhase === "dismissed") return;

  const app = document.querySelector<HTMLDivElement>("#app");
  if (!app) return;

  const rect = app.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;

  // Distance from cursor to nearest visible content element (in screen coords)
  const contentDist = distToNearestContent(e.clientX, e.clientY);
  const overContent = contentDist === 0;

  // Log mouse vs center on first few moves (throttled)
  if (!("_logCount" in document)) (document as any)._logCount = 0;
  if ((document as any)._logCount < 5 || (document as any)._logCount % 50 === 0) {
    console.log(`[mux-pos] mouse: clientX=${e.clientX} clientY=${e.clientY} local=(${Math.round(mx)},${Math.round(my)}) center=(${CENTER},${CENTER}) contentDist=${Math.round(contentDist)}`);
  }
  (document as any)._logCount++;

  if (mousePhase === "pristine") {
    if (overContent || contentDist < DISMISS_MARGIN) {
      const prev = mousePhase;
      mousePhase = "tracking";
      console.log(`[mux-mouse] phase=${prev}→tracking contentDist=${Math.round(contentDist)} overContent=${overContent}`);
      setVisibleZone(directionalZone(mx, my));
    }
    // Still far from content — stay pristine, show all
    return;
  }

  // mousePhase === "tracking"
  if (!overContent && contentDist > DISMISS_MARGIN) {
    const prev = mousePhase;
    mousePhase = "dismissed";
    console.log(`[mux-mouse] phase=${prev}→dismissed contentDist=${Math.round(contentDist)}`);
    setVisibleZone("none");
    invoke("mux_hide_overlay");
    return;
  }

  setVisibleZone(directionalZone(mx, my));
  updateLRSlide(mx, my);
});

// ---------------------------------------------------------------------------
// Dismiss handlers
// ---------------------------------------------------------------------------

// Dismiss the overlay on Escape key
document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    await invoke("mux_hide_overlay");
  }
});

// First printable keystroke: hide star, show input, seed with that char
document.addEventListener("keydown", (e: KeyboardEvent) => {
  // Ignore modifier-only, navigation, and Escape keys
  if (e.key === "Escape" || e.key === "Tab" || e.metaKey || e.ctrlKey || e.altKey) return;
  if (e.key.startsWith("Arrow") || e.key === "Shift" || e.key === "Meta" || e.key === "Control" || e.key === "Alt") return;

  const input = document.getElementById("center-input") as HTMLInputElement | null;
  if (!input) return;

  // If input already visible, let it handle events normally
  if (input.style.display !== "none") return;

  const star = document.getElementById("center-star");

  // Only activate on printable characters (single char keys)
  if (e.key.length !== 1) return;

  e.preventDefault();

  // Hide star, show input, seed with the pressed key
  if (star) star.style.display = "none";
  input.style.display = "";
  input.value = e.key;
  input.focus();
  applyFilter(input.value);
});

// Dismiss the overlay when the window loses focus (click outside).
// Grace period: if focus was gained very recently (e.g. overlay was just
// summoned from a terminal right-click), ignore the immediate focus-loss
// that occurs when a native context menu steals focus in the caller window.
let focusLossTimer: ReturnType<typeof setTimeout> | null = null;
let lastFocusGained = 0;
const currentWindow = getCurrentWindow();
currentWindow.onFocusChanged(async ({ payload: focused }) => {
  console.log(`[mux-mouse] focus-change: focused=${focused} phase=${mousePhase}`);
  if (focused) {
    // Log the window position and center for debugging
    const appEl = document.querySelector<HTMLDivElement>("#app");
    if (appEl) {
      const appRect = appEl.getBoundingClientRect();
      console.log(`[mux-pos] window: left=${Math.round(appRect.left)} top=${Math.round(appRect.top)} w=${Math.round(appRect.width)} h=${Math.round(appRect.height)}`);
      console.log(`[mux-pos] center: x=${Math.round(appRect.left + appRect.width/2)} y=${Math.round(appRect.top + appRect.height/2)}`);
      console.log(`[mux-pos] screenX=${window.screenX} screenY=${window.screenY} outerW=${window.outerWidth} outerH=${window.outerHeight}`);
      console.log(`[mux-pos] CENTER const=${CENTER} APP_SIZE=${APP_SIZE}`);
    }
    lastFocusGained = Date.now();
    if (focusLossTimer !== null) {
      clearTimeout(focusLossTimer);
      focusLossTimer = null;
    }
    // Reset overlay to fresh state when re-summoned.
    // Without this, mousePhase stays "dismissed" from the previous
    // session and the overlay stops responding on subsequent opens.
    resetMousePhase();
    // Force DOM refresh: resetMousePhase sets visibleZone="all", so
    // setVisibleZone("all") would short-circuit. Clear it first.
    visibleZone = "none";
    setVisibleZone("all");
    activeZoneLabel = null;
    filterText = "";
    const input = document.getElementById("center-input") as HTMLInputElement | null;
    const star = document.getElementById("center-star");
    if (input) { input.style.display = "none"; input.value = ""; }
    if (star) star.style.display = "";
    hideSpotlightDropdown();
  } else {
    // If we just gained focus within the last 2s, don't auto-dismiss
    if (Date.now() - lastFocusGained < 2000) return;
    focusLossTimer = setTimeout(async () => {
      focusLossTimer = null;
      await invoke("mux_hide_overlay");
    }, 300);
  }
});

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
buildOverlay();
