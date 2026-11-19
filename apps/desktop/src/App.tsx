import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { NavBar, ToastProvider } from "./components/ui";
import { getSettings } from "./lib/ipc";
import { NavContext, type Nav, type Route } from "./nav";
import { Home } from "./screens/Home";
import { Library } from "./screens/Library";
import { Mic } from "./screens/Mic";
import { Player } from "./screens/Player";
import { Ranking } from "./screens/Ranking";
import { Reference } from "./screens/Reference";
import { Result } from "./screens/Result";

type TopLevel = "home" | "library" | "mic" | "ranking";
const navItems: { key: TopLevel; label: string }[] = [
  { key: "home", label: "Início" },
  { key: "library", label: "Biblioteca" },
  { key: "mic", label: "Microfone" },
  { key: "ranking", label: "Ranking" },
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
      return <Player songId={route.songId} />;
    case "reference":
      return <Reference songId={route.songId} />;
    case "result":
      return <Result songId={route.songId} outcome={route.outcome} />;
    case "ranking":
      return <Ranking songId={route.songId} />;
  }
}

export default function App() {
  const [stack, setStack] = useState<Route[]>([{ name: "home" }]);
  const route = stack[stack.length - 1];
  const main = useRef<HTMLElement>(null);

  const navigate = useCallback((to: Route) => setStack((s) => [...s, to]), []);
  const back = useCallback(() => setStack((s) => (s.length > 1 ? s.slice(0, -1) : s)), []);
  const nav = useMemo<Nav>(() => ({ route, navigate, back }), [route, navigate, back]);

  // In-app reduced-motion setting overrides the OS preference (styles.css reads data-reduced-motion).
  useEffect(() => {
    getSettings().then(
      (s) => {
        if (s.reducedMotion !== "system") document.documentElement.dataset.reducedMotion = s.reducedMotion;
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
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented && !document.querySelector("dialog[open]")) back();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back]);

  const fullscreen = route.name === "player";
  return (
    <NavContext.Provider value={nav}>
      <ToastProvider>
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
