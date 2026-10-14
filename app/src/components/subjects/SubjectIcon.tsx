import { Book } from "@phosphor-icons/react";
import { CatalogueIcon } from "@/components/ui/icon/CatalogueIcon";
import { courseColor, iconKey, useSubjectIconStore } from "@/stores/planning/subjectIconStore";

/** A subject's stored identity, with a Book and course colour as defaults. */
export function SubjectIcon({ code, size = 14, className }: { code: string; size?: number; className?: string }) {
  const pref = useSubjectIconStore(s => s.prefs[iconKey(code)]);
  return <CatalogueIcon name={pref?.icon} fallback={Book} color={pref?.color ?? courseColor(code)} size={size} className={className} />;
}
