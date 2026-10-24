import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { DotsThree, Trash } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Input } from "@/components/ui/input";
import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
} from "@/components/ui/layout/SideNav";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { deleteTopic, updateTopic, type RevisionTopic } from "@/lib/revision";

/**
 * A topic in the Revision nav: a link that selects it and its entry count.
 * A named topic adds a `…` on hover for rename and delete, sitting over the
 * link so no button is nested in an anchor. Unsorted has no menu.
 */
export function TopicNavRow({
  topic,
  label,
  icon,
  to,
  active,
  count,
  onDeleted,
}: {
  /** Absent for the Unsorted row. */
  topic?: RevisionTopic;
  label: ReactNode;
  icon: ReactNode;
  /** The topic selected, or cleared if it already is. */
  to: string;
  active: boolean;
  count: number;
  onDeleted?: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState(topic?.name ?? "");
  const [error, setError] = useState<string | null>(null);

  const rename = async () => {
    if (!topic || name.trim() === "" || name.trim() === topic.name) {
      setName(topic?.name ?? "");
      setError(null);
      return;
    }
    try {
      await updateTopic(topic.id, { name: name.trim() });
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async () => {
    if (!topic) return;
    setBusy(true);
    try {
      await deleteTopic(topic.id);
      setConfirming(false);
      onDeleted?.();
    } catch (e) {
      setConfirming(false);
      setMenu(true);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="group/topic relative">
      <Link
        to={to}
        aria-current={active ? "page" : undefined}
        className={cn(SIDE_NAV_ROW, active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
      >
        <span className="flex size-4 shrink-0 items-center justify-center">{icon}</span>
        <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{label}</span>
        <span
          className={cn(
            "w-6 shrink-0 text-right text-[11px] tabular-nums text-muted-foreground/70",
            topic && "group-hover/topic:opacity-0 group-has-[button:focus-visible]/topic:opacity-0",
            menu && "opacity-0",
            SIDE_NAV_FOLDS,
          )}
        >
          {count}
        </span>
      </Link>

      {topic && (
        <Popover
          open={menu}
          onOpenChange={(open) => {
            setMenu(open);
            if (!open) {
              setName(topic.name);
              setError(null);
            }
          }}
        >
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={`Edit topic ${topic.name}`}
              className={cn(
                "absolute top-1/2 right-1.5 flex size-5 -translate-y-1/2 cursor-pointer items-center justify-center rounded text-muted-foreground outline-none",
                "opacity-0 group-hover/topic:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100",
                "hover:bg-sidebar-item-active hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
              )}
            >
              <DotsThree size={16} weight="bold" />
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-60 p-3">
            <Input
              autoFocus
              value={name}
              aria-label="Topic name"
              aria-invalid={error != null}
              onChange={(e) => {
                setName(e.target.value);
                setError(null);
              }}
              onBlur={() => void rename()}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void rename();
                }
              }}
              className="h-8 rounded-lg text-[13px]"
            />
            {error && <p className="mt-1.5 px-0.5 text-[11px] text-destructive">{error}</p>}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setMenu(false);
                setConfirming(true);
              }}
              className="mt-2 -mb-1 w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
            >
              <Trash size={13} />
              Delete topic
            </Button>
          </PopoverContent>
        </Popover>
      )}

      {topic && (
        <ConfirmDialog
          open={confirming}
          onCancel={() => setConfirming(false)}
          onConfirm={() => void remove()}
          busy={busy}
          title={`Delete “${topic.name}”?`}
          description="Entries keep their other topics; any left with none move to Unsorted. Nothing else is deleted."
          confirmLabel="Delete topic"
          busyLabel="Deleting…"
        />
      )}
    </div>
  );
}
