import { Section } from "@/components/settings/shared/section";
import { CodeFontRow } from "./CodeFontRow";
import { CodeThemePicker } from "./CodeThemePicker";

/** Code colours per mode, side by side when the column is wide enough, and
 *  the code font. */
export function CodeSection() {
  return (
    <Section title="Code" description="How code blocks look, apart from the rest of the theme.">
      <div className="@container py-2">
        <div className="grid gap-4 @min-[40rem]:grid-cols-2">
          <CodeThemePicker mode="light" />
          <CodeThemePicker mode="dark" />
        </div>
      </div>
      <CodeFontRow />
    </Section>
  );
}
