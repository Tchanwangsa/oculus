import { afterEach, describe, expect, test } from "bun:test";
import { recordVoice } from "../../../src/lib/dictation/recording";

const original = new Map<string, PropertyDescriptor | undefined>();
function replace(name: string, value: unknown) {
  if (!original.has(name)) original.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { value, configurable: true, writable: true });
}
afterEach(() => {
  for (const [key, descriptor] of original) {
    if (descriptor) Object.defineProperty(globalThis, key, descriptor);
    else Reflect.deleteProperty(globalThis, key);
  }
  original.clear();
});

function devices() {
  let stops = 0;
  let closes = 0;
  const stream = { getTracks: () => [{ stop: () => { stops++; } }] };
  replace("navigator", { mediaDevices: { getUserMedia: async () => stream } });
  replace("AudioContext", class {
    createAnalyser() { return { fftSize: 0 }; }
    createMediaStreamSource() { return { connect() {} }; }
    async close() { closes++; }
  });
  replace("MediaRecorder", class {
    static isTypeSupported(type: string) { return type === "audio/mp4"; }
    state = "inactive";
    mimeType = "audio/mp4";
    ondataavailable?: (e: { data: Blob }) => void;
    onstop?: () => void;
    start() { this.state = "recording"; }
    stop() {
      this.state = "inactive";
      this.ondataavailable?.({ data: new Blob(["recorded audio"]) });
      this.onstop?.();
    }
  });
  return { stops: () => stops, closes: () => closes };
}

describe("microphone ownership", () => {
  test("finishing returns the clip, stops capture and permits the next composer", async () => {
    const device = devices();
    const first = await recordVoice();
    await expect(recordVoice()).rejects.toThrow("Another composer");
    const clip = await first.finish();
    expect(await clip.text()).toBe("recorded audio");
    expect(clip.type).toBe("audio/mp4");
    expect(device.stops()).toBe(1);
    expect(device.closes()).toBe(1);
    const second = await recordVoice();
    second.cancel();
  });

  test("cancel stops capture and releases the microphone", async () => {
    const device = devices();
    const first = await recordVoice();
    first.cancel();
    expect(device.stops()).toBeGreaterThan(0);
    const second = await recordVoice();
    second.cancel();
  });

  test("a refused permission releases ownership and explains how to recover", async () => {
    devices();
    replace("navigator", { mediaDevices: { getUserMedia: async () => { throw new DOMException("blocked", "NotAllowedError"); } } });
    await expect(recordVoice()).rejects.toThrow("system privacy settings");
    devices();
    const next = await recordVoice();
    next.cancel();
  });

  test("recorder construction failure stops an already opened stream", async () => {
    const device = devices();
    replace("MediaRecorder", class {
      static isTypeSupported() { return true; }
      constructor() { throw new Error("codec unavailable"); }
    });
    await expect(recordVoice()).rejects.toThrow("codec unavailable");
    expect(device.stops()).toBe(1);
    expect(device.closes()).toBe(1);
    devices();
    const next = await recordVoice();
    next.cancel();
  });
});
