import { beforeEach, describe, expect, test } from "bun:test";
import {
  claimPlayback,
  clearPlaybackOwner,
  markAdopted,
  markPlaying,
  mayAdopt,
  playbackOwner,
  releasePlayback,
  subscribePlaybackOwner,
} from "../src/lib/playbackOwner";

// Tab 1's main pane is pane 1 and its side item pane 11; tab 2's main is pane 2.
const can = (pane: number, tab: number, lecture: string) =>
  mayAdopt(playbackOwner(), pane, tab, lecture);

beforeEach(() => clearPlaybackOwner());

describe("which lecture player may adopt the video", () => {
  test("anyone on screen, while nobody owns it", () => {
    expect(can(1, 1, "a")).toBe(true);
    expect(can(11, 1, "b")).toBe(true);
  });

  test("a neighbour in the owner's tab does not take it on mount", () => {
    markAdopted(1, 1);
    expect(can(1, 1, "a")).toBe(true);
    expect(can(11, 1, "b")).toBe(false);
  });

  test("the owner's neighbour still waits while the tab is coming back", () => {
    markAdopted(11, 1);
    markPlaying(true);
    expect(can(1, 1, "a")).toBe(false);
    expect(can(11, 1, "b")).toBe(true);
  });

  test("a player in another tab takes it from an owner whose tab went back", () => {
    markAdopted(1, 1);
    expect(can(2, 2, "c")).toBe(true);
  });

  test("a claim gives it to that pane only once it shows the named lecture", () => {
    markAdopted(11, 1);
    claimPlayback(1, 1, "b");
    expect(can(11, 1, "b")).toBe(false);
    // The main pane is still on its old lecture until its route commits.
    expect(can(1, 1, "a")).toBe(false);
    expect(can(1, 1, "b")).toBe(true);
    markAdopted(1, 1);
    expect(playbackOwner().lectureId).toBeNull();
    expect(can(1, 1, "a")).toBe(true);
  });
});

describe("giving playback up", () => {
  test("a paused owner that unmounts frees it", () => {
    markAdopted(1, 1);
    releasePlayback(1);
    expect(playbackOwner().pane).toBeNull();
    expect(can(11, 1, "b")).toBe(true);
  });

  test("a playing owner keeps it, and a non-owner unmounting changes nothing", () => {
    markAdopted(1, 1);
    markPlaying(true);
    releasePlayback(1);
    releasePlayback(11);
    expect(playbackOwner().pane).toBe(1);
  });

  test("stopping clears the owner", () => {
    markAdopted(1, 1);
    markPlaying(true);
    clearPlaybackOwner();
    expect(playbackOwner()).toMatchObject({ pane: null, playing: false });
  });
});

describe("change notifications", () => {
  test("a new snapshot on change, none for a no-op", () => {
    let calls = 0;
    const off = subscribePlaybackOwner(() => calls++);
    markAdopted(1, 1);
    const before = playbackOwner();
    markAdopted(1, 1);
    markPlaying(false);
    expect(playbackOwner()).toBe(before);
    expect(calls).toBe(1);
    off();
  });
});
