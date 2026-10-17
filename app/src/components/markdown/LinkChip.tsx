import { WebIcon } from "./ReferenceLink";
import { ReferencePreview, referenceRestClass } from "./ReferencePreview";
import { hostOf } from "@/lib/browser";
import { parseLink } from "@/lib/harness/links";
import { cn } from "@/lib/utils";

/**
 * A web link in a message, drawn as a note draws one: the site's icon and its
 * words (or its host), underlined. Like `FileChip`: in the composer it is
 * atomic to the caret and `data-raw` is read back into the message; with
 * `onClick` it is a link.
 */
export function LinkChip({ raw, onClick }: { raw: string; onClick?: (newTab: boolean) => void }) {
  const link = parseLink(raw);
  if (!link) return <>{raw}</>;
  const title = link.label ?? hostOf(link.href);
  const icon = <WebIcon href={link.href} />;
  return (
    <ReferencePreview icon={icon} title={title} kind="Website" detail={link.href}>
      <span
        contentEditable={onClick ? undefined : false}
        data-raw={raw}
        {...(onClick
          ? {
              role: "link",
              tabIndex: 0,
              onClick: (e: React.MouseEvent) => {
                e.stopPropagation();
                if (window.getSelection()?.isCollapsed === false) return;
                onClick(e.metaKey || e.ctrlKey);
              },
              onKeyDown: (e: React.KeyboardEvent) => {
                if (e.key !== "Enter" && e.key !== " ") return;
                e.preventDefault();
                e.stopPropagation();
                onClick(e.metaKey || e.ctrlKey);
              },
            }
          : {})}
        className={cn(referenceRestClass, "mx-px inline box-decoration-clone whitespace-nowrap text-brand", onClick && "cursor-pointer hover:bg-accent")}
      >
        {icon}
        {title}
      </span>
    </ReferencePreview>
  );
}
