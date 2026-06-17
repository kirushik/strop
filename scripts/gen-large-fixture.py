#!/usr/bin/env python3
"""Generate a large prose fixture for performance measurement.

  python3 scripts/gen-large-fixture.py [out_path] [target_words]

Default: ~52k words to /tmp/strop-large.md. Deterministic (seeded). Sprinkles
headings and inline **bold**/*italic* so the SpanSet/BlockMap hot paths are
exercised, not just plain paragraphs.
"""
import random
import sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/strop-large.md"
TARGET = int(sys.argv[2]) if len(sys.argv) > 2 else 52000

random.seed(42)
words = (
    "the quick brown fox jumps over a lazy dog while morning light spills across "
    "the valley and the river carries its slow argument toward the sea where gulls "
    "trade rumours about the weather and the tide forgets its own name again"
).split()
words += "memory silence threshold lantern gravel orchard tremor cadence ledger filament harbor reckon thicket meridian solvent".split()


def sentence():
    n = random.randint(8, 22)
    s = " ".join(random.choice(words) for _ in range(n))
    return s[0].upper() + s[1:] + random.choice([".", ".", ".", "?", "!"])


def para():
    p = " ".join(sentence() for _ in range(random.randint(2, 6)))
    if random.random() < 0.4:
        ws = p.split()
        i = random.randrange(0, max(1, len(ws) - 2))
        ws[i] = "**" + ws[i] + "**"
        if len(ws) > i + 3:
            ws[i + 3] = "*" + ws[i + 3] + "*"
        p = " ".join(ws)
    return p


out, total, ch = [], 0, 0
while total < TARGET:
    if random.random() < 0.05:
        ch += 1
        out += [f"## Chapter section {ch}", ""]
        continue
    p = para()
    total += len(p.split())
    out += [p, ""]

text = "\n".join(out)
open(OUT, "w").write(text)
print(f"wrote {OUT}: words~{total} blocks~{len([l for l in out if l])} bytes={len(text)}")
