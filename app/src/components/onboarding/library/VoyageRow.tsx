import { ArrowSquareOut, MagnifyingGlass } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useKeyPresence } from "@/hooks/keys/useKeyPresence";
import { useVoyageKey } from "@/hooks/keys/useVoyageKey";
import { OptionCard } from "../OptionCard";
import { KeyField } from "./KeyField";

/** Where Voyage keys are made; the same link Settings → Search index uses. */
const VOYAGE_DASHBOARD = "https://dashboard.voyageai.com/";

/** The optional key, quieter than MinerU's card: page-image search through
 *  Voyage. Without it the agent still searches the library by text. */
export function VoyageRow() {
  const { hasKey, setHasKey } = useKeyPresence("voyage_has_api_key");
  const field = useVoyageKey({
    onSaved: () => setHasKey(true),
    onRemoved: () => setHasKey(false),
  });
  const connected = hasKey === true;
  return (
    <OptionCard
      className="border-dashed bg-transparent p-3.5"
      icon={<MagnifyingGlass />}
      title="Also add page search (Voyage)"
      chip={connected ? <Badge variant="success">Connected</Badge> : undefined}
      detail="Lets the agent search page images. Optional — without it the agent still searches your library by text."
      actions={
        connected ? undefined : (
          <Button
            variant="ghost"
            size="xs"
            className="text-muted-foreground"
            onClick={() => void openUrl(VOYAGE_DASHBOARD)}
          >
            Get a key
            <ArrowSquareOut />
          </Button>
        )
      }
    >
      <KeyField
        label="Voyage API key"
        field={field}
        connected={connected}
        placeholder="Paste your Voyage key"
      />
    </OptionCard>
  );
}
