import { useEffect } from "react";

import { useProviderModels } from "@/hooks/agents/useProviderModels";
import { defaultSelection } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * Fills the harness store's model and reasoning level from the selected
 * provider's list when none is chosen, as the composer does, for a page that
 * sends a turn without one. A turn always names an explicit model.
 */
export function useDefaultModel() {
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const { providers } = useProviderModels(provider);
  const active = providers.find((p) => p.id === provider);
  useEffect(() => {
    if (model || !active || active.loading || active.models.length === 0) return;
    const pick = defaultSelection(active.models);
    if (!pick.model) return;
    const store = useHarnessStore.getState();
    store.setModel(pick.model);
    store.setReasoning(pick.reasoning);
  }, [model, active]);
}
