import { ColorTokens } from "@/components/studio/themes/inspector/ColorTokens";
import { DefaultScope } from "@/components/studio/themes/inspector/DefaultScope";
import { MotionTokens } from "@/components/studio/themes/inspector/MotionTokens";
import { CornerTokens, NarrationTokens } from "@/components/studio/themes/inspector/ShapeTokens";
import { TypeTokens } from "@/components/studio/themes/inspector/TypeTokens";
import type { ComponentLevel } from "@/lib/studio/components";
import type { Tokens } from "@/lib/studio/themeDraft";
import type { ThemeDefaults, ThemeLevel } from "@/lib/studio/themes";

/**
 * The theme's tokens as a form: which libraries it is the default of, then
 * colours, fonts, type sizes, corners, motion and narration. Every edit hands
 * back the whole draft, which the stage draws at once; nothing is written
 * until Save.
 */
export function ThemeInspector({
  name,
  level,
  subject,
  tokens,
  onChange,
  defaults,
  defaultsError,
  onDefault,
}: {
  name: string;
  level: ThemeLevel;
  subject: string | null;
  /** The draft, or null while the text is loading or is not JSON. */
  tokens: Tokens | null;
  onChange: (next: Tokens) => void;
  defaults: ThemeDefaults | null;
  defaultsError: string | null;
  onDefault: (level: ComponentLevel, name: string | null) => void;
}) {
  return (
    <div className="space-y-5">
      <DefaultScope name={name} level={level} subject={subject} defaults={defaults} error={defaultsError} onSet={onDefault} />
      {tokens ? (
        <>
          <ColorTokens tokens={tokens} onChange={onChange} />
          <TypeTokens tokens={tokens} onChange={onChange} />
          <CornerTokens tokens={tokens} onChange={onChange} />
          <MotionTokens tokens={tokens} onChange={onChange} />
          <NarrationTokens tokens={tokens} onChange={onChange} />
        </>
      ) : (
        <p className="text-[12.5px] leading-snug text-muted-foreground">
          The tokens show once the theme file reads as JSON. Ask for a change to fix it.
        </p>
      )}
    </div>
  );
}
