import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Archive,
  CaretDown,
  CaretRight,
  CircleNotch,
  FileText,
  Warning,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { getInboxEntries, type DbInboxEntry, type DbInboxItem } from "@/lib/db";
import { useInboxStore } from "@/stores/inboxStore";
import { displayCode, fmtAgo, sqliteUtcToMs } from "@/lib/format";
import { cn } from "@/lib/utils";

type Filter = "all" | "unread" | "archived";

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unread", label: "Unread" },
  { id: "archived", label: "Archived" },
];

function Entries({ itemId }: { itemId: number }) {
  const [entries, setEntries] = useState<DbInboxEntry[] | null>(null);
  const items = useInboxStore((s) => s.items); // re-read as summaries land

  useEffect(() => {
    getInboxEntries(itemId).then(setEntries).catch(() => setEntries([]));
  }, [itemId, items]);

  if (!entries) return null;

  // Group by subject so a multi-subject sync reads as sections, not a pile.
  // A note written by an automation has no file and no subject behind it, so
  // it groups under "" and renders without a heading.
  const groups = new Map<string, DbInboxEntry[]>();
  for (const e of entries) {
    const key = e.action === "note" ? "" : (e.subject_code ?? "Other");
    groups.set(key, [...(groups.get(key) ?? []), e]);
  }

  return (
    <div className="pl-6 pr-2 pb-3 flex flex-col gap-4">
      {[...groups.entries()].map(([code, rows]) => (
        <div key={code}>
          {code !== "" && (
            <div className="text-[11px] font-medium text-muted-foreground mb-1.5">
              {code === "Other" ? code : displayCode(code)}
            </div>
          )}
          <div className="flex flex-col gap-2.5">
            {rows.map((e) => (
              <div key={e.id}>
                {e.action !== "note" && (
                  <button
                    type="button"
                    onClick={() =>
                      invoke("open_course_file", { relativePath: e.relative_path }).catch(
                        () => {},
                      )
                    }
                    className="inline-flex items-center gap-1.5 text-xs text-foreground hover:text-primary transition-colors"
                  >
                    <FileText size={12} className="shrink-0 text-muted-foreground" />
                    <span className="truncate">{e.filename}</span>
                    {/* Only a sync knows a file is new or updated. One pulled
                        from the library by a "Read Files" node is neither, and
                        labelling it "updated" would be a claim about a sync
                        that never ran. */}
                    {(e.action === "new" || e.action === "updated") && (
                      <span className="text-[10px] text-muted-foreground">{e.action}</span>
                    )}
                  </button>
                )}

                {e.status === "pending" && (
                  <div className="flex items-center gap-1.5 text-xs text-muted-foreground mt-0.5">
                    <CircleNotch size={11} className="animate-spin" /> Summarising…
                  </div>
                )}
                {e.status === "ready" && e.summary_md && (
                  <div className="text-xs text-muted-foreground [&_p]:my-1 [&_p]:text-xs">
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>
                      {e.summary_md}
                    </ReactMarkdown>
                  </div>
                )}
                {(e.status === "skipped" || e.status === "error") && (
                  <div className="flex items-center gap-1.5 text-xs text-muted-foreground mt-0.5">
                    <Warning size={11} className="shrink-0" />
                    {e.summary_md ?? "No summary."}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function Item({ item }: { item: DbInboxItem }) {
  const [open, setOpen] = useState(false);
  const { markRead, archive } = useInboxStore();
  const unread = item.read_at == null;

  return (
    <div className="border-b border-border-subtle last:border-0">
      <div className="flex items-center gap-2 py-2.5">
        <button
          type="button"
          onClick={() => {
            setOpen((o) => !o);
            if (unread) markRead(item.id);
          }}
          className="flex flex-1 items-center gap-2 min-w-0 text-left"
        >
          {open ? (
            <CaretDown size={12} className="shrink-0 text-muted-foreground" />
          ) : (
            <CaretRight size={12} className="shrink-0 text-muted-foreground" />
          )}
          <span
            className={cn(
              "w-1.5 h-1.5 rounded-full shrink-0",
              unread ? "bg-primary" : "bg-transparent",
            )}
          />
          <span
            className={cn(
              "text-[13px] truncate",
              unread ? "font-medium text-foreground" : "text-muted-foreground",
            )}
          >
            {item.title}
          </span>
          {item.status === "pending" && (
            <CircleNotch size={11} className="shrink-0 animate-spin text-muted-foreground" />
          )}
        </button>

        <span className="text-[11px] text-muted-foreground shrink-0 tabular-nums">
          {fmtAgo(sqliteUtcToMs(item.created_at) ?? undefined)}
        </span>
        {item.archived_at == null && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Archive"
            onClick={() => archive(item.id)}
          >
            <Archive size={13} />
          </Button>
        )}
      </div>

      {open && <Entries itemId={item.id} />}
    </div>
  );
}

export default function InboxPage() {
  const [filter, setFilter] = useState<Filter>("all");
  const { items, refresh, loaded } = useInboxStore();

  useEffect(() => {
    refresh(filter === "archived");
  }, [filter, refresh]);

  const shown = items.filter((i) =>
    filter === "unread"
      ? i.read_at == null
      : filter === "archived"
        ? i.archived_at != null
        : i.archived_at == null,
  );

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="shrink-0 border-b border-border-subtle">
        <div className="mx-auto max-w-3xl px-6">
          <div className="pt-5 pb-3">
            <h1 className="text-[22px] font-semibold tracking-tight text-foreground leading-none">
              Inbox
            </h1>
          </div>
          <nav className="flex items-center gap-1">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                type="button"
                onClick={() => setFilter(f.id)}
                className={cn(
                  "-mb-px border-b-2 px-2 pb-2 pt-1 text-[12px] font-medium transition-colors",
                  filter === f.id
                    ? "border-primary text-foreground"
                    : "border-transparent text-muted-foreground hover:text-foreground",
                )}
              >
                {f.label}
              </button>
            ))}
          </nav>
        </div>
      </header>

      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="mx-auto max-w-3xl px-6 py-2">
          {loaded && shown.length === 0 && (
            <p className="text-xs text-muted-foreground py-8 text-center">
              {filter === "archived"
                ? "Nothing archived."
                : "Nothing here yet. Automations deliver their results to this inbox."}
            </p>
          )}
          {shown.map((i) => (
            <Item key={i.id} item={i} />
          ))}
        </div>
      </div>
    </div>
  );
}
