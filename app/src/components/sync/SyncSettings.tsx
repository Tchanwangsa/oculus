import { useEffect, useState } from "react";
import { SlidersHorizontal } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  getSyncOptions,
  setSyncOptions,
  DEFAULT_SYNC_OPTIONS,
  type SyncOptions,
} from "@/lib/db";

const OPTION_ROWS: Array<{ key: keyof SyncOptions; label: string; hint: string }> = [
  { key: "modules", label: "Modules", hint: "Pages and files" },
  { key: "assignments", label: "Assignments", hint: "Including quizzes" },
  { key: "announcements", label: "Announcements", hint: "Course announcements" },
  { key: "ed", label: "Ed Discussion", hint: "Threads and answers" },
  { key: "lectures", label: "Lectures", hint: "Echo360 list" },
  { key: "calendar", label: "Calendar", hint: "Class times and due dates" },
];

/**
 * The control next to "Sync now": which content categories a sync fetches.
 * Persisted in settings, read by `triggerSync` at the start of every run —
 * manual and scheduled alike. The trigger carries the enabled count so the
 * toolbar says how much a sync will pull without being opened.
 */
export function SyncSettings() {
  const [options, setOptions] = useState<SyncOptions>(DEFAULT_SYNC_OPTIONS);

  useEffect(() => {
    getSyncOptions().then(setOptions).catch(console.error);
  }, []);

  const save = (next: SyncOptions) => {
    setOptions(next);
    setSyncOptions(next).catch(console.error);
  };

  const toggle = (key: keyof SyncOptions, on: boolean) =>
    save({ ...options, [key]: on });

  const setAll = (on: boolean) =>
    save(
      OPTION_ROWS.reduce(
        (acc, { key }) => ({ ...acc, [key]: on }),
        { ...options },
      ),
    );

  const enabled = OPTION_ROWS.filter(({ key }) => options[key]).length;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-7 gap-1.5 px-3 text-xs font-normal text-foreground"
        >
          <SlidersHorizontal size={13} className="text-muted-foreground" />
          View settings
          <span className="text-muted-foreground/70 tabular-nums">({enabled})</span>
        </Button>
      </PopoverTrigger>

      <PopoverContent side="bottom" align="start" className="w-64 p-0">
        <p className="border-b border-border-subtle px-3 py-2.5 font-display text-[13px] font-semibold text-foreground">
          What to sync
        </p>

        <div className="p-1.5">
          {OPTION_ROWS.map(({ key, label, hint }) => (
            <Label
              key={key}
              className="flex items-center gap-2.5 rounded-lg px-2 py-1.5 cursor-pointer hover:bg-accent"
            >
              <Checkbox
                checked={options[key]}
                onCheckedChange={(v) => toggle(key, v === true)}
                className="size-3.5 [&_svg]:size-2.5"
              />
              <span className="flex-1 min-w-0">
                <span className="block text-xs text-foreground">{label}</span>
                <span className="block text-[10px] font-normal text-muted-foreground">
                  {hint}
                </span>
              </span>
            </Label>
          ))}
        </div>

        {/* Both actions always render — hiding one would resize the popover
            as you tick boxes. Disabled is the "nothing to do" state. */}
        <div className="flex items-center justify-between border-t border-border-subtle px-1.5 py-1.5">
          <Button
            variant="ghost"
            size="xs"
            onClick={() => setAll(true)}
            disabled={enabled === OPTION_ROWS.length}
            className="text-[11px] font-normal text-muted-foreground hover:text-foreground"
          >
            Select all
          </Button>
          <Button
            variant="ghost"
            size="xs"
            onClick={() => setAll(false)}
            disabled={enabled === 0}
            className="text-[11px] font-normal text-muted-foreground hover:text-foreground"
          >
            Clear all
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
