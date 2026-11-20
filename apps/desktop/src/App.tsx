import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { NavBar, ToastProvider, useToast } from "./components/ui";
import { getSettings } from "./lib/ipc";
import { applyReducedMotion, applyTvMode } from "./display";
import { NavContext, type Nav, type Route } from "./nav";
import { handleSpatialKey } from "./spatial";
import { Home } from "./screens/Home";
import { Library } from "./screens/Library";
import { Mic } from "./screens/Mic";
import { Player } from "./screens/Player";
import { Ranking } from "./screens/Ranking";
import { Reference } from "./screens/Reference";
import { History } from "./screens/History";
import { Modes } from "./screens/Modes";
import { NewSession } from "./screens/NewSession";
import { Participants } from "./screens/Participants";
import { Session } from "./screens/Session";
import { SongDetails } from "./screens/SongDetails";
import { Settings } from "./screens/Settings";
import { Updates } from "./screens/Updates";
import { Result } from "./screens/Result";

type TopLevel = "home" | "modes" | "library" | "mic" | "ranking" | "settings";
const navItems: { key: TopLevel; label: string }[] = [
  { key: "home", label: "Início" },
  { key: "modes", label: "Jogar" },
  { key: "library", label: "Biblioteca" },
  { key: "mic", label: "Microfone" },
  { key: "ranking", label: "Ranking" },
  { key: "settings", label: "Ajustes" },
];

function Screen({ route }: { route: Route }) {
  switch (route.name) {
    case "home":
      return <Home />;
    case "library":
      return <Library />;
    case "mic":
      return <Mic />;
    case "player":
      return <Player songId={route.songId} round={route.round} />;
    case "song":
      return <SongDetails songId={route.songId} />;
    case "reference":
      return <Reference songId={route.songId} />;
    case "result":
      return <Result songId={route.songId} outcome={route.outcome} round={route.round} />;
    case "ranking":
      return <Ranking songId={route.songId} />;
    case "modes":
      return <Modes />;
    case "new-session":
      return <NewSession mode={route.mode} />;
    case "session":
      return <Session id={route.id} />;
    case "participants":
      return <Participants />;
    case "history":
      return <History />;
    case "settings":
      return <Settings section={route.section} />;
    case "updates":
      return <Updates />;
  }
}

/** SPEC-009 R4: background check at startup, only with consent (autoCheckUpdates); never installs on its own. */
function UpdateWatcher() {
  const toast = useToast();
  useEffect(() => {
    let alive = true;
    getSettings()
      .then((s) => (s.autoCheckUpdates ? check({ timeout: 30_000 }) : null))
      .then((u) => {
        if (u && alive) toast(`Versão ${u.version} disponível. Veja em Ajustes → Verificar agora.`);
        void u?.close();
      })
      .catch((e: unknown) => console.warn("update check:", e)); // R7: the app keeps working; the Updates screen shows details
    return () => {
      alive = false;
    };
  }, [toast]);
  return null;
}

export default function App() {
  const [stack, setStack] = useState<Route[]>([{ name: "home" }]);
  const route = stack[stack.length - 1];
  const main = useRef<HTMLElement>(null);

  const navigate = useCallback((to: Route) => setStack((s) => [...s, to]), []);
  const replace = useCallback((to: Route) => setStack((s) => [...s.slice(0, -1), to]), []);
  const back = useCallback(() => setStack((s) => (s.length > 1 ? s.slice(0, -1) : s)), []);
  const nav = useMemo<Nav>(() => ({ route, navigate, replace, back }), [route, navigate, replace, back]);

  // Display prefs: in-app reduced motion overrides the OS preference; Modo TV scales the rem grid.
  useEffect(() => {
    getSettings().then(
      (s) => {
        applyReducedMotion(s.reducedMotion);
        applyTvMode(s.tvMode);
      },
      () => {}, // screens surface settings errors themselves; the OS preference still applies
    );
  }, []);

  // Move focus to the new screen so keyboard and screen-reader users start at its top.
  useEffect(() => {
    main.current?.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }, [route]);

  // Esc = back, unless a native dialog is open (it consumes Esc itself).
  // Arrows = spatial focus navigation (A11Y-05), except in the Player, where they are seek/volume.
  const inPlayer = route.name === "player";
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented && !document.querySelector("dialog[open]")) back();
      else if (!inPlayer) handleSpatialKey(e);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back, inPlayer]);

  const fullscreen = route.name === "player";
  return (
    <NavContext.Provider value={nav}>
      <ToastProvider>
        <UpdateWatcher />
        {!fullscreen && (
          <NavBar
            items={navItems}
            current={navItems.some((i) => i.key === route.name) ? (route.name as TopLevel) : null}
            onNavigate={(key) => setStack([{ name: "home" }, ...(key === "home" ? [] : [{ name: key }])])}
          />
        )}
        <main ref={main} tabIndex={-1} className={fullscreen ? "outline-none" : "mx-auto max-w-7xl p-8 outline-none"}>
          <Screen route={route} />
        </main>
      </ToastProvider>
    </NavContext.Provider>
  );
}
