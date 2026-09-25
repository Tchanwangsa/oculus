import { memo, useEffect, useState } from "react";
import { Check, CircleNotch, ShieldWarning } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { CodeText } from "@/components/markdown/MdComponents";
import {
  harnessAntigravityAllow,
  harnessAntigravityRules,
  parsePermissionMeta,
  splitRule,
  type HarnessItem,
} from "@/lib/harness";
import { useHarnessStore } from "@/stores/harnessStore";
import { cn } from "@/lib/utils";

/** The approval in words; the raw `agy` rule rides in the tooltip. */
function allowLabel(rule: string): string {
  const r = splitRule(rule);
  if (!r) return "Allow";
  const leaf = r.value.split("/").filter(Boolean).pop() ?? r.value;
  switch (r.action) {
    case "command":
      return `Allow ${r.value}`;
    case "write_file":
      return `Allow writes in ${leaf}`;
    case "read_file":
      return `Allow reading ${leaf}`;
    case "read_url":
      return `Allow reading ${r.value}`;
  }
}

function headline(action: string | undefined): string {
  switch (action) {
    case "command":
      return "Antigravity stopped — it needs permission to run this command";
    case "write_file":
      return "Antigravity stopped — it needs permission to write here";
    case "read_file":
      return "Antigravity stopped — it needs permission to read this";
    case "read_url":
      return "Antigravity stopped — it needs permission to open this page";
    default:
      return "Antigravity stopped — it needs permission to go on";
  }
}

/** Sent after an approval is stored; the agent already knows what was refused. */
const CARRY_ON = "Approved — go ahead.";

/**
 * An Antigravity refusal (`permission` rows from
 * `app/src-tauri/src/harness/antigravity.rs`). `agy`'s print mode refuses and
 * ends the turn rather than asking, so the button stores the suggested rule and
 * sends a follow-up. Only the latest refusal (`actionable`) carries it.
 */
export const PermissionCard = memo(function PermissionCard({
  item,
  actionable,
  onFollowUp,
}: {
  item: HarnessItem;
  actionable: boolean;
  /** Send a message on this thread the way the composer would. */
  onFollowUp?: (text: string) => Promise<void>;
}) {
  const meta = parsePermissionMeta(item);
  const target = meta.target ?? item.content ?? "";
  const rule = actionable ? meta.rule : null;
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const [phase, setPhase] = useState<"idle" | "pending" | "allowed">("idle");
  const [error, setError] = useState<string | null>(null);

  // Show "Allowed" for a rule already stored (a local settings read).
  useEffect(() => {
    if (!rule) return;
    let stale = false;
    harnessAntigravityRules()
      .then((rules) => {
        if (!stale && rules.includes(rule)) setPhase((p) => (p === "idle" ? "allowed" : p));
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [rule]);

  const allow = async () => {
    if (!rule) return;
    setPhase("pending");
    setError(null);
    try {
      await harnessAntigravityAllow(item.thread_id, rule);
    } catch (e) {
      setError(String(e));
      setPhase("idle");
      return;
    }
    // A failed send surfaces as its own error row.
    await onFollowUp?.(CARRY_ON).catch((e) => console.error("harness send failed", e));
    setPhase("allowed");
  };

  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5">
      <ShieldWarning
        size={14}
        className={cn("mt-0.5 shrink-0", rule ? "text-brand" : "text-muted-foreground")}
      />
      <div className="min-w-0 flex-1">
        <div className="text-xs text-foreground">{headline(meta.action)}</div>
        {target &&
          (meta.action === "command" ? (
            <CodeText className="mt-1 text-muted-foreground">$ {target}</CodeText>
          ) : (
            <p className="mt-0.5 break-all text-[11.5px] leading-relaxed text-muted-foreground">
              {target}
            </p>
          ))}
        {rule && (
          <div data-copy-skip>
            <div className="mt-2 flex items-center gap-2">
              {phase === "allowed" ? (
                <span className="flex items-center gap-1 text-xs text-muted-foreground">
                  <Check size={12} />
                  Allowed
                </span>
              ) : (
                <Button
                  size="xs"
                  title={rule}
                  className="max-w-full min-w-0"
                  disabled={busy || phase === "pending"}
                  onClick={() => void allow()}
                >
                  {phase === "pending" && <CircleNotch size={12} className="animate-spin" />}
                  <span className="truncate">{allowLabel(rule)}</span>
                </Button>
              )}
            </div>
            {error && (
              <p className="mt-1.5 break-words text-[11px] leading-relaxed text-destructive">{error}</p>
            )}
            <p className="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
              Approvals also apply to agy in your terminal.
            </p>
          </div>
        )}
      </div>
    </div>
  );
});
