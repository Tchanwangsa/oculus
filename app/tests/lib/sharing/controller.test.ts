import { describe, expect, test } from "bun:test";
import type { CanvasAuthorityPhase } from "@/lib/canvas/authority";
import { shareController, type ShareSessionPort } from "@/lib/sharing/controller";
import type { CompanionState, ShareEndpoints, ShareInfo, SharingApi } from "@/lib/sharing/types";
import { pickEndpoint, shareStatusLine } from "@/lib/sharing/view";

const FILE = 7;
const endpoint = (id: string, address: string, url: string | null, family: "ipv4" | "ipv6" = "ipv4") => ({
  id, url, address, family, interface: id.split("/")[0], label: `Wi‑Fi/Ethernet ${id.split("/")[0]}`, source: "interface", generation: 1, connectedClients: 0, reachable: false,
});
const info = (patch: Partial<ShareInfo> = {}): ShareInfo => ({
  fileId: FILE, shareId: "a".repeat(32), epoch: "e1", port: 43821, generation: 1, clients: 0, phase: "live",
  endpoints: [endpoint("en0/192.168.1.23", "192.168.1.23", "http://192.168.1.23:43821/c/#key")], ...patch,
});
const state = (patch: Partial<CompanionState> = {}): CompanionState => ({ running: true, port: 43821, ipv6: false, generation: 1, endpoints: [], shares: [], ...patch });
const summary = (patch: Partial<ShareInfo> = {}) => { const { fileId, shareId, epoch, clients, phase } = info(patch); return { fileId, shareId, epoch, clients, phase }; };
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

function fakeSession(phase: CanvasAuthorityPhase = "off") {
  const calls: string[] = [];
  const session = {
    file: { id: FILE },
    authority: phase,
    attachFails: null as unknown,
    detachFails: null as unknown,
    calls,
    async attachAuthority() { calls.push("attach"); await flush(); if (session.attachFails) throw session.attachFails; session.authority = "live"; },
    async detachAuthority() { calls.push("detach"); await flush(); if (session.detachFails) throw session.detachFails; session.authority = "off"; },
  };
  return session satisfies ShareSessionPort & Record<string, unknown>;
}

function fakeApi() {
  const calls: string[] = [];
  const live = new Map<number, ShareInfo>();
  const api = {
    calls,
    live,
    startFails: null as unknown,
    addresses: { port: 43821, generation: 1, endpoints: [endpoint("en0/192.168.1.23", "192.168.1.23", "http://192.168.1.23:43821/c/")] } as ShareEndpoints,
    async start(fileId: number) { calls.push(`start ${fileId}`); await flush(); if (api.startFails) throw api.startFails; const share = info(); live.set(fileId, share); return share; },
    async stop(fileId: number) { calls.push(`stop ${fileId}`); await flush(); live.delete(fileId); },
    async stopAll() { calls.push("stopAll"); await flush(); live.clear(); },
    async shares() { calls.push("shares"); await flush(); return [...live.values()]; },
    async endpoints() { calls.push("endpoints"); await flush(); return api.addresses; },
  };
  return api satisfies SharingApi & Record<string, unknown>;
}

describe("share controller", () => {
  test("start attaches the authority before asking for a share", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    expect(await controller.start()).toBeNull();
    expect(session.calls).toEqual(["attach"]);
    expect(api.calls).toEqual([`start ${FILE}`]);
    expect(controller.view().status).toBe("sharing");
    expect(controller.view().share?.endpoints[0].url).toContain("#key");
  });

  test("a failed start detaches again and shows the native error", async () => {
    const session = fakeSession(), api = fakeApi();
    api.startFails = { code: "server", message: "No port between 43821 and 43840 could be bound." };
    const controller = shareController(session, api);
    const failure = await controller.start();
    expect(failure).toEqual({ code: "server", message: "No port between 43821 and 43840 could be bound." });
    expect(session.calls).toEqual(["attach", "detach"]);
    expect(session.authority).toBe("off");
    expect(controller.view()).toMatchObject({ status: "off", share: null, error: failure });
  });

  test("a failed attach never asks for a share", async () => {
    const session = fakeSession(), api = fakeApi();
    session.attachFails = new Error("Choose which canvas version to keep before sharing.");
    const controller = shareController(session, api);
    expect((await controller.start())?.message).toContain("Choose which");
    expect(api.calls).toEqual([]);
    expect(session.calls).toEqual(["attach"]);
    expect(controller.view().status).toBe("off");
  });

  test("a failed start leaves an authority it did not attach", async () => {
    const session = fakeSession("live"), api = fakeApi();
    api.startFails = { code: "random", message: "No secret could be made." };
    const controller = shareController(session, api);
    await controller.start();
    expect(session.calls).toEqual(["attach"]);
  });

  test("stop ends the share, then hands the file back", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.calls.length = 0; session.calls.length = 0;
    expect(await controller.stop()).toBeNull();
    expect(api.calls).toEqual([`stop ${FILE}`]);
    expect(session.calls).toEqual(["detach"]);
    expect(controller.view()).toMatchObject({ status: "off", share: null, error: null });
  });

  test("a failed stop keeps sharing and says why", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.stop = async () => { throw { code: "server", message: "busy" }; };
    session.calls.length = 0;
    expect(await controller.stop()).toEqual({ code: "server", message: "busy" });
    expect(session.calls).toEqual([]);
    expect(controller.view().status).toBe("sharing");
  });

  test("stop all stops every share and detaches this one", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    await controller.stopAll();
    expect(api.calls.at(-1)).toBe("stopAll");
    expect(session.calls.at(-1)).toBe("detach");
    expect(controller.view().status).toBe("off");
  });

  test("double clicks start and stop once", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await Promise.all([controller.start(), controller.start(), controller.start()]);
    await controller.start();
    expect(session.calls).toEqual(["attach"]);
    expect(api.calls).toEqual([`start ${FILE}`]);
    await Promise.all([controller.stop(), controller.stop()]);
    expect(api.calls.filter((c) => c.startsWith("stop"))).toHaveLength(1);
    expect(session.calls.filter((c) => c === "detach")).toHaveLength(1);
  });

  test("a share the native side ended detaches the session", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.live.clear();
    controller.observe(state({ shares: [] }));
    await flush(); await flush(); await flush();
    expect(session.calls).toEqual(["attach", "detach"]);
    expect(controller.view()).toMatchObject({ status: "ended", share: null });
  });

  test("an event that predates the share is checked, not trusted", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    controller.observe(state({ shares: [] }));
    await flush(); await flush(); await flush();
    expect(session.calls).toEqual(["attach"]);
    expect(controller.view().status).toBe("sharing");
  });

  test("events carry client counts and pauses; no address means no client", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    expect(shareStatusLine(controller.view(), session.authority).text).toBe("Sharing · no device connected");
    controller.observe(state({ shares: [summary({ clients: 2 })] }));
    expect(shareStatusLine(controller.view(), session.authority).text).toBe("Sharing · 2 devices connected");
    controller.observe(state({ shares: [summary({ clients: 1, phase: "paused" })] }));
    expect(shareStatusLine(controller.view(), session.authority).tone).toBe("warning");
  });

  test("refresh rereads links and addresses without starting or stopping anything", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.calls.length = 0; session.calls.length = 0;
    api.live.set(FILE, info({ generation: 2, endpoints: [endpoint("en0/10.0.0.9", "10.0.0.9", "http://10.0.0.9:43821/c/#key")] }));
    await Promise.all([controller.refresh(), controller.refresh()]);
    expect(api.calls.sort()).toEqual(["endpoints", "shares"]);
    expect(session.calls).toEqual([]);
    expect(controller.view().share?.endpoints[0].address).toBe("10.0.0.9");
    expect(controller.view().addresses?.port).toBe(43821);
  });

  test("an event's addresses replace the view's without a command", () => {
    const controller = shareController(fakeSession(), fakeApi());
    const bare = endpoint("en1/10.1.1.4", "10.1.1.4", "http://10.1.1.4:43821/c/");
    controller.observe(state({ generation: 4, endpoints: [bare] }));
    expect(controller.view().addresses).toEqual({ port: 43821, generation: 4, endpoints: [bare] });
  });

  test("a new client count rereads the links' per-address counts", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.calls.length = 0;
    controller.observe(state({ shares: [summary({ clients: 1 })] }));
    await flush(); await flush();
    expect(api.calls.sort()).toEqual(["endpoints", "shares"]);
    expect(session.calls).toEqual(["attach"]);
  });

  test("a new address generation in an event rereads the links", async () => {
    const session = fakeSession(), api = fakeApi();
    const controller = shareController(session, api);
    await controller.start();
    api.calls.length = 0;
    controller.observe(state({ generation: 2, shares: [summary()] }));
    await flush(); await flush();
    expect(api.calls.sort()).toEqual(["endpoints", "shares"]);
  });

  test("opening on a canvas shared earlier finds its share", async () => {
    const session = fakeSession("live"), api = fakeApi();
    api.live.set(FILE, info({ clients: 1 }));
    const controller = shareController(session, api);
    expect(controller.view().status).toBe("unknown");
    await controller.refresh();
    expect(controller.view()).toMatchObject({ status: "sharing", share: { clients: 1 } });
    expect(session.calls).toEqual([]);
  });

  test("refresh rejoins a native share that survived the desktop webview", async () => {
    const session = fakeSession("off"), api = fakeApi();
    api.live.set(FILE, info({ clients: 1 }));
    const controller = shareController(session, api);
    await controller.refresh();
    expect(session.calls).toEqual(["attach"]);
    expect(session.authority).toBe("live");
    expect(controller.view()).toMatchObject({ status: "sharing", share: { clients: 1 } });
    expect(api.calls.sort()).toEqual(["endpoints", "shares"]);
  });

  test("a refresh that fails is shown apart from the share", async () => {
    const session = fakeSession(), api = fakeApi();
    api.endpoints = async () => { throw new Error("unreachable"); };
    const controller = shareController(session, api);
    await controller.refresh();
    expect(controller.view()).toMatchObject({ status: "off", refreshError: "unreachable", error: null });
  });
});

describe("endpoint choice", () => {
  const a = endpoint("en0/192.168.1.23", "192.168.1.23", "http://192.168.1.23:43821/c/#k");
  const b = endpoint("utun9/100.64.0.2", "100.64.0.2", "http://100.64.0.2:43821/c/#k");
  const v6 = endpoint("en0/fd00::5", "fd00::5", "http://[fd00::5]:43821/c/#k", "ipv6");

  test("keeps the user's pick while it is listed", () => {
    expect(pickEndpoint([a, b], b.id)).toBe(b);
    expect(pickEndpoint([a], b.id)).toBe(a);
  });

  test("defaults to the first IPv4 address with a link", () => {
    expect(pickEndpoint([v6, a, b], null)).toBe(a);
    expect(pickEndpoint([v6], null)).toBe(v6);
    expect(pickEndpoint([{ ...a, url: null }], null)).toBeNull();
  });
});
