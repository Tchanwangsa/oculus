import { useContext } from "react";
import { useStateField } from "@openuidev/react-lang";
import { WidgetIndex } from "./state";

/** The Stack position is stable even for inline widgets without a statement
 *  id, across remounts and different timeline windows. */
export function useWidgetProgress<T>(initial: T) {
  const index = useContext(WidgetIndex);
  return useStateField(`progress:${index}`, initial);
}
