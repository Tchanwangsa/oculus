import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Ask } from "./Ask";
import { Explorer } from "./Explorer";
import { Flashcards } from "./Flashcards";
import { PageRef } from "./PageRef";
import { Quiz } from "./Quiz";
import { Steps } from "./Steps";
import { Text } from "./Text";
import { WidgetIndex } from "../state";

/** The program's root: widgets top to bottom, with Ask buttons and PageRefs
 *  gathered onto one wrapping row so they read as a footer, not a list. */
export const Stack = defineComponent({
  name: "Stack",
  description: "The root: study widgets top to bottom. Ask and PageRef items line up in a row.",
  props: z.object({
    items: z.array(z.union([Flashcards.ref, Quiz.ref, Steps.ref, Explorer.ref, Text.ref, PageRef.ref, Ask.ref])),
  }),
  component: function StackView({ props, renderNode }) {
    const items = Array.isArray(props.items) ? props.items : [];
    const inline = (n: unknown) => {
      const t = (n as { typeName?: string } | null)?.typeName;
      return t === "Ask" || t === "PageRef";
    };
    const blocks = items.map((node, i) => !inline(node) && (
      <WidgetIndex.Provider key={i} value={i}>{renderNode(node)}</WidgetIndex.Provider>
    ));
    const chips = items.filter(inline);
    return (
      <div className="flex flex-col">
        {blocks}
        {chips.length > 0 && <div className="my-2 flex flex-wrap items-center gap-2">{renderNode(chips)}</div>}
      </div>
    );
  },
});
