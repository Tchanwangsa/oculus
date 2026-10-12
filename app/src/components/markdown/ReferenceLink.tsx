import { useCallback, useState, type ReactNode } from "react";
import { Globe } from "@phosphor-icons/react";
import { CitationLink, linkCitation, linkText } from "./Citation";
import { ReferencePreview, referenceClass } from "./ReferencePreview";
import { routeReferenceDetails } from "./referenceDetails";
import { routeReferenceAppearance } from "@/lib/files/referenceAppearance";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { faviconFor } from "@/hooks/shell/useBrowserTabs";
import { hostOf } from "@/lib/browser";
import { openBeside } from "@/lib/shell/tabRouters";
import { useTabStore } from "@/stores/shell/tabStore";
import { cn } from "@/lib/utils";

export function ReferenceLink({ href, children }: { href?: string; children: ReactNode }) {
  const cite = linkCitation(href, children);
  if (cite) return <CitationLink shape={cite}>{children}</CitationLink>;
  const route = href ? routeReferenceAppearance(href) : null;
  if (route && href) return <RouteReference href={href} appearance={route}>{children}</RouteReference>;
  if (!/^[a-z][a-z0-9+.-]*:/i.test(href ?? "")) return <span title={href}>{children}</span>;
  if (/^https?:/i.test(href ?? "")) return <WebReference href={href!}>{children}</WebReference>;
  return <a href={href} className="text-brand underline underline-offset-[3px]" target="_blank" rel="noreferrer">{children}</a>;
}

function WebReference({ href, children }: { href: string; children: ReactNode }) {
  const favicon = useBrowserStore((s) => faviconFor(href, s.favicons));
  const [failed, setFailed] = useState<string | null>(null);
  const icon = favicon && failed !== favicon
    ? <img src={favicon} alt="" onError={() => setFailed(favicon)} className="mr-1.5 inline size-3.5 rounded-[2px] object-contain align-[-2px]" />
    : <Globe size={16} aria-hidden className="mr-1.5 inline align-[-3px] text-brand" />;
  return (
    <ReferencePreview icon={icon} title={linkText(children) || hostOf(href)} kind="Website" detail={href}>
      <a href={href} className={cn(referenceClass, "text-brand")} target="_blank" rel="noreferrer">{icon}{children}</a>
    </ReferencePreview>
  );
}

function RouteReference({ href, appearance, children }: {
  href: string;
  appearance: NonNullable<ReturnType<typeof routeReferenceAppearance>>;
  children: ReactNode;
}) {
  const { Icon, tone, kind, id } = appearance;
  const load = useCallback(() => id == null ? Promise.resolve(null) : routeReferenceDetails(kind, id), [kind, id]);
  const icon = <Icon size={16} aria-hidden className={`mr-1.5 inline align-[-3px] ${tone}`} />;
  return (
    <ReferencePreview icon={icon} title={linkText(children) || kind} kind={kind} detail={href} load={load}>
      <a href={href} className={referenceClass} onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.metaKey || e.ctrlKey) useTabStore.getState().addTab(href);
        else openBeside(href);
      }}>{icon}{children}</a>
    </ReferencePreview>
  );
}
