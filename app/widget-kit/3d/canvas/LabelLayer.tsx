import { useEffect, useSyncExternalStore, type CSSProperties } from "react";
import { paint, type Colour } from "../../colors";
import { loadTex, onTexReady, texEngine } from "../../tex/load";
import type { Attach, LabelRegistry } from "./labels";

/** CSS px between a point and a label attached beside it. */
const GAP = 8;

const load = () => void loadTex();

/** The shift from the label's point to its box: centred, or beside it on
 *  the side `attach` names. */
function offset(attach: Attach | undefined): string {
  const x = attach?.includes("e") ? `${GAP}px` : attach?.includes("w") ? `calc(-100% - ${GAP}px)` : "-50%";
  const y = attach?.includes("n") ? `calc(-100% - ${GAP}px)` : attach?.includes("s") ? `${GAP}px` : "-50%";
  return `translate(${x}, ${y})`;
}

function Label({
  index,
  registry,
  source,
  attach,
  color,
}: {
  index: number;
  registry: LabelRegistry;
  source: string;
  attach?: Attach;
  color?: Colour;
}) {
  const engine = useSyncExternalStore(onTexReady, texEngine, texEngine);
  useEffect(load, []);
  let html: string | null = null;
  if (engine) {
    try {
      html = engine.renderTex(source, false);
    } catch {
      html = null;
    }
  }
  const box: CSSProperties = { transform: offset(attach), color: paint(color, "foreground") };
  return (
    // Hidden until the scene places it; `place` writes its position straight to the element.
    <div ref={(el) => registry.bind(index, el)} className="o-scene-label" style={{ visibility: "hidden" }}>
      <div className="o-scene-label-box" style={box}>
        {html == null ? (
          <span className="o-tex o-tex-pending">{source}</span>
        ) : (
          <span className="o-tex" dangerouslySetInnerHTML={{ __html: html }} />
        )}
      </div>
    </div>
  );
}

/** The canvas's labels as DOM over it, never taking the pointer. */
export function LabelLayer({ registry }: { registry: LabelRegistry }) {
  const labels = useSyncExternalStore(registry.subscribe, registry.snapshot, registry.snapshot);
  if (!labels.length) return null;
  return (
    <div className="o-scene-labels">
      {labels.map((label, i) => (
        // By position: a scene drawn again re-registers its labels in the same order.
        <Label key={i} index={i} registry={registry} source={label.source} attach={label.attach} color={label.color} />
      ))}
    </div>
  );
}
