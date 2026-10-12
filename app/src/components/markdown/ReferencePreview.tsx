import { useEffect, useState, type ReactElement, type ReactNode } from "react";
import { HoverCard } from "radix-ui";
import { cn } from "@/lib/utils";

export const referenceClass = "rounded-sm px-0.5 py-px text-foreground underline decoration-border underline-offset-[3px] transition-colors hover:bg-accent hover:decoration-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

export interface ReferenceDetails {
  title?: string;
  description?: string;
  meta?: string;
}

/** A portal keeps previews out of prose layout and the composer's editable DOM. */
export function ReferencePreview({ children, icon, title, kind, detail, load }: {
  children: ReactElement;
  icon: ReactNode;
  title: string;
  kind: string;
  detail: string;
  load?: () => Promise<ReferenceDetails | null>;
}) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    const dismiss = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("keydown", dismiss);
    return () => document.removeEventListener("keydown", dismiss);
  }, [open]);
  return (
    <HoverCard.Root open={open} onOpenChange={setOpen} openDelay={400} closeDelay={150}>
      <HoverCard.Trigger asChild>{children}</HoverCard.Trigger>
      <HoverCard.Portal>
        <HoverCard.Content
          side="top" align="start" sideOffset={8} collisionPadding={12}
          onClick={(e) => e.stopPropagation()}
          className="z-50 w-80 max-w-[calc(100vw-24px)] rounded-2xl border border-border bg-card p-4 text-card-foreground shadow-lg outline-none"
        >
          <PreviewBody icon={icon} title={title} kind={kind} detail={detail} load={load} />
        </HoverCard.Content>
      </HoverCard.Portal>
    </HoverCard.Root>
  );
}

function PreviewBody({ icon, title, kind, detail, load }: Omit<React.ComponentProps<typeof ReferencePreview>, "children">) {
  const [details, setDetails] = useState<ReferenceDetails | null>(null);
  useEffect(() => {
    if (!load) return;
    let live = true;
    load().then((value) => { if (live) setDetails(value); }).catch(() => {});
    return () => { live = false; };
  }, [load]);
  return (
    <span className="block text-left font-sans text-[13px] leading-relaxed">
      <span className="flex items-start gap-2.5">
        <span className="mt-0.5 shrink-0 [&>svg]:size-5 [&>img]:size-5">{icon}</span>
        <span className="min-w-0 truncate font-medium text-foreground" title={details?.title || title}>{details?.title || title}</span>
      </span>
      {details?.description && <span className="mt-2.5 block line-clamp-3 text-muted-foreground">{details.description}</span>}
      <span className={cn("mt-2.5 block text-xs text-muted-foreground", details?.description && "mt-3")}>{[kind, details?.meta].filter(Boolean).join(" · ")}</span>
      <span className="mt-1 block truncate text-xs text-muted-foreground/80" title={detail}>{detail}</span>
    </span>
  );
}
