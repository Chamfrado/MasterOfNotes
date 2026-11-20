import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "./lib/ipc";
import { SAMPLE_RATE, decodeMono44k, resetSongAudio, withSongAudio } from "./songAudio";

vi.mock("./lib/ipc", () => ({ uploadSongAudio: vi.fn(), videoSrc: (s: { id: string }) => `asset://${s.id}.mp4` }));

const mono = new Float32Array([0.5, 0.25]);
let decodedChannels = 2;
const contexts: { channels: number; length: number; rate: number; decoded?: ArrayBuffer; rendered?: unknown }[] = [];

class FakeOfflineAudioContext {
  info: (typeof contexts)[number];
  constructor(channels: number, length: number, rate: number) {
    this.info = { channels, length, rate };
    contexts.push(this.info);
  }
  async decodeAudioData(b: ArrayBuffer) {
    this.info.decoded = b;
    return { numberOfChannels: decodedChannels, length: 2, getChannelData: () => mono };
  }
  createBufferSource() {
    const info = this.info;
    return {
      set buffer(b: unknown) {
        info.rendered = b;
      },
      connect: () => {},
      start: () => {},
    };
  }
  async startRendering() {
    return { getChannelData: () => mono };
  }
}

const song = (id: string) => ({ id }) as ipc.Song;
const fetchMock = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  resetSongAudio();
  contexts.length = 0;
  decodedChannels = 2;
  fetchMock.mockImplementation(async () => ({ ok: true, status: 200, arrayBuffer: async () => new ArrayBuffer(8) }));
  vi.stubGlobal("fetch", fetchMock);
  vi.stubGlobal("OfflineAudioContext", FakeOfflineAudioContext);
  vi.mocked(ipc.uploadSongAudio).mockResolvedValue("tok-1");
});

describe("decodeMono44k", () => {
  it("decodes at 44.1 kHz and downmixes stereo to one channel", async () => {
    expect(await decodeMono44k("asset://a.mp4")).toBe(mono);
    expect(fetchMock).toHaveBeenCalledWith("asset://a.mp4", { signal: undefined });
    expect(contexts[0]).toMatchObject({ channels: 1, rate: SAMPLE_RATE });
    expect(contexts[0].decoded).toBeInstanceOf(ArrayBuffer);
    expect(contexts[1]).toMatchObject({ channels: 1, length: 2, rate: SAMPLE_RATE });
    expect(contexts[1].rendered).toBeDefined();
  });

  it("skips the downmix when the audio is already mono", async () => {
    decodedChannels = 1;
    await decodeMono44k("asset://a.mp4");
    expect(contexts).toHaveLength(1);
  });

  it("reports an unreadable video", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 403 });
    await expect(decodeMono44k("asset://a.mp4")).rejects.toThrow("Não foi possível ler o vídeo (HTTP 403).");
  });
});

describe("withSongAudio", () => {
  it("uploads once per song and reuses the token (Rust keeps one item)", async () => {
    const onExtract = vi.fn();
    const fn = vi.fn(async (t: string) => t);
    expect(await withSongAudio(song("a"), fn, { onExtract })).toBe("tok-1");
    expect(await withSongAudio(song("a"), fn, { onExtract })).toBe("tok-1");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(onExtract).toHaveBeenCalledTimes(1);
    expect(ipc.uploadSongAudio).toHaveBeenCalledWith("a", mono);

    vi.mocked(ipc.uploadSongAudio).mockResolvedValue("tok-2");
    expect(await withSongAudio(song("b"), fn)).toBe("tok-2"); // another song replaces the item
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("extracts again once on audio_token_invalid", async () => {
    await withSongAudio(song("a"), async () => null);
    vi.mocked(ipc.uploadSongAudio).mockResolvedValue("tok-new");
    const fn = vi.fn(async (t: string) => {
      if (t === "tok-1") throw { code: "audio_token_invalid", message: "Áudio da música não está mais disponível." };
      return t;
    });
    expect(await withSongAudio(song("a"), fn)).toBe("tok-new");
    expect(fn).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("does not retry other errors", async () => {
    const fn = vi.fn(async () => {
      throw { code: "busy", message: "Já há uma geração em andamento." };
    });
    await expect(withSongAudio(song("a"), fn)).rejects.toMatchObject({ code: "busy" });
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("an aborted extraction never uploads", async () => {
    const abort = new AbortController();
    abort.abort();
    await expect(withSongAudio(song("a"), async () => null, { signal: abort.signal })).rejects.toThrow();
    expect(ipc.uploadSongAudio).not.toHaveBeenCalled();
  });
});
