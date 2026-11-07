/** The title a widget names on its first line (`// title: …`), or "Widget". */
export function widgetTitle(source: string): string {
  const first = source.replace(/^\s*\n/, "").split("\n", 1)[0] ?? "";
  const title = /^\s*\/\/\s*title\s*:(.*)$/i.exec(first)?.[1]?.trim();
  return title || "Widget";
}
