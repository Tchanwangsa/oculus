import { defineComponent, useIsStreaming } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { MoreComing } from "../../Streaming";
import { formatNumber } from "../format";
import { Rich } from "../Frame";

/** Rows past this show as a count; a reply table is a summary, not a sheet. */
const MAX_ROWS = 50;

const cell = z.union([z.string(), z.number(), z.boolean()]).nullable();

/** Rows of cells under column headings; numbers right-aligned in tabular
 *  figures. It scrolls sideways rather than squeezing a wide table. */
export const Table = defineComponent({
  name: "Table",
  description: "Columns and rows of cells (strings, numbers, booleans or null), e.g. Table([\"Task\", \"Due\"], [[\"Report\", \"Fri\"]]). At most 50 rows show.",
  props: z.object({
    columns: z.array(z.string()),
    rows: z.array(z.array(cell)),
  }),
  component: function TableView({ props }) {
    const streaming = useIsStreaming();
    const columns = Array.isArray(props.columns) ? props.columns : [];
    const rows = (Array.isArray(props.rows) ? props.rows : []).filter(Array.isArray);
    const numeric = columns.map((_, j) => rows.length > 0 && rows.every((r) => r[j] == null || typeof r[j] === "number"));
    if (!columns.length && !rows.length) return streaming ? <MoreComing /> : null;
    return <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-[13px]">
        <thead><tr className="border-b border-border">
          {columns.map((c, j) => <th key={j} scope="col" className={`px-2 py-1.5 text-xs font-medium whitespace-nowrap text-muted-foreground first:pl-0 last:pr-0 ${numeric[j] ? "text-right" : "text-left"}`}><Rich text={c} /></th>)}
        </tr></thead>
        <tbody>{rows.slice(0, MAX_ROWS).map((r, i) => <tr key={i} className="openui-mark border-b border-border-subtle last:border-b-0">
          {columns.map((_, j) => {
            const v = r[j];
            return <td key={j} className={`px-2 py-1.5 align-top first:pl-0 last:pr-0 ${numeric[j] ? "text-right tabular-nums" : ""}`}>
              {typeof v === "number" ? formatNumber(v, undefined, 2) : typeof v === "boolean" ? (v ? "Yes" : "No") : v == null ? <span className="text-muted-foreground">—</span> : <Rich text={v} className="text-foreground" />}
            </td>;
          })}
        </tr>)}</tbody>
      </table>
      {rows.length > MAX_ROWS && <p className="mt-1 text-xs text-muted-foreground">and {rows.length - MAX_ROWS} more</p>}
      {streaming && !rows.length && <MoreComing />}
    </div>;
  },
});
