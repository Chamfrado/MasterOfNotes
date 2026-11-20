# ADR-0004 — Referência melódica automática

- **Status:** **Aceita** · 2026-11-19 · coordenador (Agente 0), com decisão do proprietário (seção final). Rascunho original: Agente 2.
- **Contexto:**
  - O proprietário quer baixar o vídeo de karaokê e cantar, sem procurar MIDI.
  - O ADR-0002 deixou a extração automática fora do MVP, com status sempre `experimental`.
  - O spike T-005-06 ([auto-reference-spike.md](../research/auto-reference-spike.md)) mediu as alternativas.

## Fatos medidos (spike)

1. **Separação de voz + o nosso YIN + segmentação** gera, a partir de um vídeo com voz-guia, uma referência boa para pontuar.
   - Na voz sintética de formantes: 97–98% de precisão; um cantor perfeito tira 98,2–98,7 contra ela.
   - Detecta com honestidade quando não há voz: 0 notas; o stem de voz fica a mais de 30 dB abaixo da mistura.
   - Custo: ~2 min de relógio por música de 4 min.
2. **Nenhum modelo de separação tem licença comercial clara dos pesos:** HTDemucs é efetivamente não comercial; MDX/UVR, Spleeter e RoFormer não declaram licença.
3. **O Basic Pitch** (Apache-2.0, 230 KB) sem separação gera referências com muitas notas espúrias: um cantor perfeito tira 70–78. Também "acha" melodia em acompanhamento puro.
4. **"Cantar a referência"** (o usuário canta uma vez; o nosso YIN + segmentação gera as notas) dá 98,6 para o cantor perfeito, sem nenhum modelo ou licença de terceiros.
5. **DTW em croma** alinha um áudio ao vídeo com mediana de 7 ms e p95 de 16 ms, em 7 s.
6. **Voz real ainda sem verdade:** no único vídeo do proprietário, a separação é consistente entre modelos (85%), mas YIN e Basic Pitch concordam em ~48% das notas.

## Decisão proposta

1. **Agora (licença limpa): modo "Cantar a referência".**
   - Grava o usuário cantando a melodia uma vez, com o vídeo tocando e o microfone já calibrado, e gera as notas pelo `audio-engine` + um segmentador novo no `scoring-engine`, puro e testável.
   - Mostra as notas sobre o vídeo para revisão.
   - O status nasce `experimental` (ADR-0002); o usuário valida.
2. **Formato:** a referência gerada guarda o `midi` **fracionário** com a afinação da gravação, aplicada só quando o desvio global é concentrado. Não muda o `mon-melody` v1, que já aceita fração.
3. **Extração automática do vídeo: adiada até existir um modelo de separação com licença comercial clara.** O runtime fica decidido desde já:
   - ONNX via `ort` (MIT/Apache), sem Python e sem torch;
   - STFT em Rust (`rustfft`);
   - processamento em trechos;
   - resultado salvo uma vez por música.

   Caminhos de licença, para o proprietário escolher:
   - (a) treinar um modelo próprio (arquitetura MDX-like, ~60 MB) com dados licenciados para uso comercial;
   - (b) obter licença escrita dos autores de um modelo existente;
   - (c) não fazer.

   Um modelo "baixado pelo próprio usuário" precisa de parecer jurídico antes; não está proposto.
4. **Basic Pitch:** não entra como fonte de referência competitiva. Fica opcional, num possível rascunho `experimental` para vídeos só instrumentais, se o coordenador entender que a Lei 19 permite modelo de terceiros Apache-2.0.
5. **Sem melodia:** quando houver separação, a regra é stem de voz < −20 dB da mistura ou < 20% de frames vozeados → mensagem "Não encontramos uma voz-guia neste vídeo." Sem separação, não se promete detecção.

## Consequências

- **O modo "Cantar a referência" custa pouco:**
  - não muda dependências nem o instalador;
  - reaproveita a captura, a calibração e a latência que já existem;
  - a qualidade da referência depende de quem canta, por isso a revisão é obrigatória.
- **A extração automática fica parada** numa decisão de licença ou de dados, não de engenharia. A engenharia está viável:
  - +ONNX Runtime no instalador (❓ ~10–20 MB);
  - ~2 min de CPU por música.
- **Antes de qualquer extração automática ir para o produto,** é preciso uma validação com voz real: trechos anotados à mão.

## Alternativas consideradas

- **HTDemucs embarcado:** melhor qualidade medida, mas pesos não comerciais e dependência do PyTorch. Descartado.
- **Basic Pitch sem separação como fonte principal:** licença ok, qualidade insuficiente para pontuar (70–78). Descartado como principal.
- **Áudio original do usuário + DTW:** o alinhamento funciona, mas o original ainda precisa de separação para virar notas, com o mesmo bloqueio. Útil depois para alinhar MIDI ou UltraStar importado (❓ não medido).


## Decisão do proprietário (2026-11-19)

O proprietário escolheu seguir com **três frentes**, nesta ordem:

| Frente | Entrega | Escopo |
|---|---|---|
| **A. Cantar a referência** | 0.1.2 | Performance em modo `reference_capture`: o app ouve alguém cantar a melodia sobre o vídeo e grava **só as notas** (nunca o áudio, Lei 14) como referência `manual`, status `experimental`. Revisão e validação pela tela Referência, como hoje. Contrato em [contracts.md §2.4](../architecture/contracts.md). |
| **B. Modelo opcional baixado pelo usuário** | 0.1.3 | O app **não distribui** pesos de terceiros. O usuário baixa da fonte original (ex.: MDX-Net ONNX), vê e aceita os termos da fonte e aponta o arquivo para o app, que confere o checksum de modelos conhecidos e roda via ONNX Runtime (`ort`), em trechos. Só para vídeos **com voz**; sem voz, o app diz que não dá. Resultado `extracted` + `experimental`, com validação pelo usuário. **Antes de comercializar o produto, o proprietário faz a análise jurídica desse modo.** |
| **C. Modelo próprio** | pesquisa de longo prazo | Treinar separação com dados de licença comercial. Exige orçamento de dados e GPU; reavaliar quando o produto tiver escala. Não bloqueia A nem B. |

O Basic Pitch sem separação fica **fora** (insuficiente: 70–78 para um cantor perfeito). Vídeos só instrumentais, sem melodia, continuam exigindo MIDI/JSON ou o modo A.
