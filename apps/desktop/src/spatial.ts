// A11Y-05: arrow-key spatial navigation (TV remote / 10-foot). Tab order keeps working as usual.

type Dir = "up" | "down" | "left" | "right";
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Index of the best candidate in `dir` from `from`, or -1. Candidates must lie beyond `from`'s edge in that
 * direction; score = distance along the axis + 3 × misalignment on the cross axis (0 when they overlap on it), so
 * the same row/column wins over a nearer diagonal.
 */
export function pickNext(from: Box, candidates: Box[], dir: Dir): number {
  const cx = from.x + from.w / 2;
  const cy = from.y + from.h / 2;
  let best = -1;
  let bestScore = Infinity;
  candidates.forEach((c, i) => {
    const ccx = c.x + c.w / 2;
    const ccy = c.y + c.h / 2;
    const overlapX = c.x < from.x + from.w && from.x < c.x + c.w;
    const overlapY = c.y < from.y + from.h && from.y < c.y + c.h;
    let main: number;
    let cross: number;
    switch (dir) {
      case "right":
        main = c.x - (from.x + from.w);
        cross = overlapY ? 0 : Math.abs(ccy - cy);
        if (ccx <= cx) return;
        break;
      case "left":
        main = from.x - (c.x + c.w);
        cross = overlapY ? 0 : Math.abs(ccy - cy);
        if (ccx >= cx) return;
        break;
      case "down":
        main = c.y - (from.y + from.h);
        cross = overlapX ? 0 : Math.abs(ccx - cx);
        if (ccy <= cy) return;
        break;
      case "up":
        main = from.y - (c.y + c.h);
        cross = overlapX ? 0 : Math.abs(ccx - cx);
        if (ccy >= cy) return;
        break;
    }
    const score = Math.max(0, main) + 3 * cross;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

const FOCUSABLE = 'button:not([disabled]), a[href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
const keyDir: Record<string, Dir> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right" };

/** Arrows already mean something inside these controls; leave them alone. */
function ownsArrows(el: Element | null): boolean {
  if (el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement) return true;
  if (el instanceof HTMLInputElement) return !["checkbox", "button", "submit"].includes(el.type);
  return false;
}

/** keydown handler: moves focus to the nearest focusable in the arrow's direction (inside an open dialog, if any). */
export function handleSpatialKey(e: KeyboardEvent): void {
  const dir = keyDir[e.key];
  if (!dir || e.defaultPrevented || e.altKey || e.ctrlKey || e.metaKey || ownsArrows(document.activeElement)) return;
  const scope = document.querySelector("dialog[open]") ?? document.querySelector("main") ?? document.body;
  const root = scope.tagName === "DIALOG" ? scope : document.body; // NavBar is outside <main>
  const all = [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.getClientRects().length > 0);
  const current = document.activeElement as HTMLElement | null;
  if (!current || !all.includes(current)) {
    const first = scope.querySelector<HTMLElement>(FOCUSABLE);
    if (first) {
      e.preventDefault();
      first.focus();
    }
    return;
  }
  const box = (el: HTMLElement): Box => {
    const r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, w: r.width, h: r.height };
  };
  const others = all.filter((el) => el !== current);
  const i = pickNext(box(current), others.map(box), dir);
  if (i >= 0) {
    e.preventDefault();
    others[i].focus();
    others[i].scrollIntoView({ block: "nearest", inline: "nearest" });
  }
}
