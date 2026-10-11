import { isWindows } from "@/lib/platform";
import type { Provider } from "./providers";

export interface ProviderAvailability {
  supported: boolean;
  install: boolean;
  update: boolean;
  note: string | null;
}

const AVAILABLE: ProviderAvailability = { supported: true, install: true, update: true, note: null };

/** Platform capabilities shared by Settings, model pickers and defaults. */
export function providerAvailability(provider: Provider, windows = isWindows): ProviderAvailability {
  if (!windows) return AVAILABLE;
  switch (provider) {
    case "claude":
      return { supported: false, install: false, update: false, note: "Needs WSL2 (coming soon)." };
    case "antigravity":
      return {
        supported: true,
        install: false,
        update: false,
        note: "Install and update Antigravity in your terminal, then Recheck.",
      };
    default:
      return AVAILABLE;
  }
}

/** New chats start with an agent this desktop can run. */
export const DEFAULT_CHAT_PROVIDER: Provider = providerAvailability("claude").supported ? "claude" : "codex";
