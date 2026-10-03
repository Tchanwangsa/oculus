import { useEffect, useState } from "react";
import { Book, type Icon as PhosphorIcon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { courseColor, iconKey, useSubjectIconStore } from "@/stores/subjectIconStore";

let catalogue: Map<string, PhosphorIcon> | null = null;
let loading: Promise<Map<string, PhosphorIcon>> | null = null;

/** The full Phosphor catalogue, loaded once and cached for every glyph. Only a
 *  stored custom icon or an opened picker asks for it. */
export function loadIconCatalogue(): Promise<Map<string, PhosphorIcon>> {
  loading ??= import("@/components/subjects/iconCatalog").then(({ ICON_BY_NAME }) => {
    catalogue = ICON_BY_NAME;
    return ICON_BY_NAME;
  }).catch((error) => {
    loading = null;
    throw error;
  });
  return loading;
}

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
  const name = pref?.icon;
  const [, setLoaded] = useState(catalogue != null);
  useEffect(() => {
    if (!name || catalogue) return;
    let live = true;
    loadIconCatalogue().then(() => live && setLoaded(true)).catch(() => {});
    return () => { live = false; };
  }, [name]);
  const Icon = (name && catalogue?.get(name)) || Book;

  return (
    <span
      className={cn("flex items-center justify-center shrink-0", className)}
      style={{ width: size, height: size }}
    >
      <Icon size={size} weight="fill" color={color} />
    </span>
  );
}
