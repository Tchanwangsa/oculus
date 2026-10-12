import { Note, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { SubLabel } from "@/components/studio/panel/fields";
import { ColorPopover, ColorSwatch } from "@/components/ui/color/ColorField";
import { parseHex } from "@/lib/ui/color";
import { colorGroups, sectionKeys, setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";
import { cn } from "@/lib/utils";

/**
 * Every `color.*` token, grouped by what it colours (surfaces, text, accent,
 * ink, status): two tiles to a line, each a swatch, the name and the hex,
 * and the whole tile opens the colour control.
 */
export function ColorTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const groups = colorGroups(sectionKeys(tokens, "color"));
  return (
    <>
      {groups.map((group) => (
        <div key={group.id}>
          <SubLabel>{group.title}</SubLabel>
          <div className="grid grid-cols-2 gap-x-1 gap-y-0.5">
            {group.keys.map((key, i) => {
              const value = tokenAt(tokens, ["color", key]);
              const hex = typeof value === "string" ? value : "";
              const ok = parseHex(hex) != null;
              return (
                <ColorPopover
                  key={key}
                  label={tokenLabel(key)}
                  value={hex}
                  align={i % 2 === 0 ? "start" : "end"}
                  onChange={(next) => onChange(setToken(tokens, ["color", key], next))}
                  className="group flex min-w-0 items-center gap-2 rounded-lg px-1.5 py-1 transition-colors hover:bg-accent"
                >
                  <ColorSwatch value={hex} className="size-6" />
                  <span className="min-w-0">
                    <span className="block truncate text-[12.5px] leading-tight text-foreground">{tokenLabel(key)}</span>
                    <span
                      className={cn(
                        "block truncate text-[10.5px] uppercase leading-tight tabular-nums",
                        ok ? "text-muted-foreground" : "text-destructive",
                      )}
                    >
                      {hex || "none"}
                    </span>
                  </span>
                </ColorPopover>
              );
            })}
          </div>
        </div>
      ))}
      <Note>bg is the canvas under every frame; text colours any Text that sets none.</Note>
    </>
  );
}
