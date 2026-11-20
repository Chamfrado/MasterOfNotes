// SPEC-012 R6: the song's audio, decoded in the WebView (same codecs the player plays), mono 44.1 kHz f32,
// uploaded once per song. Rust keeps ONE item (token valid until the next upload), mirrored here.
import { uploadSongAudio, videoSrc, type Song } from "./lib/ipc";

export const SAMPLE_RATE = 44_100;
export const UNDECODABLE =
  "Não foi possível ler o áudio deste vídeo. O arquivo pode não ter som ou usar um formato de áudio não suportado.";

let cached: { songId: string; token: string } | null = null;

/** fetch → decodeAudioData (resamples to the context rate) → downmix to mono. The encoded file is released as soon as it is decoded. */
export async function decodeMono44k(src: string, signal?: AbortSignal): Promise<Float32Array> {
  const res = await fetch(src, { signal });
  if (!res.ok) throw new Error(`Não foi possível ler o vídeo (HTTP ${res.status}).`);
  let encoded: ArrayBuffer | null = await res.arrayBuffer();
  // decodeAudioData detaches `encoded`; dropping our reference lets the (possibly huge) video bytes be collected.
  let decoded: AudioBuffer;
  try {
    decoded = await new OfflineAudioContext(1, 1, SAMPLE_RATE).decodeAudioData(encoded);
  } catch (e) {
    console.warn("decodeAudioData:", e); // technical detail only in the log
    throw new Error(UNDECODABLE);
  } finally {
    encoded = null;
  }
  signal?.throwIfAborted();
  if (decoded.numberOfChannels === 1) return decoded.getChannelData(0);
  const mix = new OfflineAudioContext(1, decoded.length, SAMPLE_RATE); // Web Audio speaker downmix: mono = (L + R) / 2
  const node = mix.createBufferSource();
  node.buffer = decoded;
  node.connect(mix.destination);
  node.start();
  return (await mix.startRendering()).getChannelData(0);
}

/** Token for this song's audio: reuses the uploaded one, otherwise decodes and uploads. `fresh` forces a new upload (after audio_token_invalid). */
async function songAudioToken(song: Song, opts: { signal?: AbortSignal; fresh?: boolean } = {}): Promise<string> {
  if (!opts.fresh && cached?.songId === song.id) return cached.token;
  cached = null;
  const pcm = await decodeMono44k(videoSrc(song), opts.signal);
  opts.signal?.throwIfAborted();
  const token = await uploadSongAudio(song.id, pcm);
  cached = { songId: song.id, token };
  return token;
}

const isTokenInvalid = (e: unknown) => typeof e === "object" && e != null && (e as { code?: unknown }).code === "audio_token_invalid";

/** Runs `fn` with the song's audio token; on audio_token_invalid, extracts again once. `onExtract` fires before a decode. */
export async function withSongAudio<T>(
  song: Song,
  fn: (token: string) => Promise<T>,
  opts: { signal?: AbortSignal; onExtract?: () => void } = {},
): Promise<T> {
  if (cached?.songId !== song.id) opts.onExtract?.();
  try {
    return await fn(await songAudioToken(song, opts));
  } catch (e) {
    if (!isTokenInvalid(e)) throw e;
    opts.onExtract?.();
    return fn(await songAudioToken(song, { ...opts, fresh: true }));
  }
}

/** Test hook: forget the mirrored token. */
export const resetSongAudio = () => {
  cached = null;
};
