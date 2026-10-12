import { useRef } from "react";
import { useNavigate } from "react-router-dom";
import { ChatsCircle, Tray } from "@phosphor-icons/react";
import { dockPath, dockServiceById, type DockServiceId, type InboxItem } from "@/lib/dock";
import { fmtAgo } from "@/lib/format/format";
import { useDockStore } from "@/stores/shell/dockStore";
import { useNow } from "@/hooks/ui/useNow";
import { useScrollFade } from "@/hooks/ui/useScrollFade";

/**
 * Messaging's first tab: every arrival Rust caught while Oculus was open,
 * newest first. A row opens its service, never the chat itself, so reading
 * the Inbox sends no read receipt. Nothing native sits over it.
 */
export function Inbox({ enabled }: { enabled: readonly DockServiceId[] }) {
  const items = useDockStore((s) => s.inbox);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", items.length > 0);
  // Re-render each minute so "5m ago" keeps up.
  useNow();

  if (items.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
        <Tray size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">
          New messages show up here while Oculus is open.
        </p>
        {enabled.includes("discord") && (
          <p className="text-xs text-muted-foreground/70">
            Discord sends them only with its own Enable Desktop Notifications setting on.
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1">
      <div ref={scrollRef} className="page-scroll">
        <ul className="mx-auto w-full max-w-2xl px-3 py-2">
          {items.map((item) => (
            <InboxRow key={item.id} item={item} />
          ))}
        </ul>
      </div>
    </div>
  );
}

function InboxRow({ item }: { item: InboxItem }) {
  const navigate = useNavigate();
  const service = dockServiceById(item.service);
  const Icon = service?.icon ?? ChatsCircle;

  return (
    <li>
      <button
        type="button"
        disabled={!service}
        onClick={() => service && navigate(dockPath(service.id))}
        className="flex w-full cursor-pointer items-start gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-sidebar-item-hover disabled:cursor-default disabled:hover:bg-transparent"
      >
        <Icon
          size={16}
          aria-label={service?.label ?? item.service}
          className="mt-0.5 shrink-0 text-muted-foreground"
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-medium text-foreground">{item.sender}</p>
          <p className="line-clamp-2 break-words text-[12.5px] leading-snug text-muted-foreground">
            {item.preview}
          </p>
        </div>
        <span className="shrink-0 pt-px text-[11.5px] text-muted-foreground tabular-nums">
          {fmtAgo(item.at)}
        </span>
      </button>
    </li>
  );
}
