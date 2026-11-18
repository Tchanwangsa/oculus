import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ShareDialogBody } from "@/components/canvas/share/ShareDialogBody";
import { SharingSettingsView } from "@/components/settings/sharing/SharingSettingsView";
import type { ShareView } from "@/lib/sharing/controller";
import type { ShareEndpoint, ShareInfo } from "@/lib/sharing/types";

const KEY = `${"0123456789abcdef".repeat(2)}.${"A".repeat(43)}`;
const endpoint = (iface: string, address: string, url: string | null = `http://${address}:43821/c/#${KEY}`): ShareEndpoint => ({
  id: `${iface}/${address}`, url, address, family: "ipv4", interface: iface, label: `Wi‑Fi/Ethernet ${iface}`, source: "interface", generation: 1, connectedClients: 0, reachable: false,
});
const share = (patch: Partial<ShareInfo> = {}): ShareInfo => ({
  fileId: 1, shareId: "f".repeat(32), epoch: "e", port: 43821, generation: 1, clients: 0, phase: "live",
  endpoints: [endpoint("en0", "192.168.1.23"), endpoint("utun9", "100.64.0.2")], ...patch,
});
const view = (patch: Partial<ShareView> = {}): ShareView => ({
  status: "off", share: null, addresses: { port: null, generation: 1, endpoints: [endpoint("en0", "192.168.1.23", null)] },
  preferred: null, error: null, refreshError: null, ...patch,
});
const noop = () => {};
const render = (v: ShareView, confirmingStop = false, authority: "off" | "live" | "paused" = "off") => renderToStaticMarkup(
  <ShareDialogBody view={v} authority={authority} confirmingStop={confirmingStop}
    onStart={noop} onStop={noop} onSelect={noop} onRetry={noop} onDismissError={noop} onConfirmStop={noop} />,
);
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ");

describe("share dialog", () => {
  test("not shared: a start button and the limits, no link or code", () => {
    const html = render(view());
    expect(text(html)).toContain("Not shared");
    expect(text(html)).toContain("Start sharing");
    expect(text(html)).toContain("guest Wi‑Fi");
    expect(html).not.toContain("<svg");
    expect(html).not.toContain(KEY);
  });

  test("sharing with nobody connected says so; the code and link use the first address", () => {
    const html = render(view({ status: "sharing", share: share() }), false, "live");
    expect(text(html)).toContain("Sharing · no device connected");
    expect(html).toContain('aria-label="QR code of the share link"');
    expect(html).toContain(`value="http://192.168.1.23:43821/c/#${KEY}"`);
    expect(html).toMatch(/<input type="radio"[^>]*checked=""[^>]*>[^]*?Wi‑Fi\/Ethernet en0/);
  });

  test("the chosen address drives the link while it is listed", () => {
    const chosen = render(view({ status: "sharing", share: share(), preferred: "utun9/100.64.0.2" }), false, "live");
    expect(chosen).toContain(`value="http://100.64.0.2:43821/c/#${KEY}"`);
    const gone = render(view({ status: "sharing", share: share({ endpoints: [endpoint("en0", "10.0.0.9")] }), preferred: "utun9/100.64.0.2" }), false, "live");
    expect(gone).toContain(`value="http://10.0.0.9:43821/c/#${KEY}"`);
  });

  test("client counts come from the share, and a pause is shown", () => {
    expect(text(render(view({ status: "sharing", share: share({ clients: 1 }) }), false, "live"))).toContain("Sharing · 1 device connected");
    const paused = text(render(view({ status: "sharing", share: share({ phase: "paused" }) }), false, "paused"));
    expect(paused).toContain("Paused");
    expect(paused).toContain("choose a version in the canvas");
  });

  test("an address names its devices only when a socket came in through it", () => {
    const via = share({ clients: 1, endpoints: [{ ...endpoint("en0", "192.168.1.23"), connectedClients: 1, reachable: true }, endpoint("utun9", "100.64.0.2")] });
    const html = text(render(view({ status: "sharing", share: via }), false, "live"));
    expect(html).toContain("192.168.1.23 · 1 device connected via this address");
    expect(html).not.toContain("100.64.0.2 ·");
  });

  test("stop asks inline before it stops", () => {
    expect(text(render(view({ status: "sharing", share: share() }), false, "live"))).not.toContain("lose access");
    const confirming = text(render(view({ status: "sharing", share: share() }), true, "live"));
    expect(confirming).toContain("Stop sharing? Devices on this link lose access at once.");
    expect(confirming).toContain("Cancel");
  });

  test("errors and ended shares are inline", () => {
    const failed = render(view({ error: { code: "not-live", message: "This canvas is not open for sharing." } }));
    expect(failed).toContain('role="alert"');
    expect(text(failed)).toContain("This canvas is not open for sharing.");
    const ended = text(render(view({ status: "ended" })));
    expect(ended).toContain("Sharing ended");
    expect(ended).toContain("Share again");
    expect(text(render(view({ refreshError: "boom" })))).toContain("Could not read this Mac’s addresses: boom");
  });

  test("no address at all is said before starting", () => {
    expect(text(render(view({ addresses: { port: null, generation: 2, endpoints: [] } })))).toContain("no address another device could use");
  });
});

describe("Settings → Device sharing", () => {
  const props = {
    endpoints: [endpoint("en0", "192.168.1.23", "http://192.168.1.23:43821/c/")], stopping: new Set<number>(), stoppingAll: false,
    confirmingAll: false, error: null, onStop: noop, onStopAll: noop, onConfirmAll: noop,
  };

  test("an empty state, and what sharing is and is not", () => {
    const html = text(renderToStaticMarkup(<SharingSettingsView {...props} shares={[]} />));
    expect(html).toContain("Nothing is shared.");
    expect(html).toContain("off whenever Oculus starts");
    expect(html).toContain("anyone with it can edit");
    expect(html).not.toContain("Stop all");
  });

  test("each share with its device count, Stop, and Stop all; no link shown", () => {
    const shares = [
      { share: { fileId: 1, shareId: "a".repeat(32), epoch: "e", clients: 2, phase: "live" as const }, title: "Lecture 3 notes" },
      { share: { fileId: 2, shareId: "b".repeat(32), epoch: "e", clients: 0, phase: "live" as const }, title: "Sketch" },
    ];
    const html = renderToStaticMarkup(<SharingSettingsView {...props} shares={shares} />);
    expect(text(html)).toContain("Lecture 3 notes 2 devices connected");
    expect(text(html)).toContain("Sketch No device connected");
    expect(text(html)).toContain("Stop all");
    expect(html).not.toContain("/c/");
  });
});
