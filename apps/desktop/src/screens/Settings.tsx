// SPEC-010 T-010-04: settings. Each change is persisted by Rust (update_settings validates ranges/enums).
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import { Button, Card, ErrorState, LoadingState, Select, errorMessage, useToast } from "../components/ui";
import {
  getSettings,
  installSeparationModel,
  listKnownSeparationModels,
  removeSeparationModel,
  updateSettings,
  type AppSettings,
  type InstalledSeparationModel,
} from "../lib/ipc";
import { applyReducedMotion, applyTvMode } from "../display";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

export type SettingsSection = "autoReference";

export function Settings({ section }: { section?: SettingsSection }) {
  const [load, reload] = useAsync(getSettings, []);
  return (
    <div className="flex max-w-4xl flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Ajustes</h1>
      {load.status === "loading" && <LoadingState />}
      {load.status === "error" && <ErrorState title="Não foi possível carregar os ajustes" error={load.error} onRetry={reload} />}
      {load.status === "ready" && <Form initial={load.data} section={section} />}
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

function Form({ initial, section }: { initial: AppSettings; section?: SettingsSection }) {
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

      <AutoReference
        installed={s.separationModel ?? null}
        onChange={(m) => setS((cur) => ({ ...cur, separationModel: m }))}
        focus={section === "autoReference"}
      />
    </>
  );
}

const mb = (bytes: number) => `${Math.round(bytes / 1_048_576).toLocaleString("pt-BR")} MB`;
const day = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short" });

/** SPEC-012 R1/R8: third-party separation model, downloaded by the user, checked by SHA-256 in Rust. */
function AutoReference({
  installed,
  onChange,
  focus,
}: {
  installed: InstalledSeparationModel | null;
  onChange: (m: InstalledSeparationModel | null) => void;
  focus: boolean;
}) {
  const toast = useToast();
  const [known, reload] = useAsync(listKnownSeparationModels, []);
  const [accepted, setAccepted] = useState(false);
  const [busy, setBusy] = useState(false);
  const title = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (focus) title.current?.focus(); // arrived from "Gerar automaticamente" without a model
  }, [focus]);

  const install = async () => {
    let path: string | null;
    try {
      path = await open({
        multiple: false,
        directory: false,
        title: "Escolha o modelo baixado",
        filters: [{ name: "Modelo ONNX", extensions: ["onnx"] }],
      });
    } catch (e) {
      toast(errorMessage(e), "error");
      return;
    }
    if (!path) return;
    setBusy(true);
    try {
      const m = await installSeparationModel(path, accepted);
      onChange(m);
      setAccepted(false);
      toast(`Modelo ${m.name} instalado.`, "success");
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await removeSeparationModel();
      onChange(null);
      toast("Modelo removido.", "success");
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-5">
      <h2 ref={title} tabIndex={-1} className="font-display text-2xl font-semibold">
        Referência automática
      </h2>
      <p className="text-muted">
        Com um modelo de separação de voz, o app ouve a voz do próprio vídeo e escreve as notas para você. O resultado pode
        ter erros: confira sobre o vídeo e valide antes de valer ranking.
      </p>
      {installed ? (
        <div className="flex flex-wrap items-center justify-between gap-4 rounded-md bg-surface-2 px-4 py-3">
          <p>
            <strong>{installed.name}</strong> <span className="text-muted">· instalado em {day.format(installed.acceptedMs)}</span>
          </p>
          <Button variant="danger" disabled={busy} onClick={remove}>
            Remover
          </Button>
        </div>
      ) : (
        <>
          {known.status === "loading" && <LoadingState label="Carregando modelos…" />}
          {known.status === "error" && <ErrorState title="Não foi possível listar os modelos" error={known.error} onRetry={reload} />}
          {known.status === "ready" && (
            <ul className="flex flex-col gap-3">
              {known.data.map((m) => (
                <li key={m.id} className="flex flex-col gap-2 rounded-md bg-surface-2 px-4 py-3">
                  <p className="font-semibold">
                    {m.name} <span className="font-normal text-muted">· {mb(m.sizeBytes)}</span>
                  </p>
                  <p className="text-sm">Licença: {m.license}</p>
                  <p className="text-sm break-all text-muted">
                    Arquivo {m.fileName} · SHA-256 {m.id}
                  </p>
                  <p className="text-sm">Leia a página do projeto antes de baixar.</p>
                  {/* Order matters: credits/terms first, then the file (the browser downloads it, never the app). */}
                  <div className="flex flex-wrap gap-3">
                    <Button onClick={() => openUrl(m.pageUrl).catch((e: unknown) => toast(errorMessage(e), "error"))}>
                      Abrir página do projeto
                    </Button>
                    <Button onClick={() => openUrl(m.sourceUrl).catch((e: unknown) => toast(errorMessage(e), "error"))}>
                      Baixar o arquivo do modelo
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
          <p className="text-sm text-muted">
            O modelo é de terceiros: o download acontece no seu navegador. O app nunca baixa nem envia o modelo e só aceita
            os arquivos listados acima.
          </p>
          <label className="flex min-h-12 cursor-pointer items-start gap-4">
            <input
              type="checkbox"
              checked={accepted}
              onChange={(e) => setAccepted(e.currentTarget.checked)}
              className="mt-1 size-6 shrink-0 accent-(--color-accent)"
            />
            <span>Li e aceito os termos da fonte; o modelo é de terceiros e o uso é de minha responsabilidade.</span>
          </label>
          <Button className="self-start" variant="primary" disabled={!accepted || busy} onClick={install}>
            {busy ? "Conferindo o arquivo…" : "Escolher arquivo baixado…"}
          </Button>
        </>
      )}
    </Card>
  );
}
