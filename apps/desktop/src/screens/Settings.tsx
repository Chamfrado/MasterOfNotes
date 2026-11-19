// SPEC-010 T-010-04: settings. Each change is persisted by Rust (update_settings validates ranges/enums).
import { useEffect, useState } from "react";
import { Button, Card, ErrorState, LoadingState, Select, errorMessage, useToast } from "../components/ui";
import { getSettings, updateSettings, type AppSettings } from "../lib/ipc";
import { applyReducedMotion, applyTvMode } from "../display";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

export function Settings() {
  const [load, reload] = useAsync(getSettings, []);
  return (
    <div className="flex max-w-4xl flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Configurações</h1>
      {load.status === "loading" && <LoadingState />}
      {load.status === "error" && <ErrorState title="Não foi possível ler as configurações" error={load.error} onRetry={reload} />}
      {load.status === "ready" && <Form initial={load.data} />}
    </div>
  );
}

function Toggle({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex min-h-12 cursor-pointer items-start gap-4">
      <input
        type="checkbox"
        role="switch"
        checked={checked}
        onChange={(e) => onChange(e.currentTarget.checked)}
        className="mt-1 size-6 shrink-0 accent-(--color-accent)"
      />
      <span className="flex flex-col">
        <span className="font-medium">{label}</span>
        {hint && <span className="text-sm text-muted">{hint}</span>}
      </span>
    </label>
  );
}

function Form({ initial }: { initial: AppSettings }) {
  const toast = useToast();
  const { navigate } = useNav();
  const [s, setS] = useState(initial);

  // <html> follows the saved state, so a value Rust rejects is undone on screen too.
  useEffect(() => applyTvMode(s.tvMode), [s.tvMode]);
  useEffect(() => applyReducedMotion(s.reducedMotion), [s.reducedMotion]);

  const patch = async (p: Partial<AppSettings>) => {
    const before = s;
    setS({ ...s, ...p }); // optimistic; reverted on error
    try {
      setS(await updateSettings(p));
    } catch (e) {
      setS(before);
      toast(errorMessage(e), "error");
    }
  };

  return (
    <>
      <Card className="flex flex-col gap-5">
        <h2 className="font-display text-2xl font-semibold">Jogo</h2>
        <Select
          label="Dificuldade"
          hint="Quanto a afinação pode desviar para contar como acerto. O ranking separa por dificuldade."
          value={s.difficulty}
          onChange={(e) => patch({ difficulty: e.currentTarget.value as AppSettings["difficulty"] })}
          options={[
            { value: "easy", label: "Fácil" },
            { value: "normal", label: "Normal" },
            { value: "hard", label: "Difícil" },
          ]}
        />
        <Select
          label="Painel de feedback no player"
          hint="Também muda com a tecla H durante a música."
          value={s.feedbackOverlay}
          onChange={(e) => patch({ feedbackOverlay: e.currentTarget.value as AppSettings["feedbackOverlay"] })}
          options={[
            { value: "full", label: "Completo (nota e afinação)" },
            { value: "compact", label: "Compacto (só pontuação)" },
            { value: "hidden", label: "Oculto" },
          ]}
        />
      </Card>

      <Card className="flex flex-col gap-5">
        <h2 className="font-display text-2xl font-semibold">Tela e acessibilidade</h2>
        <Toggle
          label="Modo TV"
          hint="Textos e botões maiores, para ver de longe."
          checked={s.tvMode}
          onChange={(v) => patch({ tvMode: v })}
        />
        <Select
          label="Reduzir movimento"
          value={s.reducedMotion}
          onChange={(e) => patch({ reducedMotion: e.currentTarget.value as AppSettings["reducedMotion"] })}
          options={[
            { value: "system", label: "Seguir o sistema" },
            { value: "on", label: "Sempre reduzir" },
            { value: "off", label: "Nunca reduzir" },
          ]}
        />
      </Card>

      <Card className="flex flex-col gap-5">
        <h2 className="font-display text-2xl font-semibold">Biblioteca</h2>
        <Toggle
          label="Procurar também nas subpastas"
          hint="Vale a partir da próxima atualização da biblioteca."
          checked={s.recursiveScan}
          onChange={(v) => patch({ recursiveScan: v })}
        />
      </Card>

      <Card className="flex flex-col gap-5">
        <h2 className="font-display text-2xl font-semibold">Atualizações</h2>
        <Toggle
          label="Verificar atualizações ao abrir"
          hint="É o único uso de internet do app. Nada é instalado sem você confirmar."
          checked={s.autoCheckUpdates}
          onChange={(v) => patch({ autoCheckUpdates: v })}
        />
        <Button className="self-start" onClick={() => navigate({ name: "updates" })}>
          Verificar agora
        </Button>
      </Card>
    </>
  );
}
