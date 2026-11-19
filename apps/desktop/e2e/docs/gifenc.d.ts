// gifenc 1.0.3 ships no types; only what media.ts uses. CommonJS, so Node ESM sees a default export.
declare module "gifenc" {
  type Palette = number[][];
  const gifenc: {
    quantize(rgba: Uint8Array, maxColors: number): Palette;
    applyPalette(rgba: Uint8Array, palette: Palette): Uint8Array;
    GIFEncoder(): {
      writeFrame(
        index: Uint8Array,
        width: number,
        height: number,
        opts?: { palette?: Palette; delay?: number; repeat?: number; transparent?: boolean; transparentIndex?: number; dispose?: number },
      ): void;
      finish(): void;
      bytes(): Uint8Array;
    };
  };
  export default gifenc;
}
