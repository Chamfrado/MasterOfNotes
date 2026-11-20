# QA independente do `song-analysis` (SPEC-012, T-012-05, parte 3)

Agente 4 · 2026-11-20

## Base e ambiente
- **Código medido:** `agent-2/work` @ 7876130, por merge local na `agent-4/work`.
- **Material:** sintético e de autoria própria (Lei 19), gerado pelas sondas. É independente do gerador do spike.
- **Sondas:** [song-analysis-qa-probes.rs](song-analysis-qa-probes.rs), cópia arquivada; o cabeçalho explica como reproduzir.
- **Modelo:** `UVR-MDX-NET-Voc_FT.onnx` (SHA-256 `534b2070…e111`, 66.762.490 bytes, conferido).
- **Runtime:** `onnxruntime.dll` 1.31.1 (SHA-256 `cb2fa518…db0c`, conferido).
- O modelo e a DLL ficaram só no scratchpad e foram apagados ao final.
- **Execução:** `cargo test -p song-analysis --release`, com a CPU livre nas medições de tempo. O pico de memória é o `PeakWorkingSet64` do processo de teste, amostrado a cada 0,5 s.

**Material**
- **Voz:** formantes com troca de vogal por nota, jitter 1 %, shimmer 10 %, drift ±15 c, scoop −80 c, vibrato tardio ±40 c e consoante de ruído de 25 ms no ataque.
- **Banda:** pad I–V–vi–IV **no registro da melodia** (o caso difícil), baixo e bateria.
- **Melodia:** passeio em dó maior, notas de 180–900 ms, notas repetidas e pausas de frase.

**Métrica:** a do spike. Nota correta = casamento 1-para-1, onset a ±100 ms e altura a ±50 c. A "pontuação simulada" é a de um cantor perfeito avaliado contra a referência gerada (Normal).

## Veredito
**Nenhum achado ALTO.** O núcleo cumpre R3, R4, R5 e R7 no material sintético:
- precisão de 97–99 % com a voz audível;
- 4 min em 132 s, com pico de 1353 MiB;
- sem falso "tem voz" em instrumental;
- alinhamento honesto nas armadilhas;
- cancelamento ≤ 1 trecho.

Há quatro achados **médios** (M1–M4) e três **baixos** (B1–B3, sendo o B1 já mitigado no app), abaixo. Nenhum grava dado errado sem a revisão humana prevista (a referência nasce experimental).

| ID | Gravidade | Achado | Sugestão |
|---|---|---|---|
| M1 | Médio | **Andamento diferente passa com confiança alta.** Se a referência (ex.: MIDI) está 0,25–1 % fora do andamento do vídeo, nenhum offset único serve, mas a proposta vem com confiança 0,68–0,88, acima do limiar da UI (0,5). O erro residual fica entre 120 e 464 ms nas pontas da música; o ritmo perde crédito acima de 80 ms e zera em 300 ms. A partir de 2 % a confiança cai, como deve. 0,8 % equivale a 120 × 121 BPM, comum em MIDI da internet. | Calcular o lag também na 1ª e na 2ª metade. Se diferirem em mais de ~100 ms, baixar a confiança, e a UI diz "o andamento da referência não acompanha o vídeo; um ajuste único não resolve". |
| M2 | Médio | **Voz baixa perto do limiar gera referência muito incompleta sem aviso.** Com a voz 10 dB abaixo da banda (nível −16,8 dB, acima do limiar), a referência é salva com só 23 de 81 notas corretas (recall de 28 %, precisão de 79 %). A pontuação simulada fica alta (95,8) só porque mede o que existe; a maior parte da melodia não é pontuada. | Avisar quando a voz ficar entre −20 e cerca de −10 dB, ou quando houver poucas notas por minuto de voz: "A voz do vídeo está fraca; a referência pode ficar incompleta. Confira no vídeo." |
| M3 | Médio | **A mensagem de "sem voz" é falsa quando há voz baixa.** Voz-guia a −15/−20 dB da banda na música toda, ou a −6 dB em só 20 % dela, cai abaixo de `NO_VOICE_DB` e mostra "Este vídeo não tem a voz da melodia (é só instrumental)". As notas descartadas seriam ruins (recall ≤ 36 %, precisão de 59–83 %), então **o limiar está certo**; o texto é que afirma algo falso. | Texto: "Não encontramos uma voz clara neste vídeo (pode ser só instrumental ou a voz estar baixa demais). Use Gravar cantando ou importe um MIDI." (R3 da SPEC). |
| M4 | Médio | **A memória cresce com a duração.** Pico de 1353 MiB em 4 min e 1535 MiB em 10 min, cerca de 30 MiB por minuto de música. A R7 (≤ 1,5 GB) é cumprida na música de 4 min da meta, mas 10 min passam um pouco e 20 min chegariam perto de 1,85 GB. | Documentar o limite (ou um teto de duração com mensagem), ou processar sem manter 3 cópias completas do áudio (padded, saída, mistura). |
| B1 | Baixo, ✅ **já mitigado no app** | **NaN na entrada contamina a saída.** Uma única amostra NaN em 10 s gera 253.440 NaN na voz separada (57 %). O `voice_level_db` dá NaN, e `NaN < −20` é falso, então o veredito vira "tem voz". Só acontece chamando o crate direto: o `upload_song_audio` (`parse_audio` em `src-tauri/src/analysis.rs`) já recusa com `audio_invalid` qualquer amostra não finita, com teste `analysis::tests::audio_upload_is_validated_and_kept_once` (conferido no código). | Nada a fazer no app; registrado para quem usar o crate. |
| B2 | Baixo | **Viés fixo no alinhamento:** +31 ms com voz cantada (consoante e scoop no ataque) e −36 ms com lead de piano (decaimento), contra o critério de ±30 ms da SPEC. Não tem efeito na pontuação, que dá crédito total de ritmo até 80 ms. O Agente 2 mediu 3–17 ms no material dele; a diferença vem da forma do ataque. | Só registrar; se quiser, critério de ±50 ms. |
| B3 | Baixo | **Inglês na mensagem para arquivo que não é modelo:** "não foi possível abrir o modelo (… Protobuf parsing failed.)". É inalcançável no app, porque a instalação confere o SHA-256 antes. | Opcional. |

## Medições

### Q1: precisão (R7: ≥ 95 % de notas corretas)
Música de 60 s, com 77 notas.

| Caso | Nível da voz | Corretas | Recall | Precisão | Espúrias | Pontuação simulada | Tempo |
|---|---|---|---|---|---|---|---|
| `transcribe` na voz limpa, sem separação (teto) | — | 75/77 | 97,4 % | 97,4 % | 0 | 97,9 | — |
| Voz +6 dB sobre a banda | −1,0 dB | 66/77 | 85,7 % | 97,1 % | 0 | 96,9 | 33 s ¹ |
| Voz 0 dB | −3,1 dB | 65/77 | 84,4 % | **98,5 %** | 0 | 96,7 | 33 s ¹ |
| Voz −6 dB | −7,2 dB | 58/77 | 75,3 % | 96,7 % | 0 | 96,3 | 35 s ¹ |
| Voz −10 dB | −10,8 dB | 56/77 | 72,7 % | 98,2 % | 0 | 96,0 | 39 s ¹ |
| 4 min, voz 0 dB | −0,8 dB | 300/341 | 88,0 % | **99,3 %** | 0 | 96,7 | **132 s** + 3 s de transcrição |
| 10 min, voz 0 dB | −0,7 dB | 774/854 | 90,6 % | 99,2 % | 0 | 96,8 | 318 s + 8 s |

¹ Medido em paralelo com um typecheck e um E2E curto; os tempos de 4 e 10 min foram medidos com a CPU livre.

Os números conferem com os do Agente 2 (precisão de 98,3 %, recall de 77 %, pontuação de 98,2). As notas perdidas só afetam o recall.

### Q2: "sem voz" (R3, `NO_VOICE_DB` = −20 dB)
Música de 60 s, banda no registro da melodia.

| Caso | Nível da voz | Veredito | Notas | Esperado |
|---|---|---|---|---|
| Banda só (karaokê) | −46,6 dB | sem voz | 0 | sem voz ✅ |
| Banda + melodia no piano (lead instrumental) | −47,3 dB | sem voz | 0 | sem voz ✅ |
| Silêncio | −∞ | sem voz | 0 | sem voz ✅ |
| Ruído branco −30 dBFS | −38,9 dB | sem voz | 0 | sem voz ✅ |
| Voz a 0 dB, a música toda | −4,6 dB | voz | 67 | voz ✅ |
| Voz a 0 dB só em 10 % da música | −15,6 dB | voz | 6 | voz ✅ |
| Voz a 0 dB só em **20 %** | **−10,8 dB** | voz | 14 | voz ✅ (o Agente 2 estimou cerca de −12 dB) |
| Voz a 0 dB só em 30 % | −8,5 dB | voz | 21 | voz ✅ |
| Voz a 0 dB só em 50 % | −6,3 dB | voz | 34 | voz ✅ |
| Voz a −6 dB só em 20 % | −23,4 dB | sem voz | 2 | voz → M3 |
| Voz a −10 dB, a música toda | −16,8 dB | voz | 29 | fronteira → M2 |
| Voz a −15 dB, a música toda | −21,4 dB | sem voz | 35 | fronteira → M3 |
| Voz a −20 dB, a música toda | −24,7 dB | sem voz | 49 | fronteira → M3 |

**Q2b: qualidade das notas na fronteira**

| Caso | Veredito | Corretas | Recall | Precisão |
|---|---|---|---|---|
| Voz a −10 dB, a música toda | voz | 23/81 | 28 % | 79 % |
| Voz a −15 dB | sem voz | 29/81 | 36 % | 83 % |
| Voz a −20 dB | sem voz | 29/81 | 36 % | 59 % |
| Voz a −25 dB | sem voz | 0/81 | 0 % | 0 % |
| Voz a −6 dB só em 20 % | sem voz | 2/17 | 12 % | 100 % |

### Q3: alinhamento (R5)
Sem modelo. Música de 90 s com introdução instrumental de 15 s.

- **Deslocamento conhecido.** Testados −5000, −1200, −300, −40, 0, +40, +250, +1200, +4000, +9000, +14500 e +17000 ms.
  - **Mistura com voz:** erro constante de +25 a +31 ms, com confiança de 0,91–0,92 → B2.
  - **Lead no piano:** erro constante de −36 ms, com confiança de 0,83–0,86.
  - **Karaokê sem melodia:** proposta errada, mas com confiança de 0,01–0,37, sempre abaixo de 0,5 ✅.
  - **+17 s** (fora da busca de ±15 s): confiança de 0,30–0,32 ✅.
- **Armadilhas.** Todas dão confiança baixa ou acertam:

| Caso | Resultado |
|---|---|
| Refrão repetido 2× com 10 s de distância, referência = só a 1ª frase, vídeo +300 ms | +10330 ms (a 2ª ocorrência), confiança 0,26 ✅ |
| Forma A-B-A-B-B, referência completa, vídeo +1200 ms | +1229 ms, confiança 0,74 ✅ |
| Música errada (outra melodia) | confiança 0,01 ✅ |
| Música errada sobre os **mesmos acordes**, vídeo karaokê | confiança 0,03 ✅ |
| Referência transposta +2 / +5 st | confiança 0,36 / 0,28 ✅ |
| Referência com só 6 notas, vídeo +1200 ms | +1229 ms, confiança 0,32 (acerta, mas fica abaixo do limiar) |

- **Andamento diferente (Q3c) → M1:**

| Vídeo mais lento | Proposta | Confiança | Pior erro residual |
|---|---|---|---|
| 0 % | +31 ms | 0,91 | 31 ms |
| 0,25 % | +157 ms | 0,88 | 120 ms |
| 0,5 % | +286 ms | 0,84 | 211 ms |
| 1 % | +419 ms | **0,68** | **464 ms** |
| 2 % | +1676 ms | 0,06 | 1376 ms |
| 4 % | +3322 ms | 0,14 | 2722 ms |

### Q4: limites e robustez
- **Entradas-limite, sem modelo** (vazio, 0,3 s, silêncio de 20 s, clipping ×8, conteúdo de 8 kHz reamostrado, DC +0,3, NaN/Inf):
  - nenhum pânico;
  - clipping, 8 kHz e DC mantêm 28/28 notas corretas e alinhamento de +33 ms;
  - com NaN/Inf, `transcribe` e `align` seguem corretos, mas o `voice_level_db` dá NaN → B1.
- **Taxa diferente de 44,1 kHz:** erro claro em pt-BR ("O áudio precisa estar em 44100 Hz…"). Entrada vazia: 0 amostras, sem erro.
- **10 min, sem modelo:** `transcribe` em 6,7 s (recall de 99,1 % na voz limpa) e `align` em 5,7 s (+31 ms, confiança 0,96).
- **Cancelamento** em instantes aleatórios, inclusive no meio de um trecho (6 tentativas): latência de 0,12 a **2,85 s**, abaixo de um trecho (~3,6 s). Sempre "Operação cancelada.", com progresso monotônico ✅.
- **Modelo fora da lista:** recusado ("não é um modelo conhecido") ✅.
- **Arquivo que não é modelo:** erro de modelo, com texto em inglês → B3.
- **DLL ausente:** "o componente ONNX Runtime não foi encontrado (…). Reinstale o Master Of Notes.", sem pânico ✅.

## Limites desta QA
- ❓ Voz sintética não é voz real. A precisão em voz real precisa de um trecho anotado por uma pessoa (⏳ proprietário), como já registrado no spike.
- ❓ A banda sintética não cobre mixagens com reverb e efeitos na voz-guia; o efeito disso no limiar não foi medido.
- O pico de memória é o do processo de teste, que também guarda o material gerado (cerca de 100 MB por cópia de 10 min). No app o perfil é parecido: áudio enviado + cópia com padding + saída.
