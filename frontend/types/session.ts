export interface TmuxSession {
  name: string;
  windows: TmuxWindow[];
}

export interface TmuxWindow {
  index: number;
  name: string;
  panes: TmuxPane[];
}

export interface TmuxPane {
  id: string;
  index: number;
  width: number;
  height: number;
  top: number;
  left: number;
  agent: string | null;
}

export type LayoutNode =
  | { type: "row"; children: LayoutEntry[] }
  | { type: "col"; children: LayoutEntry[] }
  | { type: "pane"; agent: string };

export interface LayoutEntry {
  node: LayoutNode;
  percent: number | null;
}
