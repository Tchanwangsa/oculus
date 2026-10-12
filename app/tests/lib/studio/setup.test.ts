import { expect, test } from "bun:test";

import type { AlignerStatus } from "@/lib/studio/aligner";
import type { KokoroVoice } from "@/lib/studio/kokoroVoices";
import { setupProblems } from "@/lib/studio/setup";
import type { VoiceSettings } from "@/lib/studio/voice";
import type { VoicePacksStatus } from "@/lib/studio/voicePacks";

const voice = (patch: Partial<VoiceSettings>): VoiceSettings => ({
  engine: "kokoro",
  baseUrl: null,
  model: null,
  voice: "af_heart",
  speed: 0.9,
  ...patch,
});
const packs = (status: VoicePacksStatus["status"]): VoicePacksStatus => ({
  status,
  bytes: 101_000_000,
  packs: [],
});
const aligner = (status: AlignerStatus["status"]): AlignerStatus => ({ status, bytes: 378_000_000 });
const heart = (status: KokoroVoice["status"]): KokoroVoice => ({
  id: "af_heart",
  name: "Heart",
  accent: "American",
  gender: "female",
  grade: "A",
  bytes: 522_240,
  status,
});

const ids = (v: VoiceSettings, p: VoicePacksStatus, a: AlignerStatus, voices = [heart("present")]) =>
  setupProblems(v, p, a, voices).map((x) => x.id);

test("a configured voice with its packs, its file and the aligner is ready", () => {
  expect(ids(voice({}), packs("present"), aligner("present"))).toEqual([]);
});

test("the aligner is needed whatever the engine", () => {
  expect(ids(voice({}), packs("present"), aligner("missing"))).toEqual(["timing"]);
  expect(ids(voice({}), packs("present"), aligner("downloading"))).toEqual(["timing"]);
  const server = voice({ engine: "server", baseUrl: "http://localhost:8880", model: "kokoro", voice: "alloy" });
  expect(ids(server, packs("missing"), aligner("missing"), [])).toEqual(["timing"]);
});

test("Kokoro's model and pronunciation pack are needed only for the Kokoro engine", () => {
  expect(ids(voice({}), packs("missing"), aligner("present"))).toEqual(["packs"]);
  expect(ids(voice({}), packs("downloading"), aligner("present"))).toEqual(["packs"]);
});

test("the chosen voice must be downloaded, and the model alone does not make it so", () => {
  expect(ids(voice({}), packs("present"), aligner("present"), [heart("missing")])).toEqual(["voice"]);
  expect(ids(voice({}), packs("present"), aligner("present"), [heart("downloading")])).toEqual(["voice"]);
  expect(ids(voice({}), packs("missing"), aligner("present"), [heart("missing")])).toEqual(["packs", "voice"]);
  const other = voice({ voice: "am_puck" });
  expect(ids(other, packs("present"), aligner("present"))).toEqual(["voice"]);
  const message = setupProblems(voice({}), packs("present"), aligner("present"), [heart("missing")])[0].message;
  expect(message).toContain("Heart");
});

test("a voice nobody chose is the narration problem alone", () => {
  expect(ids(voice({ voice: null }), packs("present"), aligner("present"))).toEqual(["narration"]);
});

test("each problem names the page that fixes it, narration first", () => {
  const all = setupProblems(voice({ engine: null, voice: null }), packs("missing"), aligner("missing"), []);
  expect(all.map((p) => p.id)).toEqual(["narration", "timing"]);
  expect(all.map((p) => p.href)).toEqual(["/settings/narration", "/settings/word-timing"]);
  const sizes = setupProblems(voice({}), packs("missing"), aligner("missing"), [heart("present")]);
  expect(sizes[0].message).toContain("101 MB");
  expect(sizes[1].message).toContain("378 MB");
});
