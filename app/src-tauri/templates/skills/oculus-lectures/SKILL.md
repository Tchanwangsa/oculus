---
name: oculus-lectures
description: Reads what was said in a recorded lecture in an Oculus course library, through its merged transcript and slide outline in the recording's folder under Course/Recordings, and quotes it with timestamps. Use for "what did the lecture say about X", "find where they covered Y", "quote the recording", or any question that has to be answered from a lecture rather than from a slide deck or a Canvas page.
---

# Lectures

A lecture is a **folder of files**, not a row to query. `oculus grep` does not
reach it: that command scans Canvas markdown and PDF page text, and a
recording is neither. So the shape of this work is always the same — find
the recording's folder, open the merged document, read the span.

## 1. Find the folder

```bash
oculus list -l MULT20015
```

One row per recording: its id, date, length and title, and under it the
recording's folder from the library root:

```
  2d2a4a23 2026-09-08    52m  video transcript       MULT20015_2026_SM2 MO L105
           courses/MULT20015_2026_SM2/Course/Recordings/2026-09-08-mo-l105/
```

The folder is `<YYYY-MM-DD>-<slug>`, the date the lecture was given and its
title, so the date is what tells two recordings of the same timetable slot
apart. A recording with no folder line has nothing on disk yet. `--json`
gives the same as `folder`. An `oculus lecture` command takes a **unique
prefix** of the id (`2d2a4a23` is enough); a path takes the folder.

If the subject is not known yet, `oculus list -l` with no code lists them all.

## 2. Read `outline.md`, not the `.vtt`

From the chat's working directory the folder is `../` and that path:

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

## 3. Find the span before reading

A lecture is around **2500 cues**. Read the relevant span rather than
loading the whole recording into context.

Find the span, then read it:

```bash
grep -n "superposition" ../courses/MULT20015_2026_SM2/Course/Recordings/2026-09-08-mo-l105/outline.md
```

```
412:    725  00:12:05  An equal superposition.
```

`grep -n` puts the file's **line number** first (`412`), before the outline's
own first column, which is **seconds** into the recording (`725`). `sed -n`
takes line numbers, so read around `412`, not `725`:

```bash
sed -n '390,450p' ../courses/MULT20015_2026_SM2/Course/Recordings/2026-09-08-mo-l105/outline.md
```

Two calls, each a single command. Widen with a second `sed -n` rather than
re-reading from the top.

Where the chat sandbox restricts shell reads, use the native `grep` and
`read` tools with an offset instead.

## 4. Quote it by time

Cite the clock — "at 12:03 they say…" — never the bare second in the first
column, which is for matching a chapter's `start` to a detected boundary.

## What not to run to answer a question

`oculus lecture chapters` and `oculus lecture end` **generate** things — they
drive a model, cost quota (chapters take minutes to an hour), and leave an
existing result alone unless given `--force`. They are not lookups. For a
generation request, use the LectureChapters or LectureEnd workflow in
`oculus-plan`. Never generate them to find out what was said.

`oculus lecture candidates` is free and writes nothing, but it returns
boundary moments with no words attached — `outline.md` already has those
markers in place, so it is rarely the better door.
