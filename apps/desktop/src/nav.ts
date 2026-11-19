// Minimal in-memory router: a desktop app has no URL bar, so a typed route stack is all we need.
import { createContext, useContext } from "react";
import type { PerformanceOutcome } from "./lib/ipc";

export type Route =
  | { name: "home" }
  | { name: "library" }
  | { name: "mic" }
  | { name: "player"; songId: string }
  | { name: "result"; songId: string; outcome: PerformanceOutcome }
  | { name: "ranking"; songId?: string };

export interface Nav {
  route: Route;
  navigate: (to: Route) => void;
  back: () => void;
}

export const NavContext = createContext<Nav | null>(null);

export function useNav(): Nav {
  const nav = useContext(NavContext);
  if (!nav) throw new Error("useNav outside NavContext");
  return nav;
}
