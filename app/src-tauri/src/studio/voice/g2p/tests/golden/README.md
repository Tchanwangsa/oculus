# A2C golden files: the oracle for the Rust G2P port

Made by `tools/golden.py` (route code in `tools/a2c.py`); deterministic, rerunning gives
byte-identical files. `MANIFEST.json` has the versions and sha256 of every input.
Plan: `data/plans/studio-onnx-voice.md`, stage 1.

## Files

| file | what |
| --- | --- |
| `<input>.jsonl` (7) | the bake-off inputs, **library lexicon only** (`lexicon.library.json`) |
| `nolex/<input>.jsonl` (7) | same inputs, **empty lexicon**: exercises the guesser and the `-ied`/`-ise` rules on real prose |
| `special.jsonl` | 104 units, one per rule or edge case (`covers` names it), **library + subject lexicon** (subject over library) |
| `lexicon.library.json`, `lexicon.subject.json` | the two lexicon layers the runs used |
| `lexicon-errors.jsonl` | entries the port must reject: `bad-phoneme` (char not in Kokoro's vocab), `unknown-part` (respelling part not in the dictionary) |
| `guesser.jsonl` | the guesser alone: the 305 verify words (`verify`), every word or sub-token A2C guessed (`guessed`), with the encoder `input_ids` and the greedy phonemes |
| `SUMMARY.txt` | counts per input, every guessed word, and A2C against A2 and C |
| `dict/us_gold.json`, `dict/us_silver.json` | misaki's dictionary trimmed to the entries these files touch (`tools/trim_dict.py`, which reruns the oracle on the trimmed pair and diffs every file byte for byte), so the Rust golden test needs no 6 MB pack |

Row: `{id, text, phonemes, words: [{word, phonemes, source, guessed?}], covers?}`.
`phonemes` is the string that gets tokenised for Kokoro. `word` is a whitespace chunk
(punctuation split off); `source` is `lexicon | guess | rule | dict` (strongest wins);
`guessed` lists the sub-tokens the guesser answered.

## What the port must match

- **Whole-unit phonemes, byte for byte**, for every row of every file, except rows
  with a `guess` word, where only the guessed words may differ (see the ORT note).
- A2C equals A2 on all 461 bake-off units (the lexicon covers every unknown word), so
  the library-only files also pin the dictionary path, stems, heteronym table and the
  two extra rules (`multiplied`, `renormalise`, `factorisation`, `authorisation`).
- Lookup order: lexicon (any window of a word, longest first, so `Cauchy-Schwarz` finds
  `Cauchy`; `'s` by misaki's `_s`; exact key, then lower case) -> dictionary + stems +
  the two extra rules -> guesser, **one sub-token at a time**.
- Whitespace is kept as written (runs of spaces, newlines, trailing), except leading
  whitespace, which misaki's `preprocess` strips (`x098_whitespace`).
- Nothing is dropped. A word with letters and no phonemes is an error.

## Tagging without spaCy (part of the spec)

Punctuation tags (`. , : -LRB- -RRB- `` ''`); `PRP` for `I`; `DT` for a/an/the, `TO` for
to, `IN` for in; a capital `A` that is not first among the unit's words gets `NNP`;
every other word has no tag unless the heteronym table (`tools/heteronyms.py`) sets
one. `vs.` and `by` therefore never get misaki's `IN`/`ADV` readings.

## Quirks the oracle reproduces on purpose

- misaki's sub-token regex splits `O'Brien` into `O'B` + `rien` and `d'Alembert` into
  `d'A` + `lembert`; both parts are guessed.
- `x012_vs`: `vs.` is read `vˈiz` and the dot ends a sentence.
- A sentence-initial `A` is the article (`ɐ`), mid-sentence capital `A` is the letter.
- `Lead pipe` / `of lead` get the metal reading from the table, not the dictionary.

## ORT note for the guesser

Goldens were made with Python ORT 1.31; the app's `ort` crate links ORT 1.28. The
decode is greedy argmax, so a near-tie could flip on another build. The spike's TTS
audio was bit-identical across the two versions; if a guesser word differs, check
the logit margin before calling it a bug.
