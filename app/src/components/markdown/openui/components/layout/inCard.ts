import { createContext } from "react";

/** True inside a card (Panel, Explorer, Tabs, a grouped Stack run): children
 *  draw flat instead of opening a card of their own. */
export const InCard = createContext(false);
