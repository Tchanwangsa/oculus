import { memo } from "react";
import { ShieldWarning } from "@phosphor-icons/react";
import { CodeText } from "@/components/markdown/MdComponents";
import { parsePermissionMeta, type HarnessItem } from "@/lib/harness";

function headline(action: string | undefined): string {
  switch (action) {
    case "command":
      return "Antigravity stopped — it needed permission to run this command";
    case "write_file":
      return "Antigravity stopped — it needed permission to write here";
    case "read_file":
      return "Antigravity stopped — it needed permission to read this";
    case "read_url":
      return "Antigravity stopped — it needed permission to open this page";
    default:
      return "Antigravity stopped — it needed permission to go on";
  }
}

/**
 * A refusal from Antigravity's old print mode (a `permission` row), kept as
 * what happened. Antigravity now asks through requests (`RequestRow`), so
 * nothing here can be allowed.
 */
export const PermissionCard = memo(function PermissionCard({ item }: { item: HarnessItem }) {
  const meta = parsePermissionMeta(item);
  const target = meta.target ?? item.content ?? "";
  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5">
      <ShieldWarning size={14} className="mt-0.5 shrink-0 text-muted-foreground" />
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
      </div>
    </div>
  );
});
