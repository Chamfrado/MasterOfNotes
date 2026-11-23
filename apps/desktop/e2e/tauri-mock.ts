// Test-only Tauri IPC mock (Law 03: never imported by src/). Installs window.__TAURI_INTERNALS__
// before the app loads, answering each command from serializable data, so specs stay declarative.
import type { Page } from "@playwright/test";
import type { AppError } from "../src/lib/ipc";

type MockReply = { ok: unknown } | { err: AppError };
/** Replies by command. Change one mid-test with `setReply` (call counts are unreliable:
 * React StrictMode runs mount effects twice in dev). */
export type MockSpec = Record<string, MockReply>;
interface IpcCall {
  cmd: string;
  args: unknown;
  /** Raw IPC (e.g. upload_song_audio): `args` is the Uint8Array body and the headers come here. */
  options?: InvokeOptions;
}
interface InvokeOptions {
  headers?: Record<string, string>;
}

declare global {
  interface Window {
    __E2E_CALLS__?: IpcCall[];
    __E2E_SPEC__?: MockSpec;
    __E2E_VIDEO_URL__?: string;
    /** Stateful handlers (e2e/fake-backend.ts); they win over the static spec. Throw (or return a rejected
     * promise with) an AppError to reject; a promise result is awaited (long jobs with progress). */
    __E2E_HANDLERS__?: Record<string, (args: Record<string, unknown>, options?: InvokeOptions) => unknown>;
  }
}

/** Every <video src> resolves here; specs decide what the "file" returns. */
const ASSET_ORIGIN = "http://asset.localhost";

export async function mockTauri(page: Page, spec: MockSpec): Promise<{ pageErrors: string[] }> {
  const pageErrors: string[] = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  // Offline by default: no asset request leaves the test.
  await page.route(`${ASSET_ORIGIN}/**`, (route) => route.fulfill({ status: 404 }));

  await page.addInitScript(
    ({ spec, assetOrigin }) => {
      const calls: IpcCall[] = [];
      const callbacks = new Map<number, (data: unknown) => void>();
      let nextId = 1;

      // Plugin commands the shell needs; a spec entry overrides them.
      const builtins: Record<string, unknown> = {
        "plugin:event|listen": 0,
        "plugin:event|unlisten": null,
        "plugin:window|set_fullscreen": null,
        "plugin:window|is_fullscreen": false,
      };

      const reply = (cmd: string, args: unknown, options?: InvokeOptions): Promise<unknown> => {
        const handler = window.__E2E_HANDLERS__?.[cmd];
        if (handler) {
          try {
            return Promise.resolve(handler((args ?? {}) as Record<string, unknown>, options)).then((v) => structuredClone(v));
          } catch (e) {
            return Promise.reject(e);
          }
        }
        const r = window.__E2E_SPEC__?.[cmd];
        if (r === undefined) {
          if (cmd in builtins) return Promise.resolve(builtins[cmd]);
          return Promise.reject({ code: "E2E_UNMOCKED", message: `Comando não mockado no teste: ${cmd}` });
        }
        return "ok" in r ? Promise.resolve(structuredClone(r.ok)) : Promise.reject(r.err);
      };

      const unregisterCallback = (id: number) => void callbacks.delete(id);
      Object.assign(window, {
        __E2E_CALLS__: calls,
        __E2E_SPEC__: spec,
        __TAURI_INTERNALS__: {
          metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
          invoke: (cmd: string, args: unknown, options?: InvokeOptions) => {
            calls.push({ cmd, args, options });
            return reply(cmd, args, options);
          },
          transformCallback: (cb: (data: unknown) => void, once = false) => {
            const id = nextId++;
            callbacks.set(id, (data) => {
              if (once) unregisterCallback(id);
              cb(data);
            });
            return id;
          },
          unregisterCallback,
          runCallback: (id: number, data: unknown) => callbacks.get(id)?.(data),
          convertFileSrc: (path: string) => window.__E2E_VIDEO_URL__ ?? `${assetOrigin}/${encodeURIComponent(path)}`,
        },
        __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: (_event: string, id: number) => unregisterCallback(id) },
      });
    },
    { spec, assetOrigin: ASSET_ORIGIN },
  );
  return { pageErrors };
}

/** Records a short synthetic WebM in the page (canvas + MediaRecorder, no binary fixture, Law 19)
 * and makes every video src resolve to it. Call after `page.goto`. `audio`: adds a 440 Hz Opus track
 * (SPEC-012 decodes the video's audio in the WebView). */
export const useSyntheticVideo = (page: Page, ms = 800, audio = false) =>
  page.evaluate(async ([ms, audio]) => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 64;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas 2d unavailable");
    const stream = canvas.captureStream(30);
    const ac = audio ? new AudioContext() : null;
    if (ac) {
      const osc = ac.createOscillator();
      const dest = ac.createMediaStreamDestination();
      osc.connect(dest);
      osc.start();
      stream.addTrack(dest.stream.getAudioTracks()[0]);
    }
    const rec = new MediaRecorder(stream, { mimeType: audio ? "video/webm;codecs=vp8,opus" : "video/webm;codecs=vp8" });
    const chunks: Blob[] = [];
    rec.ondataavailable = (e) => chunks.push(e.data);
    const stopped = new Promise((resolve) => (rec.onstop = resolve));
    let frame = 0;
    const draw = setInterval(() => {
      ctx.fillStyle = frame++ % 2 ? "#202020" : "#404040";
      ctx.fillRect(0, 0, 64, 64);
    }, 33);
    rec.start();
    await new Promise((resolve) => setTimeout(resolve, ms));
    rec.stop();
    clearInterval(draw);
    await stopped;
    await ac?.close();
    window.__E2E_VIDEO_URL__ = URL.createObjectURL(new Blob(chunks, { type: "video/webm" }));
  }, [ms, audio] as const);

export const setReply =(page: Page, cmd: string, reply: MockReply) =>
  page.evaluate(([cmd, reply]) => void ((window.__E2E_SPEC__ ??= {})[cmd] = reply), [cmd, reply] as const);

export const ipcCalls = (page: Page, cmd: string) =>
  page.evaluate((cmd) => (window.__E2E_CALLS__ ?? []).filter((c) => c.cmd === cmd), cmd);

type Ipc = typeof import("../src/lib/ipc");

/** Runs `fn` in the page with the app's real ipc module (src/lib/ipc.ts, served by Vite): contract tests of a
 * fake through the client the screens use. Returns `fn`'s result, or `{ rejected: AppError }`. */
export const withIpc = <T,>(page: Page, fn: (ipc: Ipc, arg: string) => Promise<T>, arg: string | null) =>
  page.evaluate(
    async ([src, arg]) => {
      const ipc = (await import("/src/lib/ipc.ts" as string)) as Ipc;
      return (new Function(`return ${src}`)() as typeof fn)(ipc, arg ?? "").catch((e: unknown) => ({ rejected: e }));
    },
    [fn.toString(), arg] as const,
  );
