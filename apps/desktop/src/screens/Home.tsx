import { Button, Card, ErrorState, LoadingState, errorMessage, useToast } from "../components/ui";
import { useState } from "react";
import { consentAskedOf, markUpdateConsentAsked } from "../display";
import { getSettings, updateSettings } from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

export function Home() {
  const { navigate } = useNav();
  const [settings, reload] = useAsync(getSettings, []);
  return (
    <div className="flex flex-col gap-12">
      <section className="flex animate-enter flex-col items-start gap-6 pt-8">
        <h1 className="max-w-4xl font-display text-7xl leading-none font-bold">
          Cante. Afine.{" "}
          <span className="bg-(image:--gradient-accent) bg-clip-text text-transparent">Suba no ranking.</span>
        </h1>
        <p className="max-w-2xl text-xl text-muted">
          Master Of Notes pontua sua voz contra a melodia de verdade, nota por nota. Tudo offline, no seu computador.
        </p>
        <div className="flex flex-wrap gap-4">
          <Button variant="primary" size="lg" onClick={() => navigate({ name: "library" })}>
            Escolher música
          </Button>
          <Button size="lg" onClick={() => navigate({ name: "mic" })}>
            Configurar microfone
          </Button>
        </div>
      </section>

      <section aria-labelledby="setup-title" className="flex flex-col gap-4">
        <h2 id="setup-title" className="font-display text-2xl font-semibold">
          Preparação
        </h2>
        {settings.status === "loading" && <LoadingState label="Carregando configurações…" />}
        {settings.status === "error" && (
          <ErrorState title="Não foi possível ler as configurações" error={settings.error} onRetry={reload} />
        )}
        {settings.status === "ready" && !settings.data.autoCheckUpdates && !consentAskedOf(settings.data) && <UpdateConsent />}
        {settings.status === "ready" && (
          <div className="grid gap-4 md:grid-cols-2">
            <SetupStep
              done={settings.data.libraryFolder != null}
              title="Biblioteca"
              doneText={settings.data.libraryFolder ?? ""}
              todoText="Escolha a pasta com seus vídeos de karaokê."
              action="Abrir biblioteca"
              onAction={() => navigate({ name: "library" })}
            />
            <SetupStep
              done={settings.data.noiseFloorDbfs != null}
              title="Microfone"
              doneText="Calibrado para o ruído do ambiente."
              todoText="Calibre o microfone para uma pontuação justa."
              action="Calibrar"
              onAction={() => navigate({ name: "mic" })}
            />
          </div>
        )}
      </section>
    </div>
  );
}

/** SPEC-001 R2 / Law 10: autoCheckUpdates starts off; ask once. */
function UpdateConsent() {
  const toast = useToast();
  const [asked, setAsked] = useState(false);
  if (asked) return null;
  const answer = async (yes: boolean) => {
    if (yes) {
      try {
        await updateSettings({ autoCheckUpdates: true });
        toast("Combinado: vamos verificar atualizações ao abrir.", "success");
      } catch (e) {
        toast(errorMessage(e), "error");
        return;
      }
    }
    void markUpdateConsentAsked();
    setAsked(true);
  };
  return (
    <Card role="region" aria-label="Atualizações automáticas" className="flex flex-wrap items-center gap-4">
      <p className="flex-1">
        Quer que o Master Of Notes verifique se há versão nova ao abrir? É o único uso de internet do app, e nada é instalado
        sem você confirmar.
      </p>
      <div className="flex gap-3">
        <Button variant="ghost" onClick={() => answer(false)}>
          Agora não
        </Button>
        <Button variant="primary" onClick={() => answer(true)}>
          Sim, verificar
        </Button>
      </div>
    </Card>
  );
}

function SetupStep(p: { done: boolean; title: string; doneText: string; todoText: string; action: string; onAction: () => void }) {
  return (
    <Card className="flex items-center gap-4">
      <span
        aria-hidden
        className={`grid size-12 shrink-0 place-items-center rounded-full font-bold ${p.done ? "bg-success text-accent-fg" : "bg-surface-3 text-muted"}`}
      >
        {p.done ? "✓" : "•"}
      </span>
      <div className="flex-1">
        <h3 className="font-semibold">
          {p.title} <span className="sr-only">{p.done ? "(pronto)" : "(pendente)"}</span>
        </h3>
        <p className="text-muted">{p.done ? p.doneText : p.todoText}</p>
      </div>
      <Button variant={p.done ? "ghost" : "secondary"} onClick={p.onAction}>
        {p.action}
      </Button>
    </Card>
  );
}
