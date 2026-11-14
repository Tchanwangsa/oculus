import { Lightning } from "@phosphor-icons/react";

/**
 * A picked skill inside the composer's sentence: atomic to the caret, and
 * `data-skill` is read back into the message as `/name`. `display: inline`
 * and no vertical padding, so it never pushes the line apart.
 */
export function SkillChip({ name }: { name: string }) {
  return (
    <span
      contentEditable={false}
      data-skill={name}
      className="mx-px inline whitespace-nowrap rounded-sm bg-brand/10 px-1 text-brand"
    >
      <Lightning size="1.05em" weight="fill" aria-hidden className="mr-[0.2em] inline align-[-0.15em]" />
      {name}
    </span>
  );
}
