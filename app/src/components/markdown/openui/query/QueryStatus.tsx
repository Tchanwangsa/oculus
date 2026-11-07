import { Renderer } from "@openuidev/react-lang";

export function QueryStatus() {
  return <>
    <Renderer.QueryLoading><p role="status" className="text-shimmer my-2 text-[13px]">Loading data…</p></Renderer.QueryLoading>
    <Renderer.QueryError>
      <div role="alert" className="my-3 flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3 text-[13px]">
        <span className="min-w-0 flex-1 text-muted-foreground">Couldn't load this data.</span>
        <Renderer.Retry className="cursor-pointer text-brand hover:underline disabled:cursor-default disabled:opacity-50">Retry</Renderer.Retry>
      </div>
    </Renderer.QueryError>
  </>;
}
