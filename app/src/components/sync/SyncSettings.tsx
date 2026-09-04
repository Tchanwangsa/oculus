import { useEffect, useState } from "react";
import { GearSix } from "@phosphor-icons/react";
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
 * The gear next to "Sync now": which content categories a sync fetches.
 * Persisted in settings, read by `triggerSync` at the start of every run —
 * manual and scheduled alike.
 */
export function SyncSettings() {
  const [options, setOptions] = useState<SyncOptions>(DEFAULT_SYNC_OPTIONS);

  useEffect(() => {
    getSyncOptions().then(setOptions).catch(console.error);
  }, []);

  const toggle = (key: keyof SyncOptions, on: boolean) => {
    const next = { ...options, [key]: on };
    setOptions(next);
    setSyncOptions(next).catch(console.error);
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Sync settings"
          className="text-muted-foreground/60 hover:text-foreground"
        >
          <GearSix size={13} />
        </Button>
      </PopoverTrigger>

      <PopoverContent side="bottom" align="end" className="w-64 p-0">
        <p className="px-3 pt-2.5 pb-1.5 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">
          What to sync
        </p>
        <div className="px-1 pb-2">
          {OPTION_ROWS.map(({ key, label, hint }) => (
            <Label
              key={key}
              className="flex items-center gap-2.5 rounded-md px-2 py-1.5 cursor-pointer hover:bg-surface"
            >
              <Checkbox
                checked={options[key]}
                onCheckedChange={(v) => toggle(key, v === true)}
              />
              <span className="flex-1 min-w-0">
                <span className="block text-xs text-foreground">{label}</span>
                <span className="block text-[11px] font-normal text-muted-foreground">
                  {hint}
                </span>
              </span>
            </Label>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
