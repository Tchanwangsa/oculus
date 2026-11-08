import { createContext, useContext } from "react";

/**
 * Set by the reply that is still streaming, when it streams as the row it
 * will become (`Timeline`'s streaming reply): its thread and run. A widget
 * fence under it builds its frame now, and the row keeps that frame when
 * the reply commits (docs/widgets.md#a-widget-draws-while-it-streams).
 * Null everywhere else, including `LiveTail`.
 */
export const WidgetStream = createContext<{ thread: number; run: number } | null>(null);
export const useWidgetStream = () => useContext(WidgetStream);
