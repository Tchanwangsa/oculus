import { useMemo } from "react";
import { CodeText } from "@/components/markdown/MdComponents";
import { diffLines, type DiffLine } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { FoldBlock } from "./FoldBlock";

const TONE: Record<DiffLine["tone"], string> = {
  add: "bg-success/10 text-success",
  remove: "bg-destructive/10 text-destructive",
  hunk: "text-muted-foreground",
  context: "text-foreground/80",
};

/** A unified diff, its added and removed lines tinted, folded past its
 *  height limit (`FoldBlock`). */
export function DiffBlock({ diff }: { diff: string }) {
  const lines = useMemo(() => diffLines(diff), [diff]);
  return (
    <FoldBlock text={diff}>
      <CodeText className="text-[11.5px]">
        {lines.map((line, i) => (
          <span key={i} className={cn("block px-2.5", TONE[line.tone])}>
            {line.text || " "}
          </span>
        ))}
      </CodeText>
    </FoldBlock>
  );
}
