import { NotePencil } from "@phosphor-icons/react";
import { SideNavSearch } from "@/components/ui/layout/SideNav";
import { cn } from "@/lib/utils";

/** The column's header: the search field with the new-chat button at its
 *  right, lit while no thread is open. */
export function ListHeader({
  query,
  onQuery,
  onSearchEnter,
  fresh,
  onNew,
}: {
  query: string;
  onQuery: (query: string) => void;
  /** Enter in the field: open the first result. */
  onSearchEnter: () => void;
  /** No thread is open: bare `/chat` is the new chat. */
  fresh: boolean;
  onNew: () => void;
}) {
  return (
    <div className="flex items-center gap-1.5 pt-3 pr-2 pb-2 pl-2.5">
      <SideNavSearch
        value={query}
        onChange={onQuery}
        onEnter={onSearchEnter}
        placeholder="Search chats"
        className="min-w-0 flex-1 p-0"
      />
      <button
        type="button"
        data-side-nav-close
        aria-label="New chat"
        title="New chat"
        aria-current={fresh ? "page" : undefined}
        onClick={onNew}
        className={cn(
          "flex size-[30px] shrink-0 items-center justify-center rounded-lg transition-colors",
          fresh
            ? "bg-sidebar-item-active text-foreground"
            : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
        )}
      >
        <NotePencil size={17} />
      </button>
    </div>
  );
}
