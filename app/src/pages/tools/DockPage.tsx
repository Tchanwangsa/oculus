import { useNavigate, useParams } from "react-router-dom";
import { ArrowClockwise, ChatsCircle } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { dock, dockServiceById, type DockService, type DockServiceId } from "@/lib/dock";
import { useDockStore } from "@/stores/shell/dockStore";
import { useDockPrefsStore } from "@/stores/shell/dockPrefsStore";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { Button } from "@/components/ui/button";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { DockHeader } from "./dock/DockHeader";
import { Inbox } from "./dock/Inbox";
import { useDockViewport } from "./dock/useDockViewport";

/**
 * Messaging: a header of tabs (the Inbox, then one per enabled service) over
 * the Inbox on bare `/dock`, or over an empty slot that Rust places the
 * service's native WKWebView under (`app/src-tauri/src/shell/dock/`), shown
 * through a hole the app cuts there (`lib/shell/nativeHoles.ts`). The header
 * also keeps the card's rounded top corners clear, since the hole rounds only
 * its bottom corners. Pages outlive this route; leaving a service,
 * for the Inbox included, unmounts its slot and parks it.
 */
export default function DockPage() {
  const params = useParams();
  const loaded = useDockPrefsStore((s) => s.loaded);
  const enabled = useDockPrefsStore((s) => s.enabled);

  if (!params.service) {
    if (!loaded) return <LoadingFill />;
    if (enabled.length === 0) return <DockOff service={null} />;
    return <InboxView enabled={enabled} />;
  }

  const service = dockServiceById(params.service);
  if (!service || (loaded && !enabled.includes(service.id))) {
    return <DockOff service={service} />;
  }
  return <DockView service={service} enabled={enabled} />;
}

function InboxView({ enabled }: { enabled: DockServiceId[] }) {
  const empty = useDockStore((s) => s.inbox.length === 0);
  const clear = useDockStore((s) => s.clearInbox);

  return (
    <div className="flex h-full flex-col">
      <DockHeader
        current={null}
        enabled={enabled}
        actions={
          <Button
            variant="ghost"
            size="xs"
            disabled={empty}
            onClick={() =>
              clear().catch((e) => console.error("[oculus] dock: could not clear the Inbox", e))
            }
            className="text-muted-foreground hover:text-foreground"
          >
            Clear
          </Button>
        }
      />
      <Inbox enabled={enabled} />
    </div>
  );
}

function DockView({ service, enabled }: { service: DockService; enabled: DockServiceId[] }) {
  const page = useDockStore((s) => s.pages.find((p) => p.id === service.id));

  return (
    <div className="flex h-full flex-col">
      <DockHeader
        current={service.id}
        enabled={enabled}
        actions={
          <button
            onClick={() => dock.reload(service.id).catch(() => {})}
            disabled={!page}
            aria-label={`Reload ${service.label}`}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent"
          >
            <ArrowClockwise size={15} className={cn(page?.loading && "animate-spin")} />
          </button>
        }
      />
      {/* The slot mounts once Rust has the page, so its first place lands. */}
      {page ? <DockSlot key={service.id} service={service.id} /> : <LoadingFill />}
    </div>
  );
}

function DockOff({ service }: { service: DockService | null }) {
  const navigate = useNavigate();
  const Icon = service?.icon ?? ChatsCircle;
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2">
      <Icon size={24} className="text-muted-foreground/40" />
      <p className="text-sm text-muted-foreground">
        {service ? `${service.label} is off.` : "No messaging service is on."}
      </p>
      <Button
        variant="link"
        className="h-auto p-0 text-xs font-normal"
        onClick={() => navigate("/settings/messaging")}
      >
        Settings → Messaging
      </Button>
    </div>
  );
}

function DockSlot({ service }: { service: DockServiceId }) {
  const slotRef = useDockViewport(service, useTabId(), useTabActive());
  return <div ref={slotRef} className="min-h-0 flex-1" />;
}
