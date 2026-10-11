import { ArrowSquareOut, FilePdf } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useMineruKey } from "@/hooks/keys/useMineruKey";
import { OptionCard } from "../OptionCard";
import { KeyField } from "./KeyField";

/** MinerU's API-token page; it asks for a sign-in first. */
const MINERU_TOKENS = "https://mineru.net/apiManage/token";

/** The recommended key: MinerU's cloud parser turns the library's PDFs into
 *  text. Saving it goes through `useMineruKey`, the same path as Settings →
 *  Parsing, so a key saved here re-queues PDFs that failed without one. */
export function MineruCard() {
  const field = useMineruKey();
  const connected = field.hasKey === true;
  return (
    <div>
      <OptionCard
        icon={<FilePdf weight="duotone" />}
        title="MinerU — reads your PDFs"
        chip={
          connected ? (
            <Badge variant="success">Connected</Badge>
          ) : (
            <Badge>Recommended</Badge>
          )
        }
        selected={connected}
        detail="Free: up to 5,000 files a day. Sign up, create a token, done — tokens last 90 days."
        actions={
          connected ? undefined : (
            <Button
              variant="outline"
              size="xs"
              onClick={() => void openUrl(MINERU_TOKENS)}
            >
              Get a key
              <ArrowSquareOut />
            </Button>
          )
        }
      >
        <KeyField
          label="MinerU API token"
          field={field}
          connected={connected}
          placeholder="Paste your MinerU token"
        />
        {field.hasKey === null && field.checkError && (
          <p data-selectable className="mt-2 text-xs/relaxed text-muted-foreground">
            Couldn't check for a saved token: {field.checkError}
          </p>
        )}
      </OptionCard>
      {field.hasKey === false && (
        <p className="mt-2 px-1 text-xs/relaxed text-muted-foreground">
          Without a key your PDFs stay unparsed, and the library marks each one. You can add it
          later in Settings → Parsing.
        </p>
      )}
    </div>
  );
}
