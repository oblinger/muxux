import type { LayoutNode } from "./session";

export type TileKind = "agent" | "composition" | "session";

export interface Tile {
  name: string;
  kind: TileKind;
  role: string | null;
  layout: LayoutNode | null;
}
