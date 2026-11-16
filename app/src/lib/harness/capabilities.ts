import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "./providers";

/** `Capabilities` in `app/src-tauri/src/harness/providers/capabilities.rs`:
 *  what one bridge can do. Rust is the authority; nothing here is compiled in. */
export interface Capabilities {
  provider: Provider;
  questions: boolean;
  /** Stop ends the running turn. */
  interrupt: boolean;
  /** The CLI can drop a question from its own context: gates Rewind, Edit and Retry. */
  rewind: boolean;
  /** One server process answers every chat of this provider, so Force stop
   *  ends the other chats' turns too. */
  sharedProcess: boolean;
  /** A message can be written into the running turn. */
  activeSteering: boolean;
  /** A running turn can be stopped and started again with a correction. */
  steerByRestart: boolean;
  /** A live session takes another model with its next turn; where false a
   *  model change restarts the session and resumes its conversation. */
  modelSwitchInSession: boolean;
  /** The bridge can take over a chat another provider started, from the
   *  earlier conversation Rust hands it. Where false its models are locked
   *  in a started chat of another provider's: start a new chat to switch. */
  providerHandoff: boolean;
  /** The CLI asks before what its sandbox blocks and the student answers in
   *  the chat (`HarnessRequest`). Where false such a step is declined. */
  approvals: boolean;
  /** Auto hands requests to the CLI's own reviewer and the student is never
   *  asked. Where false, Auto behaves as Ask and the mode control says so. */
  autoReview: boolean;
  /** Some answers reach past one call; each option's `scope` says how far. */
  sessionGrants: boolean;
  /** A live session takes a new permission mode with its next turn. */
  modeSwitchInSession: boolean;
}

/** What a provider is taken to do until Rust has answered: nothing optional. */
function unknown(provider: Provider): Capabilities {
  return {
    questions: false, provider, interrupt: false, rewind: false, sharedProcess: false, activeSteering: false, steerByRestart: false,
    modelSwitchInSession: false, providerHandoff: false, approvals: false, autoReview: false,
    sessionGrants: false, modeSwitchInSession: false,
  };
}

/** One shared answer for every reader, asked for once. `read` is explicit so
 *  the cache can be checked without mocking global IPC. */
export function createCapabilities(read: () => Promise<Capabilities[]>) {
  let rows: Capabilities[] | null = null;
  let inFlight: Promise<Capabilities[]> | null = null;
  const fallback = new Map<Provider, Capabilities>();
  const listeners = new Set<() => void>();
  const load = (): Promise<Capabilities[]> => {
    if (rows) return Promise.resolve(rows);
    inFlight ??= read()
      .then((answer) => {
        rows = answer;
        for (const l of listeners) l();
        return answer;
      })
      // A failed read is asked again by the next reader.
      .finally(() => {
        inFlight = null;
      });
    return inFlight;
  };
  return {
    load,
    /** The cached answer for `provider`; a stable object, so it can back
     *  `useSyncExternalStore`. Asks Rust on first use. */
    of: (provider: Provider): Capabilities => {
      const known = rows?.find((c) => c.provider === provider);
      if (known) return known;
      if (!rows) void load().catch(() => {});
      let none = fallback.get(provider);
      if (!none) fallback.set(provider, (none = unknown(provider)));
      return none;
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
}

export const capabilities = createCapabilities(() => invoke<Capabilities[]>("harness_capabilities"));
