import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export function CredentialField({
  label, value, connected, busy, placeholder, onChange, onSave, onRemove,
  status, note, children,
}: {
  label: string;
  value: string;
  connected: boolean;
  busy: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSave: () => void;
  onRemove: () => void;
  status?: ReactNode;
  note: { kind: "error" | "warn"; text: string } | null;
  children?: ReactNode;
}) {
  return (
    <div className="py-2">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="text-xs text-foreground">{label}</p>
          <p className="text-[11px] text-muted-foreground">
            Stored in your Mac keychain, never in the library database.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {connected ? (
            <>
              <span className="text-xs text-success">Connected</span>
              <Button variant="outline" size="xs" onClick={onRemove}>Remove</Button>
            </>
          ) : (
            <>
              {status}
              <Input
                aria-label={label}
                type="password"
                autoComplete="off"
                value={value}
                onChange={(event) => onChange(event.target.value)}
                onKeyDown={(event) => { if (event.key === "Enter") onSave(); }}
                placeholder={placeholder}
                className="h-7 w-44 text-xs"
              />
              <Button size="xs" disabled={!value.trim() || busy} onClick={onSave}>
                {busy ? "Checking…" : "Save"}
              </Button>
            </>
          )}
        </div>
      </div>
      {note && (
        <p className={cn("mt-2 text-[11px] leading-relaxed", note.kind === "error" ? "text-destructive" : "text-warning")}>
          {note.text}
        </p>
      )}
      {children}
    </div>
  );
}
