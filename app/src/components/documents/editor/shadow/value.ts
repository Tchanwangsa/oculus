import { seedOf, type Basis } from "./basis";
import type { Replay, Step, WasmShadow } from "./types";

/** Transactions a replay keeps; older ones fold into its seed. */
export const RING = 50;

/** One state chain: every state derived from one seed shares it, so the
 *  chain reports once and stops, whichever state found the mismatch. */
export interface Chain {
  stopped: boolean;
}

/** One transaction's steps and the state it started from. */
export interface TxRecord {
  before: Basis;
  steps: readonly Step[];
}

export interface Live {
  readonly shadow: WasmShadow;
  readonly chain: Chain;
  readonly names: readonly string[];
  /** The state the shadow was seeded from. */
  readonly seed: Basis;
  /** The last `RING` transactions, oldest first. */
  readonly records: readonly TxRecord[];
}

/** No wasm yet: the next transaction seeds from its start state. */
export const UNSEEDED = "unseeded";
/** A mismatch or an error ended this chain; nothing more is called. */
export const STOPPED = "stopped";

export type ShadowValue = Live | typeof UNSEEDED | typeof STOPPED;

export const isLive = (v: ShadowValue | undefined): v is Live => typeof v === "object";

/** `live` after one more transaction. */
export function extend(live: Live, shadow: WasmShadow, record: TxRecord): Live {
  const records = live.records.length < RING ? [...live.records, record] : [...live.records.slice(1), record];
  return { ...live, shadow, records };
}

/** The seed of the oldest kept transaction (or of the chain), and the steps since. */
export function replayOf(live: Live): Replay {
  return {
    seed: seedOf(live.records[0]?.before ?? live.seed),
    steps: live.records.flatMap((r) => r.steps),
  };
}
