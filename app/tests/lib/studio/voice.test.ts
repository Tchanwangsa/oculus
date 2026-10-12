import { expect, test } from "bun:test";

import { voiceProblem, type VoiceSettings } from "@/lib/studio/voice";

const settings = (patch: Partial<VoiceSettings>): VoiceSettings => ({
  engine: null,
  baseUrl: null,
  model: null,
  voice: null,
  speed: 0.9,
  ...patch,
});

test("a video needs an engine first", () => {
  expect(voiceProblem(settings({}))).toContain("engine");
  expect(voiceProblem(settings({ voice: "af_heart" }))).toContain("engine");
});

test("the Kokoro engine needs a voice and nothing else", () => {
  expect(voiceProblem(settings({ engine: "kokoro" }))).toContain("voice");
  expect(voiceProblem(settings({ engine: "kokoro", voice: "  " }))).toContain("voice");
  expect(voiceProblem(settings({ engine: "kokoro", voice: "af_heart" }))).toBeNull();
});

test("the server engine needs its URL, model and voice, in that order", () => {
  const server = { engine: "server" as const };
  expect(voiceProblem(settings({ ...server, voice: "alloy" }))).toContain("server");
  expect(voiceProblem(settings({ ...server, baseUrl: "http://localhost:8880", voice: "alloy" }))).toContain("model");
  expect(voiceProblem(settings({ ...server, baseUrl: "http://localhost:8880", model: "kokoro" }))).toContain("voice");
  expect(voiceProblem(settings({ ...server, baseUrl: " ", model: "kokoro", voice: "alloy" }))).toContain("server");
  expect(
    voiceProblem(settings({ ...server, baseUrl: "http://localhost:8880", model: "kokoro", voice: "alloy" })),
  ).toBeNull();
});
