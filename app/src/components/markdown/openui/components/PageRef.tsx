import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { CitationCode, CitationLink } from "@/components/markdown/Citation";
import { parseCitation } from "@/lib/citations";

/**
 * A page of a library file, drawn as the same chip a citation in prose gets
 * (`CitationLink`): it resolves a course-relative tail, opens the PDF at the
 * page in the side panel, and previews on hover. A path the citation grammar
 * can't read stays plain text.
 */
export const PageRef = defineComponent({
  name: "PageRef",
  description:
    "A link to a page of a library file, e.g. PageRef(\"courses/MAST10006/Lecture 4.pdf\", 12, \"Chain rule\"); same paths you cite in prose.",
  props: z.object({
    path: z.string(),
    page: z.number().optional(),
    label: z.string().optional(),
  }),
  component: ({ props }) => {
    const path = typeof props.path === "string" ? props.path : "";
    const page = typeof props.page === "number" && props.page > 0 ? Math.floor(props.page) : null;
    const label = typeof props.label === "string" && props.label ? props.label : null;
    const shape = parseCitation(page ? `${path}#page=${page}` : path);
    if (!shape) return <span className="text-sm text-muted-foreground">{label ?? path}</span>;
    // Unlabelled, it is the chip prose gets (file title and " · p. 12");
    // labelled, the label replaces the title.
    return (
      <span className="text-sm">
        {label ? (
          <CitationLink shape={shape}>{label}</CitationLink>
        ) : (
          <CitationCode shape={shape} code={<span className="text-muted-foreground">{path}</span>} />
        )}
      </span>
    );
  },
});
