import { expect, test } from "bun:test";
import { readVisit, writeVisit } from "@/hooks/data/overview/visit";

test("visits persist per subject only on explicit marking and cannot move backwards", () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const values = new Map<string, string>();
  Object.defineProperty(globalThis, "localStorage", { configurable: true, value: {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value); },
  } });
  try {
    expect(readVisit(1)).toBeNull();
    expect(values.size).toBe(0);
    writeVisit(1, 100);
    writeVisit(2, 200);
    writeVisit(1, 50);
    expect(readVisit(1)).toBe(100);
    expect(readVisit(2)).toBe(200);
    values.set("oculus:overview:last-visit:1", "bad");
    expect(readVisit(1)).toBeNull();
    writeVisit(1, NaN);
    expect(readVisit(1)).toBeNull();
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "localStorage", descriptor);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
});

test("every storage read and write tolerates disabled localStorage", () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  Object.defineProperty(globalThis, "localStorage", { configurable: true, get: () => { throw new Error("disabled"); } });
  try {
    expect(readVisit(1)).toBeNull();
    expect(() => writeVisit(1, 100)).not.toThrow();
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "localStorage", descriptor);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
});
