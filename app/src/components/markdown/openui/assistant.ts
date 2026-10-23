import { createContext, useContext } from "react";

/**
 * How a widget's `@ToAssistant` reaches the thread: Timeline provides its
 * followUp action, including in the lecture dock and Studio. Elsewhere, and
 * while the thread is running, widgets hide their ask buttons.
 */
export const AssistantSend = createContext<((text: string) => void) | null>(null);

export const useAssistantSend = () => useContext(AssistantSend);
