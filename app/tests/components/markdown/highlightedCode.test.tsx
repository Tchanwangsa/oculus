import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

import { codeLanguage } from "@/components/documents/editor/syntax/codeLanguages";
import { paint } from "@/components/markdown/HighlightedCode";

const PYTHON = 'def answer(k):\n    s_j = "DM" * j  # gem taken\n    return (f_set, 3)\n';

async function support(info: string, code = "") {
  const desc = codeLanguage(info, code)?.desc;
  return desc ? await desc.load() : undefined;
}

describe("HighlightedCode", () => {
  test("a tagged fence is coloured with the grammar's tokens, text intact", async () => {
    const html = renderToStaticMarkup(<>{paint(PYTHON, await support("python"))}</>);
    expect(html).toMatch(/<span class="[^"]+">def<\/span>/);
    expect(html).toMatch(/<span class="[^"]+">&quot;DM&quot;<\/span>/);
    expect(html.replace(/<[^>]+>/g, "").replaceAll("&quot;", '"')).toBe(PYTHON);
  });

  test("an untagged fence is guessed from the code", async () => {
    expect(codeLanguage("", PYTHON)?.label).toBe("Python");
    const html = renderToStaticMarkup(<>{paint(PYTHON, await support("", PYTHON))}</>);
    expect(html).toContain("<span");
  });

  test("code with no grammar stays plain", async () => {
    expect(paint("just words\n", await support("text"))).toBe("just words\n");
  });
});
