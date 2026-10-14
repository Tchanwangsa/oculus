import {
  BookOpen,
  CalendarBlank,
  CalendarPlus,
  ClockClockwise,
  Kanban,
  Tag,
  TextT,
  type Icon,
} from "@phosphor-icons/react";
import { PROPERTY_LABELS, type PropertyId } from "./settings";

/** The Name column, which is always shown and first, so it is not a
 *  {@link PropertyId}. */
export const NAME_PROPERTY = { label: "Name", icon: TextT } as const;

const ICONS: Record<PropertyId, Icon> = {
  tags: Tag,
  project: Kanban,
  subject: BookOpen,
  due: CalendarBlank,
  updated: ClockClockwise,
  created: CalendarPlus,
};

export function propertyIcon(id: PropertyId): Icon {
  return ICONS[id];
}

export function propertyLabel(id: PropertyId): string {
  return PROPERTY_LABELS[id];
}
