---
name: oculus-lectures
description: Read what was said in a recorded lecture in an Oculus course library, and quote it with timestamps. Use for "what did the lecture say about X", "find where they covered Y", "quote the recording", or any question that has to be answered from a lecture rather than from a slide deck or a Canvas page.
---

# Lectures

A lecture is a **folder of files**, not a row to query. `oculus grep` does not
reach it: that command scans Canvas markdown and PDF page text, and a
recording is neither. So the shape of this work is always the same — resolve
the id, open the merged document, read the span.

## 1. Resolve the id

```bash
oculus list -l MULT20015
```

One row per recording: a UUID, the title and the date. Every command and path
below takes a **unique prefix** of that UUID — `2d2a4a23` is enough, the full
36 characters are never needed.

If the subject is not known yet, `oculus list -l` with no code lists them all.

## 2. Read `outline.md`, not the `.vtt`

The lecture folder is `../lectures/<uuid>/` from this working directory:

```
outline.md      the transcript and the slide changes merged, in play order
transcript.vtt  the raw cues
source1.mp4     captured streams
source2.mp4
frames/         one JPEG per detected boundary, if they were ever written
```

**`outline.md` is the document to open.** Every line is
`second  timestamp  text`, with slide changes marked inline:

```
      1  00:00:01  Good morning.
    723  00:12:03  --- slide change · score 42.1 · pause ---
    725  00:12:05  An equal superposition.
```

That is what makes it worth more than the `.vtt`: "the picture changed here"
and "the subject turned here" land on adjacent lines, so a topic boundary is
visible instead of being reconstructed from two files. Only a lecture that has
been chaptered has one — if it is missing, fall back to `transcript.vtt`.

## 3. Never read the whole file, and never loop over it

A lecture is around **2500 cues**. Do not `cat` it, and do not write a `for`,
a `while`, an `awk` or a `$(…)` over the folder — a compound command is
refused outright and the turn continues without the answer.

Find the span, then read it:

```bash
grep -n "superposition" ../lectures/2d2a4a23/outline.md
```

```bash
sed -n '700,760p' ../lectures/2d2a4a23/outline.md
```

Two calls, each a single command. Widen with a second `sed -n` rather than
re-reading from the top.

**On opencode, `bash` is allow-listed to `oculus …` and `ls …` only** — `sed`
and `grep` are refused there. Use the `grep` and `read` tools instead, with an
offset; they are permitted and do the same job.

## 4. Quote it by time

Cite the clock — "at 12:03 they say…". The bare second in the first column is
for machinery, not for a reader: it exists because a chapter's `start` has to
match a detected boundary *exactly*, and a rounded second is not in the
allowed set.

## What not to run to answer a question

`oculus lecture chapters` and `oculus lecture reading` **generate** things —
they drive a model, cost quota, take minutes to an hour, and leave an existing
result alone unless given `--force`. They are not lookups. Reach for them only
when asked to produce chapters or a reading copy, never to find out what was
said.

`oculus lecture candidates` is free and writes nothing, but it returns
boundary moments with no words attached — `outline.md` already has those
markers in place, so it is rarely the better door.
