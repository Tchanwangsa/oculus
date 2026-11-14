import { WebIcon } from "./ReferenceLink";
import { ReferencePreview, referenceRestClass } from "./ReferencePreview";
import { hostOf } from "@/lib/browser";
import { parseLink } from "@/lib/harness/links";
import { cn } from "@/lib/utils";

/**
 * A web link in a sent message, drawn as a note draws one: the site's icon and
 * its words (or its host), underlined. The composer draws its links as text
 * instead (`ComposerLink`), so they edit like text.
 */
export function LinkChip({ raw, onClick }: { raw: string; onClick: (newTab: boolean) => void }) {
  const link = parseLink(raw);
  if (!link) return <>{raw}</>;
  const title = link.label ?? hostOf(link.href);
  const icon = <WebIcon href={link.href} />;
  return (
    <ReferencePreview icon={icon} title={title} kind="Website" detail={link.href}>
      <span
        role="link"
        tabIndex={0}
        onClick={(e) => {
          e.stopPropagation();
          if (window.getSelection()?.isCollapsed === false) return;
          onClick(e.metaKey || e.ctrlKey);
        }}
        onKeyDown={(e) => {
          if (e.key !== "Enter" && e.key !== " ") return;
          e.preventDefault();
          e.stopPropagation();
          onClick(e.metaKey || e.ctrlKey);
        }}
        className={cn(referenceRestClass, "mx-px inline cursor-pointer box-decoration-clone whitespace-nowrap text-brand hover:bg-accent")}
      >
        {icon}
        {title}
      </span>
    </ReferencePreview>
  );
}
