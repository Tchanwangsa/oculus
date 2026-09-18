import { useState } from "react";
import { Desktop, Moon, Sun } from "@phosphor-icons/react";

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { applyTheme, getStoredTheme, type Theme } from "@/lib/theme";

const OPTIONS: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Desktop },
];

/**
 * The theme switch. `applyTheme` is the only writer of both the `.dark` class
 * and the stored preference, so this holds no state beyond what it renders —
 * a remount reads the same answer back out of localStorage.
 */
export function AppearanceSection() {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  function choose(next: Theme) {
    setTheme(next);
    applyTheme(next);
  }

  return (
    <div className="flex items-center justify-between py-2">
      <div>
        <p className="text-xs text-foreground">Theme</p>
        <p className="text-xs text-muted-foreground mt-0.5">
          System follows macOS and changes with it while Oculus is open.
        </p>
      </div>

      <ToggleGroup
        type="single"
        size="sm"
        value={theme}
        onValueChange={(v) => v && choose(v as Theme)}
        className="bg-surface rounded-md p-0.5"
      >
        {OPTIONS.map(({ value, label, icon: Icon }) => (
          <ToggleGroupItem
            key={value}
            value={value}
            aria-label={label}
            className="h-6 gap-1.5 px-2.5 text-xs rounded-[5px] data-[state=on]:bg-card data-[state=on]:shadow-xs"
          >
            <Icon size={12} weight={theme === value ? "fill" : "regular"} />
            {label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </div>
  );
}
