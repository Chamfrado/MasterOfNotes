// ARCHIVED COPY (not compiled). Measures crates/song-analysis @ agent-2/work 7876130 (T-012-05, findings in song-analysis-qa.md).
// To reproduce: copy to crates/song-analysis/tests/qa_probe.rs; model UVR-MDX-NET-Voc_FT.onnx (URL and SHA-256 in known_models())
// and onnxruntime.dll 1.31.1 (apps/desktop/src-tauri/scripts/fetch-onnxruntime.mjs), both outside the repo.

//! QA probes (Agent 4, T-012-05): independent measurement of crates/song-analysis (owner: Agent 2) against the
//! SPEC-012 contract (contracts.md §2.5). MEASUREMENT ONLY, local copy, NOT committed in the crate.
//! Install: copy to crates/song-analysis/tests/qa_probe.rs (dev-dep scoring-engine is already a dep of the crate).
//! Run without model:  cargo test -p song-analysis --release --test qa_probe -- --nocapture --test-threads=1
//! With the real model (scratchpad, deleted afterwards): MON_QA_MODEL=<.onnx> ORT_DYLIB_PATH=<onnxruntime.dll> (same command);
//! Q4c one case per process: MON_QA_CASE=4min|10min|cancel|rates|unknown (peak memory = PeakWorkingSet64 of the test exe).
//! Findings: docs/testing/evidence/song-analysis-qa.md
//! All audio is synthetic and authored here (Law 19). Truth = the notes we synthesise.

use scoring_engine::{Difficulty, MelodyReference, Note, Scorer, VoiceFrame};
use song_analysis::{align_offset_ms, known_models, transcribe, voice_level_db, Separator, NO_VOICE_DB};
/// The UI flags proposals below this (Reference.tsx LOW_CONFIDENCE).
const LOW_CONFIDENCE: f32 = 0.5;
use std::f64::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const SR: u32 = 44_100; // upload format (R6)

// ---------- deterministic noise ----------
struct Rng(u64);
impl Rng {
    /// Uniform in [-1, 1).
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        2.0 * ((self.0 >> 11) as f64 / (1u64 << 53) as f64) - 1.0
    }
}
struct Wobble {
    rng: Rng,
    step: usize,
    a: f64,
    b: f64,
    i: usize,
}
impl Wobble {
    fn new(seed: u64, step: usize) -> Self {
        let mut rng = Rng(seed);
        let (a, b) = (rng.next(), rng.next());
        Self { rng, step, a, b, i: 0 }
    }
    fn next(&mut self) -> f64 {
        if self.i == self.step {
            self.i = 0;
            self.a = self.b;
            self.b = self.rng.next();
        }
        let x = self.i as f64 / self.step as f64;
        self.i += 1;
        self.a + (self.b - self.a) * x
    }
}

fn midi_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}
fn ms(n: usize) -> f64 {
    n as f64 * 1000.0 / SR as f64
}
fn samples(t_ms: f64) -> usize {
    (t_ms / 1000.0 * SR as f64) as usize
}
fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}
fn gain_db(x: &mut [f32], db: f64) {
    let g = 10f64.powf(db / 20.0) as f32;
    x.iter_mut().for_each(|s| *s *= g);
}
fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
    (0..a.len().max(b.len())).map(|i| a.get(i).unwrap_or(&0.0) + b.get(i).unwrap_or(&0.0)).collect()
}

// ---------- melody (truth) ----------
/// (start_ms, dur_ms, midi). Pop-like: C major walk C4–A4, 180–900 ms notes, rests, repeated notes, phrases of 8.
type Song = Vec<(f64, f64, f64)>;

fn melody(seed: u64, len_ms: f64, start_ms: f64) -> Song {
    let scale = [60.0, 62.0, 64.0, 65.0, 67.0, 69.0];
    let mut rng = Rng(seed);
    let (mut t, mut i, mut out) = (start_ms, 2usize, Song::new());
    while t < len_ms - 1500.0 {
        let n = out.len();
        let step = (rng.next() * 2.4).round() as i64; // −2..2 scale steps, 0 = repeated note
        i = (i as i64 + step).clamp(0, scale.len() as i64 - 1) as usize;
        let dur = 180.0 + (rng.next() + 1.0) * 360.0;
        out.push((t, dur, scale[i]));
        // Repeated notes are separated by a short consonant gap; phrase ends get a rest.
        t += dur + if n % 8 == 7 { 600.0 + (rng.next() + 1.0) * 400.0 } else { 40.0 };
    }
    out
}

fn note_at(song: &Song, t: f64) -> Option<(usize, (f64, f64, f64))> {
    song.iter().copied().enumerate().find(|&(_, (s, d, _))| t >= s && t < s + d)
}

fn reference(song: &Song) -> MelodyReference {
    MelodyReference {
        format_version: 1,
        notes: song.iter().map(|&(s, d, m)| Note { start_ms: s as u32, dur_ms: d as u32, midi: m as f32, lyric: None }).collect(),
    }
}

// ---------- realistic voice (from the scoring QA rounds, plus vowel changes per note) ----------
/// F1, F2, F3 (Hz) of /a e i o u/.
const VOWELS: [[f64; 3]; 5] = [[700.0, 1220.0, 2600.0], [480.0, 1900.0, 2500.0], [300.0, 2250.0, 3000.0], [500.0, 900.0, 2500.0], [320.0, 800.0, 2400.0]];
fn formant_gain(f: f64, v: &[f64; 3]) -> f64 {
    [(v[0], 8.0, 130.0), (v[1], 6.0, 150.0), (v[2], 3.0, 250.0)].iter().map(|&(fc, a, bw)| a * (-((f - fc) / bw).powi(2)).exp()).sum::<f64>() + 1.0
}

/// Sings `song`: jitter 1 %, shimmer 10 %, drift ±15 c, scoop −80 c over 60 ms, vibrato ±40 c at 5.5 Hz after 300 ms,
/// a 25 ms noise consonant at each onset, vowel changing per note. `shift_ms` delays everything.
fn sing(song: &Song, len_ms: f64, amp: f64) -> Vec<f32> {
    let n = samples(len_ms);
    let (mut jit, mut shim, mut drift, mut cons) = (Wobble::new(0xA11CE, 220), Wobble::new(0xB0B, 220), Wobble::new(0xD21F7, 22_050), Rng(0xC0750));
    let mut phase = 0.0f64;
    (0..n)
        .map(|k| {
            let t = ms(k);
            let (j, s, d) = (jit.next(), shim.next(), drift.next());
            let Some((idx, (st, _, m))) = note_at(song, t) else {
                phase = 0.0;
                return 0.0;
            };
            let x = t - st;
            if x < 25.0 {
                return (amp * 0.25 * cons.next()) as f32; // consonant
            }
            let cents = d * 15.0 - 80.0 * (1.0 - ((x - 25.0) / 60.0).min(1.0)) + if x > 300.0 { 40.0 * (TAU * 5.5 * x / 1000.0).sin() } else { 0.0 };
            let f0 = midi_hz(m + cents / 100.0) * (1.0 + j * 0.01);
            phase = (phase + TAU * f0 / SR as f64) % (TAU * 1e6);
            let v = &VOWELS[idx % VOWELS.len()];
            let hs = ((5000.0 / f0) as u32).max(1);
            let (mut acc, mut norm) = (0.0, 0.0);
            for h in 1..=hs {
                let g = formant_gain(f0 * h as f64, v) / h as f64;
                acc += g * (phase * h as f64).sin();
                norm += g;
            }
            (amp * (1.0 + s * 0.1) * 1.5 * acc / norm) as f32
        })
        .collect()
}

// ---------- accompaniment ----------
/// Piano-like tone: harmonics with 1/h amplitude and exponential decay.
fn pluck(out: &mut [f32], t_ms: f64, dur_ms: f64, midi: f64, amp: f64) {
    let (a, b) = (samples(t_ms), samples(t_ms + dur_ms).min(out.len()));
    let f = midi_hz(midi);
    for k in a..b {
        let x = (k - a) as f64 / SR as f64;
        let env = (-x * 3.0).exp() * (1.0 - (-x * 400.0).exp());
        let mut acc = 0.0;
        for h in 1..=6 {
            acc += (TAU * f * h as f64 * x).sin() / h as f64;
        }
        out[k] += (amp * env * acc * 0.5) as f32;
    }
}

/// I–V–vi–IV in C, 2 s per chord: pad in the melody register (C3–A4, the hard case), bass, kick/snare/hi-hat.
/// `lead` = the melody also played by the piano (instrumental lead: must NOT be taken as a voice).
fn band(len_ms: f64, lead: Option<&Song>) -> Vec<f32> {
    let n = samples(len_ms);
    let mut out = vec![0f32; n];
    let chords = [[60.0, 64.0, 67.0], [55.0, 59.0, 62.0], [57.0, 60.0, 64.0], [53.0, 57.0, 60.0]];
    let mut noise = Rng(0xD2C5);
    let mut t = 0.0;
    let mut c = 0;
    while t < len_ms {
        let ch = chords[c % 4];
        for beat in 0..4 {
            let bt = t + beat as f64 * 500.0;
            for &m in &ch {
                pluck(&mut out, bt, 500.0, m, 0.05);
            }
            pluck(&mut out, bt, 500.0, ch[0] - 24.0, 0.12); // bass
            // Kick on 1 and 3, snare on 2 and 4, hi-hat every 8th.
            let k0 = samples(bt);
            for k in k0..(k0 + samples(120.0)).min(n) {
                let x = (k - k0) as f64 / SR as f64;
                out[k] += if beat % 2 == 0 {
                    (0.25 * (-x * 30.0).exp() * (TAU * (50.0 + 60.0 * (-x * 40.0).exp()) * x).sin()) as f32
                } else {
                    (0.08 * (-x * 25.0).exp() * noise.next()) as f32
                };
            }
            for half in [0.0, 250.0] {
                let h0 = samples(bt + half);
                for k in h0..(h0 + samples(30.0)).min(n) {
                    out[k] += (0.02 * noise.next()) as f32;
                }
            }
        }
        t += 2000.0;
        c += 1;
    }
    if let Some(song) = lead {
        for &(s, d, m) in song {
            pluck(&mut out, s, d, m, 0.1);
        }
    }
    out
}

// ---------- metrics ----------
/// mir_eval-like 1:1 matching (the spike's method): onset within ±100 ms and pitch within ±50 c.
/// Returns (correct, recall %, precision %, spurious).
fn match_notes(truth: &Song, got: &[Note]) -> (usize, f64, f64, usize) {
    let mut used = vec![false; got.len()];
    let mut correct = 0;
    for &(s, _, m) in truth {
        let best = got
            .iter()
            .enumerate()
            .filter(|(i, g)| !used[*i] && (g.start_ms as f64 - s).abs() <= 100.0 && ((g.midi as f64 - m) * 100.0).abs() <= 50.0)
            .min_by(|a, b| (a.1.start_ms as f64 - s).abs().total_cmp(&(b.1.start_ms as f64 - s).abs()));
        if let Some((i, _)) = best {
            used[i] = true;
            correct += 1;
        }
    }
    let spurious = got.iter().enumerate().filter(|(i, g)| !used[*i] && !truth.iter().any(|&(s, d, _)| (g.start_ms as f64) < s + d && (g.start_ms + g.dur_ms) as f64 > s)).count();
    (correct, 100.0 * correct as f64 / truth.len().max(1) as f64, 100.0 * correct as f64 / got.len().max(1) as f64, spurious)
}

/// A perfect singer (ideal frames following the truth) scored against the generated reference (Normal).
fn simulated_score(truth: &Song, got: &[Note], len_ms: f64) -> f32 {
    let r = MelodyReference { format_version: 1, notes: got.to_vec() };
    let mut s = Scorer::new(&r, 0, Difficulty::Normal);
    for i in 0..(len_ms / 10.0) as usize {
        let t = i as f64 * 10.0;
        let m = note_at(truth, t).map(|(_, n)| n.2);
        s.push(VoiceFrame { t_ms: t, f0_hz: m.map(|m| midi_hz(m) as f32), confidence: 0.95, voiced: m.is_some() });
    }
    s.finish().score
}

fn header(t: &str) {
    eprintln!("\n### {t}");
}

fn model() -> Option<Separator> {
    let path = std::env::var("MON_QA_MODEL").ok()?;
    let p = std::path::Path::new(&path);
    let model = known_models().iter().find(|m| p.file_name().is_some_and(|f| f == m.file_name)).expect("MON_QA_MODEL is not a known model file");
    Some(Separator::load(p, model).expect("load model"))
}

fn separate(sep: &mut Separator, mix: &[f32]) -> (Vec<f32>, f64) {
    let cancel = AtomicBool::new(false);
    let t = Instant::now();
    let v = sep.vocals(mix, SR, &mut |_| {}, &cancel).expect("separation");
    (v, t.elapsed().as_secs_f64())
}

// ======================= Q1 precision on synthetic mixtures (R7: ≥ 95 % correct) =======================
#[test]
fn q1_precision_on_mixtures() {
    let len = 60_000.0;
    let song = melody(0x5EED1, len, 2000.0);
    let voice = sing(&song, len, 0.2);
    header(&format!("Q1a transcrição SEM separação (voz limpa, {} notas): teto do transcribe", song.len()));
    let got = transcribe(&voice, SR);
    let (c, r, p, sp) = match_notes(&song, &got);
    eprintln!("corretas {c}, recall {r:.1} %, precisão {p:.1} %, espúrias {sp}, pontuação simulada {:.1}", simulated_score(&song, &got, len));

    let Some(mut sep) = model() else {
        eprintln!("(MON_QA_MODEL não definido: Q1b pulado)");
        return;
    };
    header("Q1b pipeline completo (separação + transcribe) por razão voz/banda");
    eprintln!("| voz − banda | voz (dB) | corretas | recall | precisão | espúrias | pontuação simulada | tempo (s) |\n|---|---|---|---|---|---|---|---|");
    for db in [6.0, 0.0, -6.0, -10.0] {
        let mut b = band(len, None);
        let g = 20.0 * (rms(&voice) / rms(&b)).log10() - db;
        gain_db(&mut b, g);
        let m = mix(&voice, &b);
        let (v, secs) = separate(&mut sep, &m);
        let got = transcribe(&v, SR);
        let (c, r, p, sp) = match_notes(&song, &got);
        eprintln!("| {db:+} dB | {:.1} | {c}/{} | {r:.1} % | {p:.1} % | {sp} | {:.1} | {secs:.1} |", voice_level_db(&v, &m), song.len(), simulated_score(&song, &got, len));
    }
}

// ======================= Q2 "no voice" (R3) =======================
/// Voice only inside [from, to) of the song (e.g. a guide vocal only in one chorus).
fn voice_window(voice: &[f32], from: f64, to: f64) -> Vec<f32> {
    let (a, b) = (samples(from), samples(to).min(voice.len()));
    voice.iter().enumerate().map(|(i, &s)| if i >= a && i < b { s } else { 0.0 }).collect()
}

#[test]
fn q2_no_voice() {
    let Some(mut sep) = model() else {
        eprintln!("(MON_QA_MODEL não definido: Q2 pulado)");
        return;
    };
    let len = 60_000.0;
    let song = melody(0x5EED2, len, 2000.0);
    let voice = sing(&song, len, 0.2);
    let b = band(len, None);
    // Voice at 0 dB of the band where it sings (level matched on the sung part only).
    let at_db = |v: &[f32], db: f64| -> Vec<f32> {
        let mut v = v.to_vec();
        let sung: Vec<f32> = v.iter().copied().filter(|s| *s != 0.0).collect();
        gain_db(&mut v, 20.0 * (rms(&b) / rms(&sung)).log10() + db);
        v
    };
    header(&format!("Q2 nível de voz × limiar NO_VOICE_DB = {NO_VOICE_DB} dB (música de 60 s, banda no registro da melodia)"));
    eprintln!("| caso | voz (dB) | sem voz? | notas | esperado |\n|---|---|---|---|---|");
    let mut noise = Rng(7);
    let mut cases: Vec<(String, Vec<f32>, &str)> = vec![
        ("banda só (karaokê)".into(), b.clone(), "sem voz"),
        ("banda + melodia no piano (lead instrumental)".into(), band(len, Some(&song)), "sem voz"),
        ("silêncio".into(), vec![0.0; samples(len)], "sem voz"),
        ("ruído branco −30 dBFS".into(), (0..samples(len)).map(|_| (0.03 * noise.next()) as f32).collect(), "sem voz"),
        ("voz a 0 dB da banda, a música toda".into(), mix(&at_db(&voice, 0.0), &b), "voz"),
    ];
    for frac in [0.1, 0.2, 0.3, 0.5] {
        let v = voice_window(&voice, len * 0.4, len * (0.4 + frac));
        cases.push((format!("voz a 0 dB só em {:.0} % da música", frac * 100.0), mix(&at_db(&v, 0.0), &b), "voz"));
    }
    let v20 = voice_window(&voice, len * 0.4, len * 0.6);
    cases.push(("voz a −6 dB só em 20 % da música".into(), mix(&at_db(&v20, -6.0), &b), "voz"));
    for db in [-10.0, -15.0, -20.0] {
        cases.push((format!("voz a {db} dB da banda, a música toda"), mix(&at_db(&voice, db), &b), "fronteira"));
    }
    for (name, m, expected) in &cases {
        let (v, _) = separate(&mut sep, m);
        let lvl = voice_level_db(&v, m);
        eprintln!("| {name} | {lvl:.1} | {} | {} | {expected} |", if lvl < NO_VOICE_DB { "sim" } else { "não" }, transcribe(&v, SR).len());
    }
}

// ======================= Q3 alignment (R5: ±30 ms) — no model needed =======================
fn shifted(x: &[f32], d: f64) -> Vec<f32> {
    let k = samples(d.abs());
    if d >= 0.0 { [vec![0.0; k], x.to_vec()].concat() } else { x[k.min(x.len())..].to_vec() }
}
fn cell(notes: &[Note], m: &[f32], d: f64) -> String {
    let r = align_offset_ms(notes, m, SR);
    let ok = (r.offset_ms as f64 - d).abs() <= 30.0;
    let flag = match (ok, r.confidence >= LOW_CONFIDENCE) {
        (true, _) => "",
        (false, true) => " **ERRADO COM CONFIANÇA**",
        (false, false) => " (errado, conf. baixa)",
    };
    format!("{:+} / {:+.0} / {:.2}{flag}", r.offset_ms, r.offset_ms as f64 - d, r.confidence)
}

#[test]
fn q3_alignment() {
    let len = 90_000.0;
    let song = melody(0x5EED3, len, 15_000.0); // 15 s instrumental intro
    let notes = reference(&song).notes;
    let with_voice = mix(&sing(&song, len, 0.2), &band(len, None));
    let karaoke = band(len, None);
    let lead = band(len, Some(&song));
    header(&format!("Q3a deslocamento conhecido D (o vídeo atrasa as notas em D ⇒ offset correto +D); célula = proposto / erro / confiança; LOW_CONFIDENCE = {LOW_CONFIDENCE}"));
    eprintln!("| D (ms) | mistura com voz | karaokê (sem melodia) | lead no piano | tempo (s) |\n|---|---|---|---|---|");
    for d in [-5000.0, -1200.0, -300.0, -40.0, 0.0, 40.0, 250.0, 1200.0, 4000.0, 9000.0, 14_500.0, 17_000.0] {
        let t = Instant::now();
        let cells: Vec<String> = [&with_voice, &karaoke, &lead].iter().map(|m| cell(&notes, &shifted(m, d), d)).collect();
        eprintln!("| {d:+} | {} | {:.1} |", cells.join(" | "), t.elapsed().as_secs_f64() / 3.0);
    }

    header("Q3b armadilhas (devem dar confiança BAIXA, ou acertar)");
    eprintln!("| caso | proposto / erro / conf. |\n|---|---|");
    // Chorus repeated: the same phrase twice 10 s apart; reference = only the first phrase, video delayed 300 ms.
    let phrase = melody(0x5EED4, 10_000.0, 0.0);
    let twice: Song = phrase.iter().map(|&(s, d, m)| (s + 2000.0, d, m)).chain(phrase.iter().map(|&(s, d, m)| (s + 12_000.0, d, m))).collect();
    let m2 = shifted(&mix(&sing(&twice, 24_000.0, 0.2), &band(24_000.0, None)), 300.0);
    let first: Vec<Note> = reference(&twice).notes[..phrase.len()].to_vec();
    let r = align_offset_ms(&first, &m2, SR);
    eprintln!("| refrão repetido (2× a mesma frase, 10 s), referência = só a 1ª frase, vídeo +300 ms (certos: +300 ou +10300) | {:+} / conf. {:.2} |", r.offset_ms, r.confidence);
    // Whole song = verse/chorus with the chorus 3×: reference complete, so it must align (the repeats are inside).
    let chorus = melody(0x5EED5, 8_000.0, 0.0);
    let verse = melody(0x5EED6, 8_000.0, 0.0);
    let mut form = Song::new();
    for (k, part) in [&verse, &chorus, &verse, &chorus, &chorus].iter().enumerate() {
        form.extend(part.iter().map(|&(s, d, m)| (s + 3000.0 + k as f64 * 9000.0, d, m)));
    }
    let mf = mix(&sing(&form, 50_000.0, 0.2), &band(50_000.0, None));
    eprintln!("| forma A-B-A-B-B, referência completa, vídeo +1200 ms | {} |", cell(&reference(&form).notes, &shifted(&mf, 1200.0), 1200.0));
    // Wrong song, different chords: unrelated melody.
    let other = reference(&melody(0xBAD, len, 15_000.0)).notes;
    let r = align_offset_ms(&other, &with_voice, SR);
    eprintln!("| música errada (outra melodia) | {:+} / conf. {:.2} |", r.offset_ms, r.confidence);
    // Wrong song over the SAME chord progression (same key): the band's chroma matches any melody in C.
    let r = align_offset_ms(&other, &karaoke, SR);
    eprintln!("| música errada sobre os mesmos acordes, vídeo karaokê | {:+} / conf. {:.2} |", r.offset_ms, r.confidence);
    // Reference transposed (MIDI in another key): +2 and +5 semitones.
    for st in [2.0, 5.0] {
        let tr: Vec<Note> = notes.iter().map(|n| Note { midi: n.midi + st as f32, ..n.clone() }).collect();
        eprintln!("| referência transposta +{st} st, vídeo +1200 ms | {} |", cell(&tr, &shifted(&with_voice, 1200.0), 1200.0));
    }
    // Sparse reference: only 6 notes.
    eprintln!("| referência com só 6 notas, vídeo +1200 ms | {} |", cell(&notes[..6], &shifted(&with_voice, 1200.0), 1200.0));
    // Tempo mismatch: the video is 1 % slower than the reference.
    let slow: Song = song.iter().map(|&(s, d, m)| (s * 1.01, d * 1.01, m)).collect();
    let r = align_offset_ms(&notes, &mix(&sing(&slow, len * 1.01, 0.2), &band(len * 1.01, None)), SR);
    eprintln!(
        "| vídeo 1 % mais lento (desvio verdadeiro cresce de +150 a +{:.0} ms) | {:+} / conf. {:.2} |",
        song.last().map_or(0.0, |n| n.0 * 0.01),
        r.offset_ms,
        r.confidence
    );
}

// ======================= Q4 limits and robustness =======================
#[test]
fn q4_limits() {
    header("Q4a entradas-limite (sem modelo): não pode haver pânico");
    let len = 20_000.0;
    let song = melody(0x5EED5, len, 1000.0);
    let voice = sing(&song, len, 0.2);
    let notes = reference(&song).notes;
    let clipped: Vec<f32> = voice.iter().map(|s| (s * 8.0).clamp(-1.0, 1.0)).collect();
    // 8 kHz content: moving average over 5 samples (low-pass) + sample-and-hold ×5 (band-limited, back at 44.1 kHz).
    let lo: Vec<f32> = voice.chunks(5).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    let band8k: Vec<f32> = (0..voice.len()).map(|i| lo[(i / 5).min(lo.len() - 1)]).collect();
    let mut nan = voice.clone();
    nan[1000] = f32::NAN;
    nan[2000] = f32::INFINITY;
    let cases: Vec<(&str, Vec<f32>)> = vec![
        ("vazio", vec![]),
        ("0,3 s", voice[..samples(300.0)].to_vec()),
        ("silêncio 20 s", vec![0.0; voice.len()]),
        ("clipping ×8", clipped),
        ("conteúdo de 8 kHz reamostrado", band8k),
        ("DC +0,3", voice.iter().map(|s| s + 0.3).collect()),
        ("NaN e Inf no meio", nan),
    ];
    for (name, x) in &cases {
        let t = Instant::now();
        let got = std::panic::catch_unwind(|| transcribe(x, SR));
        let al = std::panic::catch_unwind(|| align_offset_ms(&notes, x, SR));
        let lvl = std::panic::catch_unwind(|| voice_level_db(x, x));
        eprintln!(
            "- {name}: transcribe {} | align {} | voice_level {} | {:.2} s",
            got.map_or("PÂNICO".into(), |g| {
                let (c, r, p, _) = match_notes(&song, &g);
                format!("{} notas, corretas {c} (recall {r:.0} %, precisão {p:.0} %)", g.len())
            }),
            al.map_or("PÂNICO".into(), |a| format!("{:+} ms, conf. {:.2}", a.offset_ms, a.confidence)),
            lvl.map_or("PÂNICO".into(), |l| format!("{l:.1} dB")),
            t.elapsed().as_secs_f64()
        );
    }
    header("Q4b 10 minutos (sem modelo): tempo de transcribe e align");
    let long = 600_000.0;
    let song = melody(0x5EED6, long, 5000.0);
    let clean = sing(&song, long, 0.2);
    let m = mix(&clean, &band(long, None));
    let t = Instant::now();
    let got = transcribe(&clean, SR);
    eprintln!("transcribe 10 min (voz limpa): {:.1} s, {} notas, recall {:.1} %", t.elapsed().as_secs_f64(), got.len(), match_notes(&song, &got).1);
    let t = Instant::now();
    let a = align_offset_ms(&reference(&song).notes, &m, SR);
    eprintln!("align 10 min: {:.1} s, proposto {:+} ms (certo 0), conf. {:.2}", t.elapsed().as_secs_f64(), a.offset_ms, a.confidence);
}

/// Q4c with the model, one case per run (`MON_QA_CASE`), so the peak memory of each is measured from outside
/// (PeakWorkingSet64 of this test process): "4min", "10min", "cancel", "rates", "unknown".
#[test]
fn q4c_model_limits() {
    let Ok(case) = std::env::var("MON_QA_CASE") else {
        eprintln!("(MON_QA_CASE não definido: Q4c pulado)");
        return;
    };
    if case == "unknown" {
        // A KnownModel that is not in known_models(), and a known entry pointing at a file that is not a model.
        let fake = song_analysis::KnownModel { id: "00", ..known_models()[0] };
        let p = std::path::PathBuf::from(std::env::var("MON_QA_MODEL").unwrap_or_default());
        eprintln!("modelo fora da lista: {}", Separator::load(&p, &fake).err().map_or("CARREGOU (ruim)".into(), |e| e.to_string()));
        let dll = std::path::PathBuf::from(std::env::var("ORT_DYLIB_PATH").unwrap_or_default());
        eprintln!("arquivo que não é modelo (a DLL): {}", Separator::load(&dll, &known_models()[0]).err().map_or("CARREGOU (ruim)".into(), |e| e.to_string()));
        return;
    }
    let mut sep = model().expect("MON_QA_MODEL");
    match case.as_str() {
        "4min" | "10min" => {
            let mins = if case == "4min" { 4.0 } else { 10.0 };
            let l = mins * 60_000.0;
            let s = melody(0x5EED7, l, 5000.0);
            let m = mix(&sing(&s, l, 0.2), &band(l, None));
            let (v, secs) = separate(&mut sep, &m);
            let t = Instant::now();
            let got = transcribe(&v, SR);
            let tr = t.elapsed().as_secs_f64();
            let (c, r, p, sp) = match_notes(&s, &got);
            eprintln!(
                "{mins} min: separação {secs:.1} s + transcrição {tr:.1} s; voz {:.1} dB; corretas {c}/{} (recall {r:.1} %, precisão {p:.1} %, espúrias {sp}); pontuação simulada {:.1}",
                voice_level_db(&v, &m),
                s.len(),
                simulated_score(&s, &got, l)
            );
        }
        "cancel" => {
            // Cancel at pseudo-random instants inside a chunk (not only between chunks): latency = flag set → return.
            let l = 60_000.0;
            let s = melody(0x5EED8, l, 2000.0);
            let m = mix(&sing(&s, l, 0.2), &band(l, None));
            let mut rng = Rng(0xCA9CE1);
            for _ in 0..6 {
                let after = 2.0 + (rng.next() + 1.0) * 4.0; // 2–10 s into the run
                let cancel = AtomicBool::new(false);
                let t = Instant::now();
                let watcher = std::thread::scope(|sc| {
                    let h = sc.spawn(|| {
                        std::thread::sleep(std::time::Duration::from_secs_f64(after));
                        cancel.store(true, Ordering::Relaxed);
                        Instant::now()
                    });
                    let (mut last, mut mono, mut calls) = (-1.0f32, true, 0);
                    let res = sep.vocals(&m, SR, &mut |p| {
                        calls += 1;
                        mono &= p >= last && (0.0..=1.0).contains(&p);
                        last = p;
                    }, &cancel);
                    let done = Instant::now();
                    let set = h.join().expect("watcher");
                    (res.err().map(|e| e.to_string()), done.saturating_duration_since(set).as_secs_f64(), mono, calls, last)
                });
                let (err, latency, mono, calls, last) = watcher;
                eprintln!(
                    "cancelar aos {after:.1} s: {} | latência {latency:.2} s | progresso monotônico {mono}, {calls} chamadas, último {last:.2} | total {:.1} s",
                    err.unwrap_or_else(|| "terminou sem cancelar".into()),
                    t.elapsed().as_secs_f64()
                );
            }
        }
        "rates" => {
            let m = mix(&sing(&melody(1, 10_000.0, 500.0), 10_000.0, 0.2), &band(10_000.0, None));
            for sr in [22_050u32, 48_000] {
                let cancel = AtomicBool::new(false);
                let r = sep.vocals(&m, sr, &mut |_| {}, &cancel);
                eprintln!("taxa {sr} Hz: {}", r.map_or_else(|e| format!("erro: {e}"), |v| format!("{} amostras", v.len())));
            }
            let cancel = AtomicBool::new(false);
            eprintln!("vazio: {}", sep.vocals(&[], SR, &mut |_| {}, &cancel).map_or_else(|e| format!("erro: {e}"), |v| format!("{} amostras", v.len())));
            let mut nan = m.clone();
            nan[5000] = f32::NAN;
            let r = sep.vocals(&nan, SR, &mut |_| {}, &cancel);
            eprintln!("NaN na entrada: {}", r.map_or_else(|e| format!("erro: {e}"), |v| format!("{} amostras, {} NaN na saída", v.len(), v.iter().filter(|s| s.is_nan()).count())));
        }
        other => panic!("MON_QA_CASE desconhecido: {other}"),
    }
}

/// Q2b: are the notes thrown away by the "no voice" verdict good ones? (precision at the border)
#[test]
fn q2b_border_quality() {
    let Some(mut sep) = model() else {
        eprintln!("(MON_QA_MODEL não definido: Q2b pulado)");
        return;
    };
    let len = 60_000.0;
    let song = melody(0x5EED2, len, 2000.0);
    let voice = sing(&song, len, 0.2);
    let b = band(len, None);
    header("Q2b qualidade das notas na fronteira do limiar (mesma música da Q2)");
    eprintln!("| caso | voz (dB) | veredito | notas | corretas | recall | precisão | pontuação simulada |\n|---|---|---|---|---|---|---|---|");
    let level = |v: &[f32], db: f64| -> Vec<f32> {
        let mut v = v.to_vec();
        let sung: Vec<f32> = v.iter().copied().filter(|s| *s != 0.0).collect();
        gain_db(&mut v, 20.0 * (rms(&b) / rms(&sung)).log10() + db);
        v
    };
    let win = voice_window(&voice, len * 0.4, len * 0.6);
    let win_truth: Song = song.iter().copied().filter(|&(s, _, _)| s >= len * 0.4 && s < len * 0.6).collect();
    let cases: Vec<(String, Vec<f32>, &Song)> = vec![
        ("voz a −10 dB, a música toda".into(), level(&voice, -10.0), &song),
        ("voz a −15 dB, a música toda".into(), level(&voice, -15.0), &song),
        ("voz a −20 dB, a música toda".into(), level(&voice, -20.0), &song),
        ("voz a −25 dB, a música toda".into(), level(&voice, -25.0), &song),
        ("voz a −6 dB só em 20 %".into(), level(&win, -6.0), &win_truth),
        ("voz a −10 dB só em 20 %".into(), level(&win, -10.0), &win_truth),
    ];
    for (name, v, truth) in &cases {
        let m = mix(v, &b);
        let (st, _) = separate(&mut sep, &m);
        let lvl = voice_level_db(&st, &m);
        let got = transcribe(&st, SR);
        let (c, r, p, _) = match_notes(truth, &got);
        eprintln!(
            "| {name} | {lvl:.1} | {} | {} | {c}/{} | {r:.0} % | {p:.0} % | {:.1} |",
            if lvl < NO_VOICE_DB { "sem voz" } else { "voz" },
            got.len(),
            truth.len(),
            simulated_score(truth, &got, len)
        );
    }
}

/// Q3c: tempo mismatch (reference at another tempo than the video). Does the confidence drop?
#[test]
fn q3c_tempo_mismatch() {
    let len = 90_000.0;
    let song = melody(0x5EED3, len, 15_000.0);
    let notes = reference(&song).notes;
    header("Q3c andamento diferente: o vídeo é k % mais lento que a referência (offset ideal muda ao longo da música)");
    eprintln!("| k | proposto | conf. | desvio verdadeiro no início / fim | pior erro residual |\n|---|---|---|---|---|");
    for k in [0.0, 0.25, 0.5, 1.0, 2.0, 4.0] {
        let f = 1.0 + k / 100.0;
        let slow: Song = song.iter().map(|&(s, d, m)| (s * f, d * f, m)).collect();
        let m = mix(&sing(&slow, len * f, 0.2), &band(len * f, None));
        let r = align_offset_ms(&notes, &m, SR);
        let (a, z) = (song[0].0 * (f - 1.0), song.last().map_or(0.0, |n| n.0) * (f - 1.0));
        let worst = (r.offset_ms as f64 - a).abs().max((r.offset_ms as f64 - z).abs());
        eprintln!("| {k} % | {:+} ms | {:.2} | {a:+.0} / {z:+.0} ms | {worst:.0} ms |", r.offset_ms, r.confidence);
    }
}
