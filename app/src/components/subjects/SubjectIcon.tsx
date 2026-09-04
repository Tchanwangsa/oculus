import { Book } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { ICON_BY_NAME } from "@/components/subjects/iconCatalog";
import { courseColor, iconKey, useSubjectIconStore } from "@/stores/subjectIconStore";

/**
 * A subject's identity glyph: the user's chosen icon + colour, defaulting to a
 * Book in the hashed course colour. Always occupies a `size`² box so swapping
 * icons never shifts the text next to it.
 */
export function SubjectIcon({
  code,
  size = 14,
  className,
}: {
  code: string;
  size?: number;
  className?: string;
}) {
  const pref = useSubjectIconStore((s) => s.prefs[iconKey(code)]);
  const color = pref?.color ?? courseColor(code);
  const Icon = (pref?.icon && ICON_BY_NAME.get(pref.icon)) || Book;

  return (
    <span
      className={cn("flex items-center justify-center shrink-0", className)}
      style={{ width: size, height: size }}
    >
      <Icon size={size} weight="fill" color={color} />
    </span>
  );
}
