import { createContext, useContext } from "react";

export const WidgetRow = createContext<{ thread: number; row: number } | null>(null);
export const useWidgetRow = () => useContext(WidgetRow);

export const WidgetIndex = createContext(0);
export const WidgetFence = createContext(0);
