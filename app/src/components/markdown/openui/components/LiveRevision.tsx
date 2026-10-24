import { Link } from "react-router-dom";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import {
  getNotes,
  getItems,
  REVISION_UPDATED_EVENT,
  type RevisionScope,
} from "@/lib/revision";
import { getSubjects } from "@/lib/db";
import { Frame, Rich } from "./Frame";
import { useLiveData } from "./useLiveData";
export const CheatNote = defineComponent({
  name: "CheatNote",
  description: "A live cheat sheet entry by id.",
  props: z.object({ id: z.number().int().positive() }),
  component: function CheatNoteView({ props }) {
    const data = useLiveData(`cheat:${props.id}`, getNotes, [
      REVISION_UPDATED_EVENT,
    ]);
    const note = data.value?.find((n) => n.id === props.id);
    return (
      <Frame title={note?.title ?? "Cheat sheet"}>
        {note ? (
          <>
            <Rich text={note.body} />
            <Link
              to={`/revision?subject=all#cheat-${note.id}`}
              className="mt-2 block text-sm text-brand"
            >
              Open entry
            </Link>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">
            {data.error ??
              (data.value
                ? "This entry is no longer available."
                : "Loading entry…")}
          </p>
        )}
      </Frame>
    );
  },
});
export const ReviewDue = defineComponent({
  name: "ReviewDue",
  description: "Due review count for a subject code or id, null for all.",
  props: z.object({ subject: z.union([z.string(), z.number()]).nullish() }),
  component: function ReviewDueView({ props }) {
    const data = useLiveData(
      `due:${props.subject}`,
      async () => {
        let scope: RevisionScope = "all";
        if (typeof props.subject === "number") scope = props.subject;
        else if (props.subject) {
          if (props.subject === "general") scope = "general";
          else {
            const subjects = await getSubjects();
            const subject = subjects.find((s) => s.code === props.subject);
            if (!subject) throw new Error("Unknown subject");
            scope = subject.id;
          }
        }
        return getItems(scope);
      },
      [REVISION_UPDATED_EVENT],
    );
    const due = data.value?.filter(
      (i) => i.status === "active" && new Date(i.due).getTime() <= Date.now(),
    ).length;
    return (
      <Frame title="Review">
        <Link
          to={`/revision/review?subject=${encodeURIComponent(String(props.subject ?? "all"))}`}
          className="text-sm text-brand"
        >
          {due == null
            ? (data.error ?? "Loading review queue…")
            : `${due} questions due`}
        </Link>
      </Frame>
    );
  },
});
