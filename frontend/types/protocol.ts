export type Response =
  | { status: "ok"; output: string }
  | { status: "error"; message: string };

export type Direction = "horizontal" | "vertical";

export type Action =
  | { action: "create_session"; name: string; cwd: string }
  | { action: "kill_session"; name: string }
  | { action: "split_pane"; session: string; direction: Direction; percent: number }
  | { action: "place_agent"; pane_id: string; agent: string }
  | { action: "create_agent"; name: string; role: string; path: string }
  | { action: "kill_agent"; name: string }
  | { action: "send_keys"; target: string; keys: string };
