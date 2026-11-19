# ADR-0002 — Referência melódica

- **Status:** Aceita · 2026-11-19 · Agente 0
- **Contexto:** vídeo de karaokê com letra embutida não contém as notas. Pontuação competitiva exige saber a nota esperada em cada instante.

## Decisão

Ordem de preferência para o MVP:

1. **Importação MIDI** (`.mid`/`.midi`, usuário escolhe a trilha vocal) e **JSON interno** (`mon-melody` v1). Precisão total da fonte, custo computacional zero, sem problema de licença de modelo.
2. **Sincronização manual**: offset global ajustável com pré-visualização (notas sobre o vídeo + tom de referência opcional). O usuário marca a referência como *validada* depois de conferir.
3. **Extração automática** (pitch da faixa original / separação de fontes): **fora do MVP**. Quando existir, gera sempre status `experimental`, nunca `validated` automaticamente.

Formato interno `mon-melody` v1:

```json
{ "format": "mon-melody", "version": 1, "notes": [ { "startMs": 12000, "durMs": 450, "midi": 64, "lyric": "a" } ] }
```

- `midi` pode ser fracionário (microtonalidade); notas ordenadas, sem sobreposição (polifonia → manter a nota mais aguda, documentado).
- `offset_ms` fica no banco, não no arquivo, e incrementa `revision`.

## Regras de validação

- Só `validated` permite performance competitiva (entra em ranking).
- Qualquer mudança de notas ou offset → `revision + 1` e status volta para `experimental`.
- Rankings agrupam por (referência, revisão, versão do algoritmo, dificuldade): resultados de revisões diferentes nunca se misturam.

## Alternativas

- Separação de voz por IA (Demucs/Spleeter): modelos grandes, CPU alta, licenças a verificar, precisão variável. Adiado, não descartado.
- UltraStar `.txt`: formato popular com notas+letra; importar é barato. Candidato a segunda importação após o MVP (verificar BPM/GAP).
