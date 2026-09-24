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

/** What the student approves by pressing the button, in words. The rule
 *  itself is `agy`'s syntax and rides in the tooltip. A folder is named by its
 *  last segment — the refused path is printed in full right above. */
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

/** What an approval sends once it is stored. Short on purpose: the agent
 *  already knows what it was refused, from its own turn. */
const CARRY_ON = "Approved — go ahead.";

/**
 * Where an Antigravity turn ended on a refusal (`permission` rows, written by
 * `app/src-tauri/src/harness/antigravity.rs`). `agy`'s print mode does not ask
 * — it refuses and the turn is over — so this card is the question it could
 * not put: what it tried, and one button that stores the suggested rule and
 * sends the next message, which resumes the conversation under it.
 *
 * Drawn like the sign-in card: the timeline's `card` with the mark in `brand`,
 * because nothing broke and one press fixes it. Only the latest refusal in a
 * thread carries the button; an older one is history, and the approval it
 * would have given is idempotent anyway.
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

  // A reload shows "Allowed" for a rule the student already gave — here or
  // from another thread — rather than a button that would change nothing.
  // A local settings read; nothing is spawned or spent.
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
    // The rule is stored whatever happens next; a send that fails arrives as
    // an error row of its own, so it is not said twice here.
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
