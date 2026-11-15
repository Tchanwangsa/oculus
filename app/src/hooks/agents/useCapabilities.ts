import { useSyncExternalStore } from "react";
import { capabilities, type Capabilities, type Provider } from "@/lib/harness";

/** What `provider`'s bridge can do, from Rust (`harness_capabilities`). Until
 *  the answer lands every optional action reads as unavailable. */
export function useCapabilities(provider: Provider): Capabilities {
  const read = () => capabilities.of(provider);
  // The same read on a server render (a test's), which has no store to wait for.
  return useSyncExternalStore(capabilities.subscribe, read, read);
}
