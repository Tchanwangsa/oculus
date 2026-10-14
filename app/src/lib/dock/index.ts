import { invoke } from "@tauri-apps/api/core";
import {
  InstagramLogo,
  MessengerLogo,
  WhatsappLogo,
  type Icon,
} from "@phosphor-icons/react";
import { DiscordIcon } from "@/components/icons/DiscordIcon";
import type { Viewport } from "@/lib/browser";

/**
 * The messaging dock. Each enabled service is one native WebView Rust owns
 * (`app/src-tauri/src/shell/dock/`), alive from enable to disable and parked
 * off-screen while no slot shows it. A `/dock/:service` route reports its
 * slot, as `/browse/:id` does, but dock pages are not browser tabs: they live
 * in their own snapshot (`dock-state`) and never enter the browser's list.
 */

export type DockServiceId = "whatsapp" | "instagram" | "messenger" | "discord";

export interface DockService {
  id: DockServiceId;
  label: string;
  icon: Icon;
  /** One line under the service's toggle in Settings → Messaging. */
  description: string;
}

/** In rail and Settings order. */
export const DOCK_SERVICES: readonly DockService[] = [
  {
    id: "whatsapp",
    label: "WhatsApp",
    icon: WhatsappLogo,
    description: "Link your phone with a QR code.",
  },
  {
    id: "instagram",
    label: "Instagram",
    icon: InstagramLogo,
    description: "Instagram's direct messages.",
  },
  {
    id: "messenger",
    label: "Messenger",
    icon: MessengerLogo,
    description: "Restore chats with your Messenger PIN.",
  },
  {
    id: "discord",
    label: "Discord",
    icon: DiscordIcon,
    description: "Discord's servers and direct messages.",
  },
];

/** One created (enabled) page, as Rust reports it. */
export interface DockPageInfo {
  id: string;
  title: string;
  url: string;
  loading: boolean;
  /** Unread count read from the page's title; 0 = none. */
  unread: number;
}

/** One arrival in the Inbox, newest first. Rust keeps the feed in memory
 *  only, so message text never reaches disk; the frontend doesn't persist it
 *  either. */
export interface InboxItem {
  id: string;
  service: string;
  sender: string;
  preview: string;
  /** Unix ms. */
  at: number;
}

export interface DockSnapshot {
  pages: DockPageInfo[];
  /** The service placed in a slot right now, if any. */
  shown: string | null;
}

/** The Messaging rail item's route: the Inbox, with a tab per service. */
export const DOCK_ROOT = "/dock";
const DOCK_PREFIX = `${DOCK_ROOT}/`;

export function isDockServiceId(id: unknown): id is DockServiceId {
  return DOCK_SERVICES.some((s) => s.id === id);
}

export function dockServiceById(id: string | undefined): DockService | null {
  return DOCK_SERVICES.find((s) => s.id === id) ?? null;
}

export function dockPath(id: DockServiceId): string {
  return `${DOCK_PREFIX}${id}`;
}

/** The dock service a route names, or null for any other route. */
export function dockService(path: string | undefined): DockService | null {
  if (!path?.startsWith(DOCK_PREFIX)) return null;
  return dockServiceById(path.slice(DOCK_PREFIX.length).split(/[/?#]/)[0]);
}

export const dock = {
  list: () => invoke<DockSnapshot>("dock_list"),
  /** Creates or closes the service's page. Persisting the choice is the
   *  frontend's (`dockPrefsStore`); Rust keeps no record of it. */
  setEnabled: (id: DockServiceId, enabled: boolean) =>
    invoke<DockSnapshot>("dock_set_enabled", { id, enabled }),
  /** Shows the page in `viewport` without taking keyboard focus. */
  place: (id: DockServiceId, viewport: Viewport) =>
    invoke("dock_place", { id, viewport }),
  setViewport: (id: DockServiceId, viewport: Viewport) =>
    invoke("dock_set_viewport", { id, viewport }),
  /** Moves the page off-screen at its last size; never `hide()`, which
   *  throttles its timers. */
  park: (id: DockServiceId) => invoke("dock_park", { id }),
  reload: (id: DockServiceId) => invoke("dock_reload", { id }),
  /** The Inbox so far, newest first; `dock-inbox` brings each new arrival. */
  inbox: () => invoke<InboxItem[]>("dock_inbox"),
  clearInbox: () => invoke("dock_inbox_clear"),
};
