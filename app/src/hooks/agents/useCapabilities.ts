import { useSyncExternalStore } from "react";
import { capabilities, type Capabilities, type Provider } from "@/lib/harness";

/** What `provider`'s bridge can do, from Rust (`harness_capabilities`). Until
 *  the answer lands every optional action reads as unavailable. */
export function useCapabilities(provider: Provider): Capabilities {
  return useSyncExternalStore(capabilities.subscribe, () => capabilities.of(provider));
}
