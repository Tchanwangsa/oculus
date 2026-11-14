import { describe, expect, test } from "bun:test";
import { ARRIVAL_GRACE_MS, ask, dismiss, EMPTY_PROMPT, nextDeadline, step, type View } from "@/lib/browser/downloads/prompt";
import type { Download } from "@/lib/browser/downloads/types";
import { download } from "./fixtures";

const view = (front: number[], live: number[] = front): View => ({ front, live: new Set(live) });

describe("which download the Save dialog asks about", () => {
  test("a new download in the tab in front opens the dialog", () => {
    const p = step(EMPTY_PROMPT, [download(1, { tab: 1 })], view([1]), 0);
    expect(p.open).toBe(1);
    expect(p.arrivals).toEqual({});
  });

  test("one in a background tab only waits in the list, even once its tab comes to the front", () => {
    let p = step(EMPTY_PROMPT, [download(1, { tab: 2 })], view([1], [1, 2]), 0);
    expect(p.open).toBeNull();
    p = step(p, [download(1, { tab: 2 })], view([1], [1, 2]), ARRIVAL_GRACE_MS);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
    p = step(p, [download(1, { tab: 2 })], view([2], [1, 2]), ARRIVAL_GRACE_MS + 10);
    expect(p.open).toBeNull();
  });

  test("with no browser tab in front, nothing opens", () => {
    expect(step(EMPTY_PROMPT, [download(1, { tab: 2 })], view([], [2]), 0).open).toBeNull();
  });

  test("a popup's download credited to its opener opens once the popup closes", () => {
    const rows = [download(1, { tab: 1 })];
    // The popup (tab 5) is in front when its download arrives.
    let p = step(EMPTY_PROMPT, rows, view([5], [1, 5]), 0);
    expect(p.open).toBeNull();
    expect(nextDeadline(p)).toBe(ARRIVAL_GRACE_MS);
    // Rust closes it; the strip has not moved off it yet.
    p = step(p, rows, view([5], [1]), 50);
    expect(p.open).toBeNull();
    // The strip closes it and the opener is in front again.
    p = step(p, rows, view([1], [1]), 60);
    expect(p.open).toBe(1);
  });

  test("a closed popup that hands the front to another tab leaves the download waiting", () => {
    const rows = [download(1, { tab: 1 })];
    let p = step(EMPTY_PROMPT, rows, view([5], [1, 3, 5]), 0);
    p = step(p, rows, view([3], [1, 3]), 50);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
  });

  test("a download-only tab the student opened, in front, asks at once and stays open when it closes", () => {
    // A link from the app, a typed address or an attachment opened in tab 5:
    // no opener, so the download is tab 5's own.
    const rows = [download(1, { tab: 5 })];
    let p = step(EMPTY_PROMPT, rows, view([5], [1, 5]), 0);
    expect(p.open).toBe(1);
    // Rust closes tab 5; the strip has not moved off it yet.
    p = step(p, rows, view([5], [1]), 50);
    expect(p.open).toBe(1);
    // The strip brings its neighbour forward: the dialog is over that tab.
    p = step(p, rows, view([1], [1]), 60);
    expect(p.open).toBe(1);
    // The 1–2 ms finish changes nothing.
    p = step(p, [download(1, { tab: 5, state: "ready", received: 900 })], view([1], [1]), 70);
    expect(p.open).toBe(1);
    // A switch of tabs after that dismisses it, as for any download.
    p = step(p, rows, view([2], [1, 2]), 80);
    expect(p.open).toBeNull();
  });

  test("it stays open when the closed tab was the last one and no browser tab is left in front", () => {
    const rows = [download(1, { tab: 5 })];
    let p = step(EMPTY_PROMPT, rows, view([5], [5]), 0);
    p = step(p, rows, view([5], []), 50);
    // The strip's last tab goes back to the new-tab page.
    p = step(p, rows, view([], []), 60);
    expect(p.open).toBe(1);
    expect(p.arrivals).toEqual({});
  });

  test("a download-only tab opened in the background only waits in the list", () => {
    const rows = [download(1, { tab: 5 })];
    let p = step(EMPTY_PROMPT, rows, view([1], [1, 5]), 0);
    expect(p.open).toBeNull();
    // Once it has closed nothing can bring it forward: no grace to sit out.
    p = step(p, rows, view([1], [1]), 50);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
    expect(nextDeadline(p)).toBeNull();
  });

  test("a save that bounces for a download whose tab is gone asks again wherever the student is", () => {
    const saving = download(1, { tab: 5, state: "delivering", choice: { kind: "path", path: "/x/a.pdf" } });
    let p = step(EMPTY_PROMPT, [download(1, { tab: 5 })], view([5], [5]), 0);
    p = step(p, [saving], view([], []), 100);
    expect(p.open).toBeNull();
    p = step(p, [download(1, { tab: 5, state: "ready", error: "a.pdf already exists there" })], view([], []), 200);
    expect(p.open).toBe(1);
  });

  test("a tab closed under its dialog leaves the dialog open", () => {
    const rows = [download(1, { tab: 1 })];
    let p = step(EMPTY_PROMPT, rows, view([1], [1, 2]), 0);
    p = step(p, rows, view([2], [2]), 10);
    expect(p.open).toBe(1);
  });

  test("a popup whose opener has closed leaves its download waiting", () => {
    const rows = [download(1, { tab: 1 })];
    let p = step(EMPTY_PROMPT, rows, view([5], [1, 5]), 0);
    p = step(p, rows, view([3], [3]), 50);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
  });

  test("the mount's fetch learns rows without asking", () => {
    const p = step(EMPTY_PROMPT, [download(1, { tab: 1 }), download(2, { tab: 1, state: "ready" })], view([1]), 0, true);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
    // Replayed by the next event: still nothing.
    expect(step(p, [download(1, { tab: 1 }), download(2, { tab: 1, state: "ready" })], view([1]), 10).open).toBeNull();
  });

  test("a dismissed dialog stays shut as the download goes on, and reopens when asked", () => {
    let rows: Download[] = [download(1, { tab: 1 })];
    let p = step(EMPTY_PROMPT, rows, view([1]), 0);
    p = dismiss(p);
    rows = [download(1, { tab: 1, received: 500 })];
    p = step(p, rows, view([1]), 500);
    rows = [download(1, { tab: 1, state: "ready", received: 900 })];
    p = step(p, rows, view([1]), 1000);
    expect(p.open).toBeNull();
    p = ask(p, rows, 1);
    expect(p.open).toBe(1);
  });

  test("switching tabs dismisses it", () => {
    const rows = [download(1, { tab: 1 })];
    let p = step(EMPTY_PROMPT, rows, view([1], [1, 2]), 0);
    expect(p.open).toBe(1);
    p = step(p, rows, view([2], [1, 2]), 10);
    expect(p.open).toBeNull();
    // Coming back does not reopen it.
    p = step(p, rows, view([1], [1, 2]), 20);
    expect(p.open).toBeNull();
  });

  test("a choice made while receiving closes it; the row is no longer waiting", () => {
    let p = step(EMPTY_PROMPT, [download(1)], view([1]), 0);
    p = step(p, [download(1, { choice: { kind: "oculus", subject_code: "X" } })], view([1]), 10);
    expect(p.open).toBeNull();
  });

  test("a delivery that bounces back to ready with an error asks again", () => {
    let p = step(EMPTY_PROMPT, [download(1)], view([1]), 0);
    p = step(p, [download(1, { state: "delivering", choice: { kind: "path", path: "/x/a.pdf" } })], view([1]), 10);
    expect(p.open).toBeNull();
    p = step(p, [download(1, { state: "ready", error: "a file is already there" })], view([1]), 20);
    expect(p.open).toBe(1);
  });

  test("a bounced delivery in a background tab only marks the list", () => {
    let p = step(EMPTY_PROMPT, [download(1, { tab: 2, state: "delivering" })], view([1], [1, 2]), 0, true);
    p = step(p, [download(1, { tab: 2, state: "ready", error: "no room" })], view([1], [1, 2]), ARRIVAL_GRACE_MS + 10);
    p = step(p, [download(1, { tab: 2, state: "ready", error: "no room" })], view([1], [1, 2]), 2 * ARRIVAL_GRACE_MS + 20);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
  });

  test("a second download while the dialog asks about the first waits in the list", () => {
    let p = step(EMPTY_PROMPT, [download(1)], view([1]), 0);
    p = step(p, [download(1), download(2)], view([1]), 10);
    expect(p.open).toBe(1);
    expect(p.arrivals).toEqual({});
  });

  test("asking about a row that is not waiting does nothing", () => {
    const rows = [download(1, { state: "failed", error: "x" })];
    const p = step(EMPTY_PROMPT, rows, view([1]), 0, true);
    expect(ask(p, rows, 1)).toBe(p);
  });

  test("a refused playlist, or a download that ends before it is seen, never asks", () => {
    const rows = [
      download(1, { state: "refused", error: "a playlist" }),
      download(2, { state: "failed", error: "the download did not finish" }),
    ];
    const p = step(EMPTY_PROMPT, rows, view([1]), 0);
    expect(p.open).toBeNull();
    expect(p.arrivals).toEqual({});
  });

  test("a download already finished when it is first seen asks like any other", () => {
    const p = step(EMPTY_PROMPT, [download(1, { state: "ready", received: 900 })], view([1]), 0);
    expect(p.open).toBe(1);
  });

  test("the dialog closes when the download it asks about fails", () => {
    let p = step(EMPTY_PROMPT, [download(1)], view([1]), 0);
    p = step(p, [download(1, { state: "failed", error: "the download did not finish" })], view([1]), 10);
    expect(p.open).toBeNull();
  });
});
