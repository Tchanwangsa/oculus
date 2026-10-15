import { Fragment, useRef, useState, type ReactElement } from "react";
import { CaretRight, DotsThree } from "@phosphor-icons/react";
import { navigateActive } from "@/lib/shell/tabRouters";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { Popover, PopoverContent, PopoverTrigger } from "./popover";

export interface BreadcrumbItem {
  label: string;
  href?: string;
  icon: ReactElement;
}

/** Shared page path; hidden ancestors stay navigable in a popover. */
export function Breadcrumb({ items }: { items: BreadcrumbItem[] }) {
  return (
    <nav aria-label="Breadcrumb" className="flex min-w-0 flex-1 items-center gap-1.5 text-[13px] text-muted-foreground">
      <BreadcrumbItems items={items} />
    </nav>
  );
}

/** Also fits page headers whose leaf title is rendered beside the path. */
export function BreadcrumbItems({ items, trailing = false, visible = 2 }: {
  items: BreadcrumbItem[];
  trailing?: boolean;
  visible?: number;
}) {
  const hidden = items.slice(0, Math.max(0, items.length - visible));
  const shown = items.slice(hidden.length);
  return <>
    {hidden.length > 0 && <><BreadcrumbOverflow items={hidden} /><Separator /></>}
    {shown.map((item, index) => (
      <Fragment key={`${item.href ?? item.label}-${index}`}>
        <Crumb item={item} current={!trailing && index === shown.length - 1} />
        {(trailing || index < shown.length - 1) && <Separator />}
      </Fragment>
    ))}
  </>;
}

function Crumb({ item, current = false, onNavigate }: {
  item: BreadcrumbItem;
  current?: boolean;
  onNavigate?: () => void;
}) {
  const content = <>{item.icon}<span className="min-w-0 truncate">{item.label}</span></>;
  const style = "flex min-w-0 items-center gap-1.5 rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring";
  return item.href ? (
    <button type="button" data-tab-href={item.href} title={item.label}
      onClick={() => { onNavigate?.(); navigateActive(item.href!); }}
      className={`${style} cursor-pointer text-left transition-colors hover:text-foreground`}>
      {content}
    </button>
  ) : <span title={item.label} aria-current={current ? "page" : undefined}
      className={`${style} ${current ? "font-medium text-foreground" : ""}`}>{content}</span>;
}

export function BreadcrumbOverflow({ items }: { items: BreadcrumbItem[] }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" aria-label="Show parent pages" className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-md transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring">
          <DotsThree size={17} weight="bold" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-72 p-1">
        <div ref={ref} className="max-h-64 overflow-y-auto" aria-label="Parent pages">
          {items.map((item, i) => <div key={`${item.href ?? item.label}-${i}`} className="rounded-md px-2 py-2 text-xs text-muted-foreground hover:bg-accent [&>button]:w-full">
            <Crumb item={item} onNavigate={() => setOpen(false)} />
          </div>)}
        </div>
      </PopoverContent>
    </Popover>
  );
}

function Separator() {
  return <CaretRight size={10} aria-hidden className="shrink-0 text-muted-foreground/50" />;
}
