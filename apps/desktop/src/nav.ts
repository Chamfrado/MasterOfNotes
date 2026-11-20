// Minimal in-memory router: a desktop app has no URL bar, so a typed route stack is all we need.
import { createContext, useContext } from "react";
import type { GameMode, PerformanceOutcome } from "./lib/ipc";

/** A performance played as a session round (SPEC-008). */
export interface RoundContext {
  sessionId: number;
  roundId: number;
  participantId: number | null;
}

export type Route =
  | { name: "home" }
  | { name: "library" }
  | { name: "mic" }
  | { name: "player"; songId: string; round?: RoundContext }
  | { name: "reference"; songId: string }
  | { name: "song"; songId: string }
  | { name: "result"; songId: string; outcome: PerformanceOutcome; round?: RoundContext }
  | { name: "ranking"; songId?: string }
  | { name: "modes" }
  | { name: "new-session"; mode: GameMode }
  | { name: "session"; id: number }
  | { name: "participants" }
  | { name: "history" }
  | { name: "settings"; section?: "autoReference" }
  | { name: "updates" };

export interface Nav {
  route: Route;
  navigate: (to: Route) => void;
  /** Swap the current screen (e.g. Player → Result) so Back skips it. */
  replace: (to: Route) => void;
  back: () => void;
}

export const NavContext = createContext<Nav | null>(null);

export function useNav(): Nav {
  const nav = useContext(NavContext);
  if (!nav) throw new Error("useNav outside NavContext");
  return nav;
}
