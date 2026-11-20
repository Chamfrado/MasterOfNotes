// SPEC-009 R4/R7: check → notes → download with progress → confirm → install (the NSIS installer restarts the app).
// Only reachable outside the player, so an update never interrupts a performance.
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useRef, useState } from "react";
import { Button, Card, Dialog, Progress, errorMessage } from "../components/ui";
import { useNav } from "../nav";

type Phase =
  | { name: "checking" }
  | { name: "current" }
  | { name: "available"; update: Update }
  | { name: "downloading"; update: Update; received: number; total: number | null }
  | { name: "ready"; update: Update }
  | { name: "installing" }
  | { name: "error"; error: unknown };

/**
 * The Rust updater plugin is only registered when the build has a signing pubkey. Without it Tauri rejects with
 * "plugin updater not found" (tauri plugin.rs) or "updater.… not allowed. Plugin not found" (ipc/authority.rs).
 */
export const updaterUnavailable = (e: unknown) => /plugin updater not found|updater.*plugin not found/i.test(errorMessage(e));

/**
 * No release manifest on the endpoint (404: only pre-releases published, so releases/latest has no latest.json).
 * The plugin reports Error::ReleaseNotFound for any non-2xx answer (tauri-plugin-updater 2.x updater.rs).
 */
export const noPublishedRelease = (e: unknown) => /valid release JSON/i.test(errorMessage(e));
export const RELEASES_URL = "https://github.com/Chamfrado/MasterOfNotes/releases";

const mb = (bytes: number) => `${(bytes / 1_048_576).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} MB`;

export function Updates() {
  const { back } = useNav();
  const [version, setVersion] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ name: "checking" });
  const [confirm, setConfirm] = useState(false);
  const held = useRef<Update | null>(null); // Rust-side resource to release on leave

  const run = async () => {
    setPhase({ name: "checking" });
    try {
      const update = await check({ timeout: 30_000 });
      held.current = update;
      setPhase(update ? { name: "available", update } : { name: "current" });
    } catch (e) {
      setPhase({ name: "error", error: e });
    }
  };

  useEffect(() => {
    getVersion().then(setVersion, () => setVersion(null));
    void run();
    return () => {
      held.current?.close().catch(() => {});
    };
  }, []);

  const download = async (update: Update) => {
    let received = 0;
    let total: number | null = null;
    setPhase({ name: "downloading", update, received, total });
    try {
      await update.download((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? null;
        if (ev.event === "Progress") received += ev.data.chunkLength;
        setPhase({ name: "downloading", update, received, total });
      });
      setPhase({ name: "ready", update });
      setConfirm(true);
    } catch (e) {
      setPhase({ name: "error", error: e });
    }
  };

  const install = async (update: Update) => {
    setConfirm(false);
    setPhase({ name: "installing" });
    try {
      await update.install(); // Windows: the installer closes and reopens the app
    } catch (e) {
      setPhase({ name: "error", error: e });
    }
  };

  return (
    <div className="flex max-w-3xl flex-col gap-8">
      <header>
        <h1 className="font-display text-5xl font-bold">Atualizações</h1>
        <p className="mt-2 text-muted">Versão instalada: {version ?? "desconhecida"}</p>
      </header>
      <Card aria-live="polite" className="flex flex-col items-start gap-4">
        {phase.name === "checking" && (
          <>
            <p>Procurando atualizações…</p>
            <div className="w-64">
              <Progress label="Procurando atualizações" />
            </div>
          </>
        )}
        {phase.name === "current" && (
          <>
            <p className="text-success">Você já está na versão mais recente.</p>
            <Button onClick={run}>Verificar de novo</Button>
          </>
        )}
        {(phase.name === "available" || phase.name === "downloading" || phase.name === "ready") && (
          <>
            <h2 className="font-display text-2xl font-semibold">Versão {phase.update.version} disponível</h2>
            {phase.update.date && <p className="text-sm text-muted">Publicada em {new Date(phase.update.date).toLocaleDateString("pt-BR")}</p>}
            {phase.update.body && <p className="whitespace-pre-line text-muted">{phase.update.body}</p>}
          </>
        )}
        {phase.name === "available" && (
          <div className="flex gap-3">
            <Button variant="primary" onClick={() => download(phase.update)}>
              Baixar atualização
            </Button>
            <Button variant="ghost" onClick={back}>
              Depois
            </Button>
          </div>
        )}
        {phase.name === "downloading" && (
          <div className="flex w-full flex-col gap-2">
            <Progress label="Baixando atualização" value={phase.total ? phase.received : undefined} max={phase.total ?? undefined} />
            <p className="text-sm text-muted">
              {mb(phase.received)}
              {phase.total ? ` de ${mb(phase.total)}` : ""}
            </p>
          </div>
        )}
        {phase.name === "ready" && (
          <Button variant="primary" onClick={() => setConfirm(true)}>
            Instalar e reiniciar
          </Button>
        )}
        {phase.name === "installing" && <p>Instalando… o Master Of Notes vai fechar e abrir de novo.</p>}
        {phase.name === "error" && updaterUnavailable(phase.error) && (
          <p>Atualizações automáticas ainda não estão configuradas nesta versão. O app funciona normalmente.</p>
        )}
        {phase.name === "error" && noPublishedRelease(phase.error) && (
          <>
            <p>Nenhuma versão nova pelo canal de atualizações automáticas.</p>
            <p className="text-sm text-muted">Versões de teste, quando houver, ficam na página de versões do projeto.</p>
            <div className="flex flex-wrap gap-3">
              <Button onClick={() => openUrl(RELEASES_URL).catch((e: unknown) => console.warn("openUrl:", e))}>
                Abrir página de versões
              </Button>
              <Button variant="ghost" onClick={run}>
                Verificar de novo
              </Button>
            </div>
          </>
        )}
        {phase.name === "error" && !updaterUnavailable(phase.error) && !noPublishedRelease(phase.error) && (
          <>
            <p role="alert" className="text-danger">
              Não foi possível verificar ou baixar a atualização. O app continua funcionando normalmente.
            </p>
            <p className="text-sm text-muted">Detalhe: {errorMessage(phase.error)}</p>
            <Button onClick={run}>Tentar de novo</Button>
          </>
        )}
      </Card>

      {phase.name === "ready" && (
        <Dialog open={confirm} onClose={() => setConfirm(false)} title="Instalar agora?">
          <p className="mb-6 text-muted">
            O app vai fechar para instalar a versão {phase.update.version} e abrir de novo. Suas músicas, rankings e
            configurações são mantidos.
          </p>
          <div className="flex justify-end gap-3">
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              Mais tarde
            </Button>
            <Button variant="primary" onClick={() => install(phase.update)}>
              Instalar e reiniciar
            </Button>
          </div>
        </Dialog>
      )}
    </div>
  );
}
