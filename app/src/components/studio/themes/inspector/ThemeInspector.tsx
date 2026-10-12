import type { ReactNode } from "react";

import { ColorTokens } from "@/components/studio/themes/inspector/ColorTokens";
import { DefaultScope } from "@/components/studio/themes/inspector/DefaultScope";
import { MotionTokens } from "@/components/studio/themes/inspector/MotionTokens";
import { NarrationTokens } from "@/components/studio/themes/inspector/NarrationTokens";
import { ShapeTokens } from "@/components/studio/themes/inspector/ShapeTokens";
import { TransitionTokens } from "@/components/studio/themes/inspector/TransitionTokens";
import { TypeTokens } from "@/components/studio/themes/inspector/TypeTokens";
import { CollapsibleSection } from "@/components/ui/CollapsibleSection";
import { useInspectorSections } from "@/hooks/studio/useInspectorSections";
import { displayCode } from "@/lib/format/format";
import type { ComponentLevel } from "@/lib/studio/components";
import { sectionKeys, tokenAt, type Tokens } from "@/lib/studio/themeDraft";
import type { ThemeDefaults, ThemeLevel } from "@/lib/studio/themes";

/** Folded until opened: the sections a theme is rarely changed in. */
const START_FOLDED = ["type", "shape", "motion", "transition", "narration"];

type Sections = ReturnType<typeof useInspectorSections>;

/**
 * The theme's tokens as a form in sections that fold and are remembered:
 * which libraries it is the default of, colours, type, shape, motion, the
 * scene transition and narration. Every edit hands back the whole draft,
 * which the stage draws at once; nothing is written until Save. A section
 * whose tokens the theme lacks is left out, except the ones that add a
 * missing token when filled in.
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
  const sections = useInspectorSections("oculus-studio-themes-sections", START_FOLDED);
  const defaultFor = [
    defaults?.global === name ? "Global" : null,
    subject && defaults?.subject === name ? displayCode(subject) : null,
  ].filter((s) => s != null);

  const colours = sectionKeys(tokens, "color").length;
  const sizes = sectionKeys(tokens, "type").filter((k) => typeof tokenAt(tokens, ["type", k, "fontSize"]) === "number");
  const radii = sectionKeys(tokens, "radius").length;
  const motion =
    sectionKeys(tokens, "motion.duration").length +
    sectionKeys(tokens, "motion.easing").length +
    sectionKeys(tokens, "motion.spring").length;
  const kind = tokenAt(tokens, ["motion", "transition", "kind"]);

  return (
    <div>
      <Part id="default" title="Default for" count={defaultFor.join(", ")} sections={sections}>
        <DefaultScope name={name} level={level} subject={subject} defaults={defaults} error={defaultsError} onSet={onDefault} />
      </Part>
      {!tokens ? (
        <p className="py-3 text-[12.5px] leading-snug text-muted-foreground">
          The tokens show once the theme file reads as JSON. Ask for a change to fix it.
        </p>
      ) : (
        <>
          {colours > 0 && (
            <Part id="colours" title="Colours" count={colours} sections={sections}>
              <ColorTokens tokens={tokens} onChange={onChange} />
            </Part>
          )}
          <Part id="type" title="Type" count={3 + sizes.length} sections={sections}>
            <TypeTokens tokens={tokens} sizes={sizes} onChange={onChange} />
          </Part>
          {radii > 0 && (
            <Part id="shape" title="Shape" count={radii} sections={sections}>
              <ShapeTokens tokens={tokens} onChange={onChange} />
            </Part>
          )}
          {motion > 0 && (
            <Part id="motion" title="Motion" count={motion} sections={sections}>
              <MotionTokens tokens={tokens} onChange={onChange} />
            </Part>
          )}
          <Part id="transition" title="Transition" count={typeof kind === "string" ? kind : null} sections={sections}>
            <TransitionTokens tokens={tokens} onChange={onChange} />
          </Part>
          <Part id="narration" title="Narration" count={3} sections={sections}>
            <NarrationTokens tokens={tokens} onChange={onChange} />
          </Part>
        </>
      )}
    </div>
  );
}

function Part({
  id,
  title,
  count,
  sections,
  children,
}: {
  id: string;
  title: string;
  count: ReactNode;
  sections: Sections;
  children: ReactNode;
}) {
  return (
    <CollapsibleSection
      title={title}
      count={count || null}
      open={sections.isOpen(id)}
      onOpenChange={(open) => sections.setOpen(id, open)}
    >
      {children}
    </CollapsibleSection>
  );
}
