"""Writes the self-authored UltraStar fixtures (Law 19) for T-013-04. Usage: python -I gen_ultrastar.py <out_dir>"""
import os
import sys

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

HEAD = "#TITLE:Canção de Teste\n#ARTIST:Master Of Notes\n#MP3:teste.mp3\n"

files = {
    # SPEC-013 criterion + golden note, F/R/G skipped, line break, "~" continuation, end.
    "basic.txt": HEAD + "#BPM:300\n#GAP:1000\n"
    ": 0 4 0 Lá\n* 4 4 2 ri\nF 8 2 4 hey\nR 10 2 0 yo\nG 12 2 0 yo\n- 16\n: 18 4 4 la\n: 22 2 5 ~\nE\n",
    # Decimal comma in BPM (common in real files): 150,5 → beat = 60000 / (150.5 × 4) ms.
    "bpm-comma.txt": HEAD + "#BPM:150,5\n#GAP:0\n: 0 4 0 a\n: 8 4 2 b\nE\n",
    # #RELATIVE:yes — beats restart after each "- x y" (y = shift added to the base).
    "relative.txt": HEAD + "#BPM:300\n#GAP:500\n#RELATIVE:yes\n"
    ": 0 4 0 um\n: 4 4 2 dois\n- 10 12\n: 0 4 4 três\n: 6 2 5 quatro\nE\n",
    # Duet: only P1 is imported (P2 has higher, overlapping notes that must not appear).
    "duet.txt": HEAD + "#BPM:300\n#GAP:0\nP1\n: 0 4 0 eu\n: 4 4 2 vou\nP2\n: 0 4 12 você\n: 4 4 14 vem\nE\n",
    # Text after E is ignored.
    "after-end.txt": HEAD + "#BPM:300\n#GAP:0\n: 0 4 0 fim\nE\n: 8 4 2 lixo\n",
}
for name, text in files.items():
    with open(os.path.join(out, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)

# CP1252 / Latin-1 without #ENCODING (old files): "Ação" and "pé" must come out as UTF-8 lyrics.
cp = (HEAD.replace("Canção", "Canção") + "#BPM:300\n#GAP:0\n: 0 4 0 Ação\n: 4 4 2 pé\nE\n").encode("cp1252")
open(os.path.join(out, "cp1252.txt"), "wb").write(cp)
# UTF-8 with BOM: the BOM must not break "#TITLE" (first header).
open(os.path.join(out, "bom.txt"), "wb").write("﻿".encode() + (HEAD + "#BPM:300\n#GAP:0\n: 0 4 0 oi\nE\n").encode())
# CRLF line endings (Windows editors).
open(os.path.join(out, "crlf.txt"), "wb").write((HEAD + "#BPM:300\n#GAP:0\n: 0 4 0 oi\n: 4 4 2 tchau\nE\n").replace("\n", "\r\n").encode())

invalid = {
    "invalid-bpm-zero.txt": "#BPM:0\n#GAP:0\n: 0 4 0 a\nE\n",
    "invalid-bpm-negative.txt": "#BPM:-120\n#GAP:0\n: 0 4 0 a\nE\n",
    "invalid-bpm-missing.txt": "#TITLE:Sem BPM\n#GAP:0\n: 0 4 0 a\nE\n",
    "invalid-bpm-text.txt": "#BPM:rápido\n#GAP:0\n: 0 4 0 a\nE\n",
    "invalid-gap-negative.txt": "#BPM:300\n#GAP:-500\n: 0 4 0 a\n: 20 4 2 b\nE\n",
    "invalid-truncated-line.txt": "#BPM:300\n#GAP:0\n: 0 4 0 a\n: 4 4\n: 8 4 2 c\nE\n",
    "invalid-overlap.txt": "#BPM:300\n#GAP:0\n: 0 8 0 a\n: 4 4 2 b\nE\n",
    "invalid-only-freestyle.txt": "#BPM:300\n#GAP:0\nF 0 4 0 a\nR 4 4 0 b\nG 8 4 0 c\nE\n",
    "invalid-pitch-high.txt": "#BPM:300\n#GAP:0\n: 0 4 0 a\n: 4 4 49 b\nE\n",
    "invalid-pitch-low.txt": "#BPM:300\n#GAP:0\n: 0 4 -37 a\nE\n",
    "invalid-negative-length.txt": "#BPM:300\n#GAP:0\n: 0 -4 0 a\nE\n",
    "invalid-empty.txt": "",
}
for name, text in invalid.items():
    with open(os.path.join(out, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
print("\n".join(sorted(os.listdir(out))))
