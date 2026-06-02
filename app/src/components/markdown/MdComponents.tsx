import type { Components } from "react-markdown";

export const MD_COMPONENTS: Components = {
  h1: (p: any) => (
    <h1 className="text-2xl font-bold text-foreground mt-6 mb-3 first:mt-0" {...p} />
  ),
  h2: (p: any) => (
    <h2 className="text-xl font-semibold text-foreground mt-6 mb-2.5 pb-1.5 border-b border-border" {...p} />
  ),
  h3: (p: any) => (
    <h3 className="text-base font-semibold text-foreground mt-5 mb-2" {...p} />
  ),
  h4: (p: any) => (
    <h4 className="text-sm font-semibold text-foreground mt-4 mb-2" {...p} />
  ),
  p: (p: any) => (
    <p className="text-sm text-foreground/90 leading-relaxed my-3" {...p} />
  ),
  a: (p: any) => (
    <a className="text-primary hover:underline" target="_blank" rel="noreferrer" {...p} />
  ),
  ul: (p: any) => (
    <ul className="list-disc pl-5 my-3 space-y-1 text-sm text-foreground/90" {...p} />
  ),
  ol: (p: any) => (
    <ol className="list-decimal pl-5 my-3 space-y-1 text-sm text-foreground/90" {...p} />
  ),
  li: (p: any) => <li className="leading-relaxed" {...p} />,
  blockquote: (p: any) => (
    <blockquote className="border-l-2 border-primary/40 pl-4 my-3 text-sm text-muted-foreground italic" {...p} />
  ),
  code: ({ className, children, ...p }: any) => {
    const isBlock = /language-/.test(className ?? "") || String(children).includes("\n");
    return isBlock ? (
      <code className="block p-3 rounded-lg bg-surface-raised text-[13px] font-mono text-foreground overflow-x-auto" {...p}>
        {children}
      </code>
    ) : (
      <code className="px-1.5 py-0.5 rounded bg-surface-raised text-[13px] font-mono text-foreground" {...p}>
        {children}
      </code>
    );
  },
  pre: (p: any) => <pre className="my-3" {...p} />,
  hr: (p: any) => <hr className="my-5 border-border" {...p} />,
  img: (p: any) => (
    <img className="max-w-full rounded-lg my-3 border border-border" {...p} />
  ),
  table: (p: any) => (
    <div className="overflow-x-auto my-3">
      <table className="w-full text-sm border-collapse" {...p} />
    </div>
  ),
  th: (p: any) => (
    <th className="border border-border px-3 py-1.5 bg-surface-raised text-left font-semibold text-xs" {...p} />
  ),
  td: (p: any) => (
    <td className="border border-border px-3 py-1.5 text-foreground/90" {...p} />
  ),
};
