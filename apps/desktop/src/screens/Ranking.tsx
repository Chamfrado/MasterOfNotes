import { useState } from "react";
import { RankingTable } from "../components/game";
import { Card, ErrorState, LoadingState, Select } from "../components/ui";
import { getLeaderboard, getSettings, listSongs, type Difficulty, type Song } from "../lib/ipc";
import { useAsync } from "../useAsync";

const difficulties: { value: Difficulty; label: string }[] = [
  { value: "easy", label: "Fácil" },
  { value: "normal", label: "Normal" },
  { value: "hard", label: "Difícil" },
];

export function Ranking({ songId }: { songId?: string }) {
  const [load, reload] = useAsync(() => Promise.all([listSongs(), getSettings()]), []);
  return (
    <div className="flex flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Ranking</h1>
      {load.status === "loading" && <LoadingState />}
      {load.status === "error" && <ErrorState title="Não foi possível carregar o ranking" error={load.error} onRetry={reload} />}
      {load.status === "ready" &&
        (!load.data[0].some((s) => s.available) ? (
          <p className="text-muted">Sua biblioteca está vazia. Adicione músicas para ver rankings.</p>
        ) : (
          <Board songs={load.data[0].filter((s) => s.available)} initialSong={songId} initialDifficulty={load.data[1].difficulty} />
        ))}
    </div>
  );
}

function Board({ songs, initialSong, initialDifficulty }: { songs: Song[]; initialSong?: string; initialDifficulty: Difficulty }) {
  const [songId, setSongId] = useState(initialSong ?? songs[0]?.id ?? "");
  const [difficulty, setDifficulty] = useState(initialDifficulty);
  const [rows, reload] = useAsync(() => getLeaderboard(songId, difficulty), [songId, difficulty]);
  const song = songs.find((s) => s.id === songId);
  return (
    <Card className="flex flex-col gap-6">
      <div className="grid gap-4 md:grid-cols-[2fr_1fr]">
        <Select label="Música" value={songId} onChange={(e) => setSongId(e.currentTarget.value)} options={songs.map((s) => ({ value: s.id, label: s.title }))} />
        <Select label="Dificuldade" value={difficulty} onChange={(e) => setDifficulty(e.currentTarget.value as Difficulty)} options={difficulties} />
      </div>
      {rows.status === "loading" && <LoadingState />}
      {rows.status === "error" && <ErrorState error={rows.error} onRetry={reload} />}
      {rows.status === "ready" && <RankingTable rows={rows.data} caption={`Ranking de ${song?.title ?? "música"}`} />}
    </Card>
  );
}
