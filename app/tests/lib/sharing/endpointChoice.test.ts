import { expect, test } from "bun:test";
import { pickEndpoint } from "@/lib/sharing/view";
import type { ShareEndpoint } from "@/lib/sharing/types";

const endpoint = (id: string, family: ShareEndpoint["family"], clients = 0): ShareEndpoint => ({
  id, address: id, interface: id, family, label: id, source: "interface", generation: 1,
  url: `http://${id}:43821/c/#key`, connectedClients: clients, reachable: clients > 0,
});

test("the default share link follows a connected device's address, while a manual pick wins", () => {
  const wifi = endpoint("wifi", "ipv4"), vpn = endpoint("vpn", "ipv4", 1);
  expect(pickEndpoint([wifi, vpn], null)).toBe(vpn);
  expect(pickEndpoint([wifi, vpn], wifi.id)).toBe(wifi);
  expect(pickEndpoint([wifi, vpn], "old-network")).toBe(vpn);
});

test("connected IPv6 is preferred to unused IPv4, and connected IPv4 wins when both work", () => {
  const wifi = endpoint("wifi", "ipv4"), v6 = endpoint("v6", "ipv6", 1), vpn = endpoint("vpn", "ipv4", 1);
  expect(pickEndpoint([wifi, v6], null)).toBe(v6);
  expect(pickEndpoint([wifi, v6, vpn], null)).toBe(vpn);
  expect(pickEndpoint([{ ...vpn, url: null }, wifi], null)).toBe(wifi);
});
