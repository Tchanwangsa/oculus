import { WidgetType, type EditorView } from "@codemirror/view";

import { parseProperties, type PropertyValue } from "../../syntax/frontmatter";

/** Digits-led values (versions, dates, counts) hold a column. */
const NUMERIC = /^[-+]?\d[\d.,:_/-]*$/;

function propertyValue(value: PropertyValue): HTMLElement {
  const dom = document.createElement("div");
  dom.className = "cm-prop-value";
  if (value.kind === "list") {
    for (const item of value.items) {
      const chip = document.createElement("span");
      chip.className = "cm-prop-chip";
      chip.textContent = item;
      dom.appendChild(chip);
    }
  } else {
    dom.textContent = value.text;
    if (value.kind === "text" && NUMERIC.test(value.text)) dom.classList.add("cm-prop-number");
  }
  return dom;
}

/** YAML frontmatter as a properties card. Pressing a row reveals the source
 *  with the caret on that row's line. */
export class PropertiesWidget extends WidgetType {
  constructor(readonly source: string) {
    super();
  }

  eq(other: PropertiesWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = "cm-props";
    const card = document.createElement("div");
    card.className = "cm-props-card";
    const label = document.createElement("div");
    label.className = "cm-props-label";
    label.textContent = "Properties";
    const grid = document.createElement("div");
    grid.className = "cm-props-grid";
    for (const prop of parseProperties(this.source)) {
      const at = String(prop.at);
      if (prop.key == null) {
        const raw = document.createElement("div");
        raw.className = "cm-prop-raw";
        raw.textContent = prop.value.kind === "list" ? prop.value.items.join(", ") : prop.value.text;
        raw.dataset.at = at;
        grid.appendChild(raw);
        continue;
      }
      const key = document.createElement("div");
      key.className = "cm-prop-key";
      key.textContent = prop.key;
      key.title = prop.key;
      key.dataset.at = at;
      const value = propertyValue(prop.value);
      value.dataset.at = at;
      grid.append(key, value);
    }
    card.append(label, grid);
    dom.appendChild(card);

    dom.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      const row = (e.target as Element).closest<HTMLElement>("[data-at]");
      // Off a row, the first line inside the fences.
      const offset = row ? Number(row.dataset.at) : this.source.indexOf("\n") + 1;
      const at = view.posAtDOM(dom) + offset;
      view.dispatch({ selection: { anchor: Math.min(at, view.state.doc.length) } });
      view.focus();
    });
    return dom;
  }

  get estimatedHeight() {
    return 40 + Math.max(0, this.source.split("\n").length - 2) * 24;
  }
}
