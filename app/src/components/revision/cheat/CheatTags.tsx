import { NameTags } from "@/components/projects/tags/NameTags";
import { useCheatTags } from "@/hooks/data/useCheatTags";

/**
 * An entry's tags as the task tag chips over the cheat sheet registry; with
 * `onChange` they are editable (see `NameTags`).
 */
export function CheatTags({
  value,
  onChange,
  disabled,
}: {
  value: string[];
  onChange?: (names: string[]) => void;
  disabled?: boolean;
}) {
  return <NameTags registry={useCheatTags()} value={value} onChange={onChange} disabled={disabled} />;
}
