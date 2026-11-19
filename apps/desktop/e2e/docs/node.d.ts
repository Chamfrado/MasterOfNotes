// The few Node APIs the capture script uses (the app has no @types/node; e2e/tsconfig has types: []).
declare module "node:fs" {
  export function mkdirSync(path: string, opts?: { recursive?: boolean }): void;
  export function writeFileSync(path: string, data: Uint8Array): void;
  export function existsSync(path: string): boolean;
  export function statSync(path: string): { size: number };
  export function readdirSync(path: string): string[];
  export function renameSync(from: string, to: string): void;
}
declare module "node:url" {
  export function fileURLToPath(url: URL): string;
}
