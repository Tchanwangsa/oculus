import { useEffect } from "react";
import { addLog, getAutomations, getPendingInboxItems } from "@/lib/db";
import {
  dueTriggers,
  fireTrigger,
  migrateAutomationGraphs,
  runEventAutomations,
} from "@/lib/automations";
import { resumePendingDigests } from "@/lib/digest";
import { useInboxStore } from "@/stores/inboxStore";
import { useSyncStore } from "@/stores/syncStore";

const CHECK_MS = 30_000;

/**
 * Drives schedule-triggered automations while the app is open, and hydrates
 * the Inbox once at startup.
 *
 * Checks every 30s and once at mount — the mount check is what catches a
 * daily graph missed while the app was closed. One trigger per tick; a
 * trigger that comes due mid-sync waits for a later tick, still due.
 *
 * Event-triggered graphs are not driven from here, with one exception:
 * `app-start` has no other home, so it is dispatched on mount. `sync-complete`
 * fires where its run id exists, in `useBackendEvents`.
 */
export function useAutomations() {
  useEffect(() => {
    let stopped = false;

    const tick = async () => {
      if (stopped || useSyncStore.getState().scraping) return;
      try {
        for (const a of await getAutomations()) {
          if (stopped) return;
          const trigger = dueTriggers(a)[0];
          if (!trigger) continue;
          await addLog(`Automation fired: ${a.name}`);
          await fireTrigger(a, trigger);
          return; // one firing per tick
        }
      } catch (e) {
        console.error("automation failed", e);
      }
    };

    // Graphs written against an older node/link shape are upgraded on parse;
    // this writes the upgrade back once, so what is stored is what runs.
    migrateAutomationGraphs()
      .catch((e) => addLog(`automation upgrade: ${e}`, "warning").catch(() => {}))
      .finally(() => {
        tick();
      });
    const timer = setInterval(tick, CHECK_MS);

    runEventAutomations("app-start", {}).catch((e) =>
      addLog(`automation: ${e}`, "warning").catch(() => {}),
    );

    // A digest whose summaries were cut short by a quit finishes here.
    useInboxStore
      .getState()
      .refresh()
      .then(() => getPendingInboxItems())
      .then((items) => (items.length ? resumePendingDigests(items) : undefined))
      .catch(() => {});

    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, []);
}
