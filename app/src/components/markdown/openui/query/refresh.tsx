import { useEffect } from "react";
import { useTriggerAction, type ParseResult, type ActionPlan } from "@openuidev/react-lang";
import { queryTools } from "./index";
import type { QueryTool } from "./shared";

type Queries = ParseResult["queryStatements"];

/** Invalidate through the runtime's @Run path, preserving the store and mounted controls. */
export function subscribeQueryRefresh(target: EventTarget, queries: Queries, run: (plan: ActionPlan) => void, tools: Record<string, QueryTool> = queryTools) {
  const byEvent = new Map<string, string[]>();
  for (const query of queries) {
    const name = query.toolAST?.k === "Str" ? query.toolAST.v : null;
    const events = name ? tools[name]?.events ?? [] : [...new Set(Object.values(tools).flatMap(t => t.events))];
    for (const event of events) byEvent.set(event, [...(byEvent.get(event) ?? []), query.statementId]);
  }
  const listeners = [...byEvent].map(([name, ids]) => {
    const listener = () => run({ steps: ids.map(statementId => ({ type: "run", refType: "query", statementId })) });
    target.addEventListener(name, listener);
    return () => target.removeEventListener(name, listener);
  });
  return () => listeners.forEach(remove => remove());
}

export function QueryRefresh({ queries, isStreaming }: { queries: Queries; isStreaming: boolean }) {
  const trigger = useTriggerAction();
  useEffect(() => {
    if (isStreaming) return;
    return subscribeQueryRefresh(window, queries, plan => { void trigger("", undefined, plan); });
  }, [queries, isStreaming, trigger]);
  return null;
}
