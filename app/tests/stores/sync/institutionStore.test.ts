import { afterAll, expect, mock, test } from "bun:test";

let answer: () => Promise<unknown> = async () => {
  throw "institution.json is unreadable";
};
mock.module("@tauri-apps/api/core", () => ({ invoke: () => answer() }));
afterAll(() => {
  // Module mocks outlive the file; leave later files an invoke that fails as it does without Tauri.
  answer = async () => {
    throw new Error("Tauri is not available");
  };
});

const { useInstitutionStore } = await import("@/stores/sync/institutionStore");

test("a failed read keeps its reason until a retry succeeds", async () => {
  const { load } = useInstitutionStore.getState();
  await expect(load()).rejects.toBe("institution.json is unreadable");
  expect(useInstitutionStore.getState().loadError).toBe("institution.json is unreadable");
  expect(useInstitutionStore.getState().institution).toBeUndefined();

  const profile = { canvas: "https://canvas.example.edu", host: "canvas.example.edu", okta: false, sso_host: null, locked: false };
  answer = async () => profile;
  await load();
  expect(useInstitutionStore.getState().loadError).toBeNull();
  expect(useInstitutionStore.getState().institution).toEqual(profile);
});
