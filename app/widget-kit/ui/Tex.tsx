import { useEffect, useSyncExternalStore } from "react";
import { describe } from "../runtime/post";
import { loadTex, onTexReady, texEngine } from "../tex/load";
import { cx } from "./cx";

const load = () => void loadTex();

/**
 * Maths, written as TeX (`<Tex>{"\\frac{a}{b}"}</Tex>` or `tex="…"`),
 * through the app's own maths engine. `display` sets it on its own line,
 * centred. Shows the TeX, muted, until the engine is in.
 */
export function Tex({ children, tex, display = false }: { children?: string | number; tex?: string; display?: boolean }) {
  const loaded = useSyncExternalStore(onTexReady, texEngine, texEngine);
  useEffect(load, []);
  const source = tex ?? (children == null ? "" : String(children));
  const Tag = display ? "div" : "span";
  if (!loaded) return <Tag className={cx("o-tex o-tex-pending", display && "o-tex-display")}>{source}</Tag>;
  let html: string;
  try {
    html = loaded.renderTex(source, display);
  } catch (error) {
    return (
      <Tag className={cx("o-tex o-tex-pending", display && "o-tex-display")} title={describe(error)}>
        {source}
      </Tag>
    );
  }
  return <Tag className={cx("o-tex", display && "o-tex-display")} dangerouslySetInnerHTML={{ __html: html }} />;
}
