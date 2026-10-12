import { CheckCircle } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { CloudKey } from "@/hooks/keys/useCloudKey";
import { cn } from "@/lib/utils";

/**
 * A key's paste field sized for a step's card: a full-width password input
 * and a Save pill, or "Connected" with Remove once a key is saved. The notes
 * are Settings' (`CredentialField`): Rust's refusal, or the unverified warning.
 */
export function KeyField({
  label,
  field,
  connected,
  placeholder,
}: {
  label: string;
  field: CloudKey;
  connected: boolean;
  placeholder: string;
}) {
  return (
    <div>
      {connected ? (
        <div className="flex h-8 items-center gap-2">
          <CheckCircle size={16} weight="fill" className="text-success" />
          <span className="text-[13px] text-foreground">Connected</span>
          <span className="text-xs text-muted-foreground">Stored in your Mac keychain.</span>
          <Button
            variant="ghost"
            size="xs"
            className="ml-auto text-muted-foreground"
            onClick={() => void field.remove()}
          >
            Remove
          </Button>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <Input
            aria-label={label}
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={field.key}
            onChange={(event) => field.setKey(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void field.save();
            }}
            placeholder={placeholder}
          />
          <Button
            size="sm"
            className="min-w-20"
            disabled={!field.key.trim() || field.busy}
            onClick={() => void field.save()}
          >
            {field.busy ? "Checking…" : "Save"}
          </Button>
        </div>
      )}
      {field.note && (
        <p
          data-selectable
          className={cn(
            "mt-2 text-xs/relaxed",
            field.note.kind === "error" ? "text-destructive" : "text-warning",
          )}
        >
          {field.note.text}
        </p>
      )}
    </div>
  );
}
