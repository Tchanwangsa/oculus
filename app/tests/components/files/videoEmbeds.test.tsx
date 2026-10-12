import { afterAll, describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { tagSource, videoDisplayName } from "@/components/markdown/embed/card/tagSource";
import { canvasFileUrl } from "@/lib/files/canvasLinks";
import { webVideoUrl } from "@/stores/lectures/webVideoStore";

// The components' imports reach modules that listen on `window` and
// `document` at load; tests run without a DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const stubbed = (["window", "document"] as const).filter((k) => !(k in globals));
for (const k of stubbed) globals[k] = new EventTarget();
afterAll(() => {
  for (const k of stubbed) delete globals[k];
});
const { MD_COMPONENTS } = await import("@/components/markdown/MdComponents");

/** The library chain, with no components: the sanitized tree as React writes it. */
function sanitized(text: string) {
  return renderToStaticMarkup(<ReactMarkdown {...fileMarkdownPlugins(text)}>{text}</ReactMarkdown>);
}

/** The library chain with the app's components, outside any subject. */
function rendered(text: string) {
  return renderToStaticMarkup(
    <ReactMarkdown {...fileMarkdownPlugins(text)} components={MD_COMPONENTS}>
      {text}
    </ReactMarkdown>,
  );
}

describe("video tags in library markdown", () => {
  test("each tag keeps its attributes through sanitize", () => {
    const html = sanitized(
      [
        'Intro\n\n<echo-video link-id="abc-123" src="https://canvas.lms.unimelb.edu.au/courses/1/external_tools/retrieve?x=1" title="Week 2 (20:52)"></echo-video>',
        '<canvas-video file-id="26446254" title="Workshop5.mp4"></canvas-video>',
        '<web-video provider="youtube" video-id="ECZUvBvPZoM" title="A talk"></web-video>',
        '<web-video provider="vimeo" video-id="123456" hash="9f8e7d"></web-video>',
      ].join("\n\n"),
    );
    expect(html).toContain('<echo-video link-id="abc-123"');
    expect(html).toContain('title="Week 2 (20:52)"');
    expect(html).toContain('<canvas-video file-id="26446254" title="Workshop5.mp4">');
    expect(html).toContain('<web-video provider="youtube" video-id="ECZUvBvPZoM" title="A talk">');
    expect(html).toContain('<web-video provider="vimeo" video-id="123456" hash="9f8e7d">');
  });

  test("a tag inside a table cell survives too", () => {
    const html = sanitized('| a |\n| --- |\n| <canvas-video file-id="7" title="x.mp4"></canvas-video> |');
    expect(html).toContain('<td><canvas-video file-id="7" title="x.mp4"></canvas-video></td>');
  });

  test("scripts, iframes and unlisted attributes still go", () => {
    const html = sanitized(
      [
        "<script>alert(1)</script>",
        '<iframe src="https://www.youtube.com/embed/ECZUvBvPZoM"></iframe>',
        '<web-video provider="youtube" video-id="x" onclick="alert(1)" style="color:red"></web-video>',
      ].join("\n\n"),
    );
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<iframe");
    expect(html).not.toContain("onclick");
    expect(html).not.toContain("style=");
  });

  test("outside a subject, Echo360 and Canvas videos are links out named by their title", () => {
    const html = rendered(
      '<echo-video link-id="abc" src="https://canvas.example/retrieve" title="Week 2 Vid1"></echo-video>\n\n<canvas-video file-id="26446254" title="Workshop5TDDandRefactoring.mp4"></canvas-video>',
    );
    expect(html).toContain("Week 2 Vid1");
    expect(html).toContain("Workshop5TDDandRefactoring<");
    expect(html).not.toContain("Workshop5TDDandRefactoring.mp4<");
  });

  test("a web video renders its card with the tag's title before any metadata", () => {
    const html = rendered('<web-video provider="youtube" video-id="ECZUvBvPZoM" title="A talk"></web-video>');
    expect(html).toContain("A talk");
    expect(html).toContain("Play");
    expect(html).toContain('data-md="&lt;web-video provider=&quot;youtube&quot;');
    expect(rendered('<web-video provider="dailymotion" video-id="x"></web-video>')).not.toContain("Play");
  });
});

describe("task lists", () => {
  test("keep their indent, drop their bullets and keep a disabled box", () => {
    const html = rendered("- [ ] read the brief\n- [x] submit");
    expect(html).toMatch(/<ul class="[^"]*list-none[^"]*pl-6[^"]*contains-task-list/);
    expect(html).not.toContain("list-disc");
    expect(html).toContain("<li class=\"leading-relaxed task-list-item\"");
    expect(html).toMatch(/<input class="[^"]*-ml-5[^"]*" type="checkbox" disabled=""/);
  });

  test("a plain list is untouched", () => {
    expect(rendered("- a\n- b")).toContain('<ul class="list-disc pl-5');
  });
});

describe("card helpers", () => {
  test("tagSource escapes values and skips empty ones", () => {
    expect(tagSource("canvas-video", [["file-id", "1"], ["title", 'a "b" & c']])).toBe(
      '<canvas-video file-id="1" title="a &quot;b&quot; &amp; c">\n</canvas-video>',
    );
    expect(tagSource("canvas-video", [["file-id", "1"], ["title", undefined]])).toBe(
      '<canvas-video file-id="1">\n</canvas-video>',
    );
  });

  test("videoDisplayName strips a video extension only", () => {
    expect(videoDisplayName("Workshop5.mp4", "x")).toBe("Workshop5");
    expect(videoDisplayName("Week 2 (20:52)", "x")).toBe("Week 2 (20:52)");
    expect(videoDisplayName("  ", "Canvas video")).toBe("Canvas video");
    expect(videoDisplayName(undefined, "Canvas video")).toBe("Canvas video");
  });

  test("links out", () => {
    const canvas = "https://canvas.example.edu";
    expect(canvasFileUrl(canvas, "26446254", 197340)).toBe(`${canvas}/courses/197340/files/26446254`);
    expect(canvasFileUrl(canvas, "26446254")).toBe(`${canvas}/files/26446254`);
    expect(webVideoUrl("youtube", "ECZUvBvPZoM")).toBe("https://www.youtube.com/watch?v=ECZUvBvPZoM");
    expect(webVideoUrl("vimeo", "123456")).toBe("https://vimeo.com/123456");
  });
});
