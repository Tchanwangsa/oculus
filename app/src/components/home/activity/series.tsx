import { DotsThree, House } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { NO_SUBJECT_SERIES, OTHER_SUBJECTS, type TypeGroup } from "@/lib/activity/usage";
import { OTHER_FILL, TYPE_SERIES } from "@/components/home/activity/constants";
import { Swatch } from "@/components/home/activity/Swatch";
import type { Series } from "@/components/home/activity/types";

/** A type group's series: its icon in its colour stands in for the swatch. */
export function typeSeries(key: string): Series {
  const t = TYPE_SERIES[key as TypeGroup];
  return {
    key,
    label: t.label,
    color: t.color,
    legendMark: <t.icon weight="fill" className="size-3 shrink-0" style={{ color: t.color }} />,
    mark: <t.icon weight="fill" className="size-3.5" style={{ color: t.color }} />,
  };
}

/**
 * A subject's series: its code, in the calendar's colour for it, and its glyph
 * in the hover card. A subject with no calendar rows takes a chart colour by
 * id, which stays the same from day to day.
 */
export function subjectSeries(
  key: string,
  byId: Map<number, Subject>,
  colors: Map<number, string>,
): Series {
  if (key === NO_SUBJECT_SERIES || key === OTHER_SUBJECTS) {
    const none = key === NO_SUBJECT_SERIES;
    const Glyph = none ? House : DotsThree;
    // Folded subjects take a tint of the grey, so the two never read as one.
    const color = none ? OTHER_FILL : `color-mix(in srgb, ${OTHER_FILL} 50%, var(--color-card))`;
    return {
      key,
      // Time on pages outside any subject: Home, chat, the browser, planning.
      label: none ? "General" : "Other subjects",
      color,
      mark: (
        <>
          <Swatch color={color} />
          <Glyph className="size-3" />
        </>
      ),
    };
  }
  const id = Number(key);
  const subject = byId.get(id);
  const color = colors.get(id) ?? `var(--color-chart-${(id % 5) + 1})`;
  return {
    key,
    label: subject ? displayCode(subject.code) : "Unknown subject",
    color,
    mark: (
      <>
        <Swatch color={color} />
        {subject && <SubjectIcon code={subject.code} size={12} />}
      </>
    ),
  };
}
