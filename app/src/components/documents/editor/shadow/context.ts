import { replayOf, type Chain, type Live } from "./value";
import type { MismatchKind, Reporter, ShadowWasm } from "./types";

/** What the field, the checks and the plugin share: the wasm (or null until
 *  it loads), the reporter, and the switch a wasm trap throws. */
export class Context {
  /** Set by a trap: the instance's memory can no longer be trusted. */
  private trapped = false;

  constructor(
    readonly wasm: () => ShadowWasm | null,
    private readonly reporter: Reporter,
  ) {}

  get off(): boolean {
    return this.trapped;
  }

  /** Reports the chain's first mismatch, with a replay, and stops it. */
  mismatch(live: Live, kind: MismatchKind, details: Record<string, unknown>) {
    if (live.chain.stopped || this.trapped) return;
    live.chain.stopped = true;
    try {
      this.reporter.mismatch(kind, { kind, ...details, replay: replayOf(live) });
    } catch {
      // A reporter that throws must not reach CodeMirror either.
    }
  }

  /** Runs `fn`; an exception stops `chain` (all chains on a trap), is
   *  reported once, and gives `fallback`. */
  guard<T>(chain: Chain | null, fallback: T, fn: () => T): T {
    try {
      return fn();
    } catch (e) {
      if (chain?.stopped || this.trapped) return fallback;
      if (chain) chain.stopped = true;
      if (typeof WebAssembly !== "undefined" && e instanceof WebAssembly.RuntimeError) this.trapped = true;
      try {
        this.reporter.error(e);
      } catch {
        // As above.
      }
      return fallback;
    }
  }
}
