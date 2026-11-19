import { useState } from "react";
import { Button, Card, ErrorState, Input, LoadingState, errorMessage } from "../components/ui";
import { createParticipant, listParticipants, type Participant } from "../lib/ipc";
import { useAsync } from "../useAsync";

const NAME_MAX = 40; // mirrors create_participant (1–40 chars, unique ignoring case)

/** Inline "new singer" field (no <form>: it is also used inside the new-session form). Rust errors show under the field. */
export function AddParticipant({ onCreated }: { onCreated: (p: Participant) => void }) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    const n = name.trim();
    if (!n) return setError("Digite um nome.");
    setBusy(true);
    try {
      onCreated(await createParticipant(n));
      setName("");
      setError(undefined);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div role="group" aria-label="Cadastrar cantor" className="flex items-start gap-3">
      <div className="flex-1">
        <Input
          label="Novo cantor"
          placeholder="Nome"
          maxLength={NAME_MAX}
          value={name}
          error={error}
          onChange={(e) => {
            setName(e.currentTarget.value);
            setError(undefined);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void submit();
            }
          }}
        />
      </div>
      <Button className="mt-7" disabled={busy} onClick={submit}>
        Adicionar
      </Button>
    </div>
  );
}

export function Participants() {
  const [list, reload] = useAsync(listParticipants, []);
  return (
    <div className="flex max-w-3xl flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Participantes</h1>
      <Card>
        <AddParticipant onCreated={reload} />
      </Card>
      {list.status === "loading" && <LoadingState />}
      {list.status === "error" && <ErrorState title="Não foi possível carregar os participantes" error={list.error} onRetry={reload} />}
      {list.status === "ready" &&
        (list.data.length === 0 ? (
          <p className="text-muted">Nenhum cantor cadastrado ainda.</p>
        ) : (
          <ul aria-label="Cantores" className="grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-3">
            {list.data.map((p) => (
              <li key={p.id} className="rounded-md border border-line bg-surface px-5 py-4 font-medium">
                {p.name}
              </li>
            ))}
          </ul>
        ))}
    </div>
  );
}
