import { afterEach, describe, expect, it } from "vitest";
import { handleSpatialKey, pickNext, type Box } from "./spatial";

const b = (x: number, y: number, w = 100, h = 50): Box => ({ x, y, w, h });

describe("pickNext", () => {
  // Grid:  A(0,0) B(200,0) C(400,0)
  //        D(0,100)        E(400,100)
  const grid = [b(0, 0), b(200, 0), b(400, 0), b(0, 100), b(400, 100)];
  const others = (i: number) => grid.filter((_, j) => j !== i);
  const pick = (i: number, dir: Parameters<typeof pickNext>[2]) => {
    const o = others(i);
    const k = pickNext(grid[i], o, dir);
    return k < 0 ? -1 : grid.indexOf(o[k]);
  };

  it("moves along the row and column", () => {
    expect(pick(0, "right")).toBe(1);
    expect(pick(1, "right")).toBe(2);
    expect(pick(0, "down")).toBe(3);
    expect(pick(2, "down")).toBe(4);
    expect(pick(4, "up")).toBe(2);
  });
  it("prefers aligned targets over closer diagonal ones", () => {
    expect(pick(3, "right")).toBe(4); // same row, even if B is closer diagonally
  });
  it("returns -1 at the edges", () => {
    expect(pick(0, "left")).toBe(-1);
    expect(pick(0, "up")).toBe(-1);
    expect(pick(4, "down")).toBe(-1);
  });
});

describe("handleSpatialKey", () => {
  afterEach(() => document.body.replaceChildren());
  const key = (k: string, target: Element = document.activeElement ?? document.body) => {
    const e = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true });
    Object.defineProperty(e, "target", { value: target });
    handleSpatialKey(e);
    return e;
  };

  it("focuses the first control in <main> when nothing is focused", () => {
    document.body.innerHTML = `<nav><button>Nav</button></nav><main><button id="a">A</button></main>`;
    // jsdom has no layout: give elements a client rect so they count as visible
    document.querySelectorAll("button").forEach((el) => (el.getClientRects = () => [{}] as unknown as DOMRectList));
    const e = key("ArrowDown");
    expect(document.activeElement?.id).toBe("a");
    expect(e.defaultPrevented).toBe(true);
  });

  it("leaves arrows to text fields, selects and sliders", () => {
    document.body.innerHTML = `<main><input id="t" /><button>B</button></main>`;
    const input = document.getElementById("t") as HTMLInputElement;
    input.focus();
    const e = key("ArrowRight", input);
    expect(e.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(input);
  });
});
