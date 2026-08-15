// ============================================================================
// Oculus Canvas scraper agent.
// Injected into the authenticated Canvas WebView via win.eval(). Tokens __PORT__
// and __SUBJECTS__ are substituted by Rust (scrape_content) before eval.
//
// Strategy: scrape flat lists (pages, assignments, announcements, files) first,
// then modules LAST as a structural index that links to already-downloaded
// artifacts by Canvas id -- avoids double-downloading.
//
// All content is POSTed to the local tiny_http IPC server:
//   POST /scrape?course=&subject_id=&path=             body: markdown (text)
//   POST /scrape-binary?course=&subject_id=&path=&canvas_id=   body: bytes
//   POST /scrape-progress  {done,total,course,phase,label}   (JSON)
//   POST /scrape-log       {level,course,message}            (JSON)
//   POST /scrape-done      {count}                            (JSON)
// ============================================================================

(() => {
  const PORT = __PORT__;
  const SUBJECTS = __SUBJECTS__;
  const BASE = "http://127.0.0.1:" + PORT;
  const CANVAS_ORIGIN = "https://canvas.lms.unimelb.edu.au";

  // Canvas fetches go through the Rust cookie proxy. This worker WebView is NOT
  // logged in (it loads a blank local page); Rust replays the persisted session
  // cookie server-side. Accepts a relative path or an absolute Canvas URL (the
  // latter comes from Link-header pagination). The proxy forwards status, body
  // and the Link header, so callers use r.ok / r.status / r.headers.get("Link")
  // exactly as before.
  // Every await in this file ultimately bottoms out here, and a promise that
  // never settles has no error to catch — the per-course try/catch in the
  // orchestrator cannot see it. One such fetch stalls the whole sync silently
  // and permanently, so the timeout is what guarantees the loop always ends.
  const CFETCH_TIMEOUT_MS = 200_000; // > the proxy's own 180s ceiling
  const CFETCH_RETRIES = 2;

  const cfetch = async (p) => {
    const abs = /^https?:\/\//.test(p) ? p : CANVAS_ORIGIN + p;
    const target = BASE + "/canvas?url=" + encodeURIComponent(abs);

    let lastErr;
    for (let attempt = 0; attempt <= CFETCH_RETRIES; attempt++) {
      try {
        return await fetch(target, { signal: AbortSignal.timeout(CFETCH_TIMEOUT_MS) });
      } catch (e) {
        lastErr = e;
        // Back off before retrying — an immediate retry of a timed-out request
        // usually just times out again.
        if (attempt < CFETCH_RETRIES) {
          await new Promise((r) => setTimeout(r, 1000 * (attempt + 1)));
        }
      }
    }
    // Surface as a normal rejection so the orchestrator's catch logs it and
    // moves to the next subject, rather than hanging here forever.
    throw new Error("fetch failed after retries: " + abs + " (" + lastErr + ")");
  };

  // PDF-only file policy for now (user decision). Expand later.
  const ALLOWED_FILE_TYPES = ["application/pdf"];
  const MAX_FILE_BYTES = 100 * 1024 * 1024; // 100 MB safety cap

  // ---- IPC helpers ----------------------------------------------------------

  const q = (obj) =>
    Object.entries(obj)
      .map(([k, v]) => k + "=" + encodeURIComponent(v))
      .join("&");

  // Fix UTF-8 bytes misread as Latin-1 (common in pasted Canvas content).
  // Matches lead byte (C0-F7) + continuation bytes (80-BF) and tries UTF-8 decode.
  function fixMojibake(str) {
    return str.replace(/[À-÷][-¿]+/g, (m) => {
      try {
        const bytes = Uint8Array.from(m, c => c.charCodeAt(0));
        return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      } catch { return m; }
    });
  }

  const postText = (course, sid, path, md) =>
    fetch(BASE + "/scrape?" + q({ course, subject_id: sid, path }), {
      method: "POST",
      headers: { "Content-Type": "text/markdown" },
      body: fixMojibake(md),
    }).catch(() => {});

  const postBinary = (course, sid, path, canvasId, buf) =>
    fetch(BASE + "/scrape-binary?" + q({ course, subject_id: sid, path, canvas_id: canvasId }), {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: buf,
    }).catch(() => {});

  const postJson = (path, obj) =>
    fetch(BASE + path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(obj),
    }).catch(() => {});

  const progress = (p) => postJson("/scrape-progress", p);
  const logMsg = (level, course, message) => postJson("/scrape-log", { level, course, message });

  // ---- Canvas API with Link-header pagination -------------------------------

  async function fetchAll(url) {
    let out = [];
    let next = url;
    while (next) {
      const r = await cfetch(next);
      if (!r.ok) {
        if (r.status === 403 || r.status === 404) break; // locked / absent -- skip
        throw new Error("API " + r.status + " " + next);
      }
      const page = await r.json();
      out = out.concat(Array.isArray(page) ? page : [page]);
      next = null;
      const link = r.headers.get("Link");
      if (link) {
        const part = link.split(",").find((s) => s.includes('rel="next"'));
        const m = part && part.match(/<([^>]+)>/);
        if (m) next = m[1];
      }
    }
    return out;
  }

  const slug = (s) =>
    (s || "untitled")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || "untitled";

  // ---- HTML -> Markdown converter (DOMParser, no deps) ----------------------

  const SKIP = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "SVG", "PATH"]);
  const NBSP = String.fromCharCode(160);

  // Escape markdown special characters in plain text so they don't get
  // misinterpreted (e.g. trailing * becoming italic/bold markers).
  const escapeMd = (s) => s.replace(/([*_`[\]\\])/g, "\\$1");

  function inlineMd(node) {
    let out = "";
    node.childNodes.forEach((n) => {
      if (n.nodeType === 3) { out += escapeMd(n.textContent); return; }
      if (n.nodeType !== 1) return;
      const t = n.tagName;
      if (SKIP.has(t)) return;
      if (t === "A") out += "[" + inlineMd(n) + "](" + (n.getAttribute("href") || "") + ")";
      else if (t === "STRONG" || t === "B") {
        const inner = inlineMd(n).trim();
        if (!inner) { /* skip */ }
        // Pure footnote/marker content (escaped stars etc.) â€” bold wrapper creates
        // ambiguous markdown. Output as plain escaped text instead.
        else if (/^[\\*â€ â€¡Â§Â¶\d\s]+$/.test(inner)) out += inner;
        else out += "**" + inner + "**";
      }
      else if (t === "EM" || t === "I") {
        const inner = inlineMd(n).trim();
        if (!inner) { /* skip */ }
        else if (/^[\\*â€ â€¡Â§Â¶\d\s]+$/.test(inner)) out += inner;
        else out += "*" + inner + "*";
      }
      else if (t === "SUP") { out += escapeMd(n.textContent); }
      else if (t === "CODE") out += "`" + n.textContent + "`";
      else if (t === "BR") out += "  \n";
      else if (t === "IMG") out += "![" + (n.getAttribute("alt") || "") + "](" + (n.getAttribute("src") || "") + ")";
      else out += inlineMd(n);
    });
    return out;
  }

  function listMd(node, depth, ordered) {
    let out = "", i = 1;
    node.childNodes.forEach((n) => {
      if (n.nodeType !== 1 || n.tagName !== "LI") return;
      const marker = ordered ? i++ + "." : "-";
      const indent = "  ".repeat(depth);
      let inlineParts = "", nested = "";
      n.childNodes.forEach((c) => {
        if (c.nodeType === 1 && (c.tagName === "UL" || c.tagName === "OL")) {
          nested += listMd(c, depth + 1, c.tagName === "OL");
        } else if (c.nodeType === 1) inlineParts += inlineMd(c);
        else if (c.nodeType === 3) inlineParts += c.textContent;
      });
      out += indent + marker + " " + inlineParts.trim() + "\n";
      if (nested) out += nested;
    });
    return out;
  }

  // GFM table cells MUST be single-line. Flatten block content: each leaf block
  // (p/div/li/heading) becomes a segment joined by <br>; lists join with "; ".
  // Strips images / svg / screenreader cruft inside the cell (they break rows).
  const BLOCK_SEL = "p,div,h1,h2,h3,h4,h5,h6,li,blockquote,section,article";

  const CELL_BLOCK = /^(P|DIV|H[1-6]|LI|BLOCKQUOTE|SECTION|ARTICLE)$/;

  function cellMd(cell) {
    const c = cell.cloneNode(true);
    c.querySelectorAll("img, svg, .screenreader-only, .external_link_icon, script, style")
      .forEach((n) => n.remove());
    // Drop empty emphasis (Canvas leaves <strong></strong> -> "****" artifacts).
    c.querySelectorAll("strong, b, em, i, span").forEach((el) => {
      if (el.childElementCount === 0 && !el.textContent.split(NBSP).join("").trim()) el.remove();
    });
    c.querySelectorAll("br").forEach((br) => br.replaceWith(document.createTextNode(" ")));

    // Lists -> "; " joined inline, in place.
    c.querySelectorAll("ul, ol").forEach((list) => {
      const items = [...list.querySelectorAll("li")]
        .map((li) => inlineMd(li).replace(/\s+/g, " ").trim())
        .filter(Boolean);
      const span = document.createElement("span");
      span.textContent = items.join("; ");
      list.replaceWith(span);
    });

    // Walk top-level children IN ORDER: accumulate inline runs, flush on each block.
    // Preserves direct inline content (e.g. <strong>1</strong>) alongside <p> blocks.
    const segs = [];
    let cur = "";
    const flush = () => { const t = cur.replace(/\s+/g, " ").trim(); if (t) segs.push(t); cur = ""; };
    c.childNodes.forEach((n) => {
      if (n.nodeType === 3) { cur += n.textContent; return; }
      if (n.nodeType !== 1) return;
      if (SKIP.has(n.tagName)) return;
      if (CELL_BLOCK.test(n.tagName)) {
        flush();
        const inner = inlineMd(n).replace(/\s+/g, " ").trim();
        if (inner) segs.push(inner);
      } else {
        cur += inlineMd(n);
      }
    });
    flush();

    // Join segments with " Â· " â€” avoids <br> HTML inside table cells which conflicts
    // with inline markdown parsing when rehypeRaw is active.
    return segs.join(" Â· ").split(NBSP).join(" ").replace(/[ \t]+/g, " ").replace(/\|/g, "\\|").trim();
  }

  function tableMd(node) {
    const rows = [...node.querySelectorAll("tr")];
    if (!rows.length) return "";
    const head = [...rows[0].children].map(cellMd);
    const cols = head.length;
    const pad = (arr) => { while (arr.length < cols) arr.push(""); return arr.slice(0, cols); };
    let out = "| " + pad(head).join(" | ") + " |\n";
    out += "| " + head.map(() => "---").join(" | ") + " |\n";
    rows.slice(1).forEach((r) => {
      out += "| " + pad([...r.children].map(cellMd)).join(" | ") + " |\n";
    });
    return out;
  }

  function blockMd(node, depth) {
    let out = "";
    node.childNodes.forEach((n) => {
      if (n.nodeType === 3) { const t = n.textContent.trim(); if (t) out += t + "\n\n"; return; }
      if (n.nodeType !== 1) return;
      const t = n.tagName;
      if (SKIP.has(t)) return;
      if (/^H[1-6]$/.test(t)) out += "#".repeat(+t[1]) + " " + inlineMd(n).trim() + "\n\n";
      else if (t === "P") { const c = inlineMd(n).trim(); if (c) out += c + "\n\n"; }
      else if (t === "UL" || t === "OL") out += listMd(n, 0, t === "OL") + "\n";
      else if (t === "PRE") out += "```\n" + n.textContent.replace(/\n$/, "") + "\n```\n\n";
      else if (t === "BLOCKQUOTE") out += blockMd(n, depth).trim().split("\n").map((l) => "> " + l).join("\n") + "\n\n";
      else if (t === "HR") out += "---\n\n";
      else if (t === "TABLE") out += tableMd(n) + "\n";
      else if (t === "DIV" || t === "SECTION" || t === "ARTICLE") out += blockMd(n, depth);
      else { const c = inlineMd(n).trim(); if (c) out += c + "\n\n"; }
    });
    return out;
  }

  // Strip decorative / accessibility cruft globally before conversion.
  function preprocess(root) {
    root
      .querySelectorAll(
        'script, style, svg, .screenreader-only, [class*="screenReaderContent"], ' +
        '.external_link_icon, button.ally-accessible-versions, img[role="presentation"], ' +
        'td img, th img'
      )
      .forEach((n) => n.remove());
    // Unwrap <sup> footnote markers â€” keep the text but not the superscript element.
    root.querySelectorAll("sup").forEach((el) => {
      el.replaceWith(document.createTextNode(el.textContent));
    });
    // Drop truly empty emphasis wrappers (no text, no children) â€” leaves "****" artifacts.
    // childElementCount check preserves <strong><img/></strong> (image in bold wrapper).
    root.querySelectorAll("strong, b, em, i").forEach((el) => {
      if (el.childElementCount === 0 && !el.textContent.split(NBSP).join("").trim()) el.remove();
    });
  }

  function toMd(html) {
    const doc = new DOMParser().parseFromString(html, "text/html");
    preprocess(doc.body);
    return blockMd(doc.body, 0).replace(/\n{3,}/g, "\n\n").trim();
  }

  // Download inline images locally + rewrite <img src> to relative 'images/...'.
  const IMG_EXT = {
    "image/png": "png", "image/jpeg": "jpg", "image/jpg": "jpg", "image/gif": "gif",
    "image/webp": "webp", "image/svg+xml": "svg", "image/bmp": "bmp",
  };
  const strHash = (s) => {
    let h = 0;
    for (let i = 0; i < s.length; i++) h = ((h << 5) - h + s.charCodeAt(i)) | 0;
    return Math.abs(h).toString(36);
  };

  async function downloadImages(root, c) {
    const imgs = [...root.querySelectorAll("img")];
    console.log("[Oculus] downloadImages: found", imgs.length, "img(s) in", c.code);
    for (const img of imgs) {
      // The <img> carries the exact same-origin API endpoint + file id.
      const dataId = img.getAttribute("data-id");
      let apiEp = img.getAttribute("data-api-endpoint");
      if (!apiEp && dataId) apiEp = "/api/v1/files/" + dataId;
      if (!apiEp) { console.log("[Oculus] img has no data-api-endpoint/data-id, skip"); continue; }
      try {
        // Canvas file URLs redirect to canvas-user-content.com CDN (no CORS) -> browser
        // fetch is blocked. info.url needs session cookies (Rust/ureq has none -> gets a
        // login HTML page). The /public_url endpoint returns a SIGNED url that needs no
        // auth -- exactly what a cookieless server-side fetch needs.
        console.log("[Oculus] file API:", apiEp);
        const infoR = await cfetch(apiEp);
        console.log("[Oculus] file API status:", infoR.status);
        if (!infoR.ok) continue;
        const info = await infoR.json();
        const ct = (info["content-type"] || info.content_type || "image/png").split(";")[0].trim();
        const ext = IMG_EXT[ct] || "png";
        const fid = dataId || info.id || strHash(apiEp);

        const pubR = await cfetch(apiEp + "/public_url");
        const pub = pubR.ok ? await pubR.json() : {};
        const cdnUrl = pub.public_url || info.url;
        console.log("[Oculus] public_url status:", pubR.status, "got url:", !!pub.public_url);
        if (!cdnUrl) { console.log("[Oculus] no signed url"); continue; }

        const path = "images/" + fid + "." + ext;
        console.log("[Oculus] proxying image ->", path);
        await fetch(BASE + "/image-proxy?" + q({ course: c.code, subject_id: c.id, path, canvas_id: fid }), {
          method: "POST",
          headers: { "Content-Type": "text/plain" },
          body: cdnUrl,
        }).catch((e) => console.error("[Oculus] proxy post failed", e));
        img.setAttribute("src", path);
        console.log("[Oculus] image done:", path);
      } catch (e) {
        console.error("[Oculus] img error:", e);
        await logMsg("warning", c.code, "img " + fid + ": " + e);
      }
    }
  }

  // Like toMd, but first downloads inline images and rewrites their src locally.
  async function toMdAssets(html, c) {
    const doc = new DOMParser().parseFromString(html, "text/html");
    preprocess(doc.body);
    await downloadImages(doc.body, c);
    return blockMd(doc.body, 0).replace(/\n{3,}/g, "\n\n").trim();
  }

  // ---- Cancel support -------------------------------------------------------
  // Rust can set window.__oculus_cancel = true via a subsequent win.eval().
  // Check between each expensive operation and bail early if set.
  const isCancelled = () => typeof window.__oculus_cancel !== "undefined" && window.__oculus_cancel === true;

  // ---- Phase: home ----------------------------------------------------------

  async function scrapeHome(c) {
    if (isCancelled()) return 0;
    const cr = await cfetch(
      "/api/v1/courses/" + c.id +
      "?include[]=syllabus_body&include[]=public_description&include[]=teachers&include[]=term"
    );
    const course = cr.ok ? await cr.json() : {};
    const name = course.name || c.code;
    const term = (course.term && course.term.name) || "";
    const teachers = (course.teachers || []).map((t) => t.display_name).filter(Boolean);

    // Syllabus — save separately regardless of front page content.
    if (course.syllabus_body) {
      let head = "# " + name + " — Syllabus\n\n";
      const meta = [];
      if (term) meta.push("**Term:** " + term);
      meta.push("**Code:** " + c.code);
      if (teachers.length) meta.push("**Staff:** " + teachers.join(", "));
      head += meta.join("  \n") + "\n\n---\n\n";
      await postText(c.code, c.id, "syllabus.md", head + await toMdAssets(course.syllabus_body, c));
    }

    let body = "", source = "";
    const r = await cfetch("/api/v1/courses/" + c.id + "/front_page");
    if (r.ok) { const j = await r.json(); if (j.body) { body = j.body; source = "Front Page"; } }
    if (!body && course.public_description) { body = "<p>" + course.public_description + "</p>"; source = "Description"; }

    if (!body) return 0; // no real content -> write nothing

    let head = "# " + name + "\n\n";
    const meta = [];
    if (term) meta.push("**Term:** " + term);
    meta.push("**Code:** " + c.code);
    if (teachers.length) meta.push("**Staff:** " + teachers.join(", "));
    head += meta.join("  \n") + "\n\n> Source: " + source + "\n\n";
    await postText(c.code, c.id, "home.md", head + "---\n\n" + await toMdAssets(body, c));
    return 1;
  }

  // ---- Phase: pages (fetched individually via module discovery) -------------
  // UniMelb disables /pages bulk listing (404) â€” pages discovered via scrapeModules.

  // Extracts Canvas page slugs + file IDs linked within an HTML body.
  function extractCanvasLinks(html, courseId) {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const pages = new Set(), files = new Set();
    const coursePrefix = "/courses/" + courseId + "/";
    doc.querySelectorAll("a[href]").forEach((a) => {
      const href = a.getAttribute("href") || "";
      // Page links: /courses/{id}/pages/{slug}
      const pm = href.match(/\/courses\/\d+\/pages\/([^?#/]+)/);
      if (pm) pages.add(pm[1]);
      // File links: /courses/{id}/files/{id} or /files/{id}
      const fm = href.match(/\/files\/(\d+)/);
      if (fm && href.includes(coursePrefix)) files.add(fm[1]);
    });
    return { pages, files };
  }

  async function fetchPage(c, pageUrl, title, orphanPageQueue, orphanFileQueue) {
    if (isCancelled()) return null;
    try {
      const r = await cfetch("/api/v1/courses/" + c.id + "/pages/" + pageUrl);
      if (!r.ok) return null;
      const full = await r.json();
      if (!full.body) return null;
      // Collect orphaned links within this page for later scraping.
      if (orphanPageQueue || orphanFileQueue) {
        const { pages, files } = extractCanvasLinks(full.body, c.id);
        pages.forEach((p) => orphanPageQueue && orphanPageQueue.add(p));
        files.forEach((f) => orphanFileQueue && orphanFileQueue.add(f));
      }
      const md =
        "# " + (full.title || title || "Untitled") + "\n\n" +
        (full.updated_at ? "_Updated: " + full.updated_at + "_\n\n" : "") +
        "---\n\n" + await toMdAssets(full.body, c);
      const filename = "pages/" + slug(full.title || title || pageUrl) + ".md";
      await postText(c.code, c.id, filename, md);
      return filename;
    } catch (e) {
      await logMsg("warning", c.code, "page " + pageUrl + ": " + e);
      return null;
    }
  }

  async function scrapePages(c) { void c; return 0; } // driven by scrapeModules now

  // ---- Phase: assignments (STUB) --------------------------------------------
  // TODO: fetchAll('/api/v1/courses/'+c.id+'/assignments?per_page=100')
  //   -> md from a.description (toMd) + due_at, points_possible, submission_types
  //   -> postText(c.code, c.id, 'assignments/'+slug(a.name)+'.md', md)
  async function scrapeAssignments(c) { void c; return 0; }

  // ---- Phase: announcements -------------------------------------------------

  async function scrapeAnnouncements(c) {
    const list = await fetchAll(
      CANVAS_ORIGIN + "/api/v1/courses/" + c.id +
      "/discussion_topics?only_announcements=true&per_page=100&include[]=author"
    );
    let n = 0;
    let idx = 0;
    for (const a of list) {
      if (isCancelled()) break;
      idx++;
      await progress({ done: idx, total: list.length, course: c.code, phase: "announcements", label: a.title || "Announcement" });
      try {
        if (!a.message) continue;
        const date = (a.posted_at || a.created_at || "").slice(0, 10);
        const author = (a.author && a.author.display_name) || "";
        const header =
          "# " + (a.title || "Announcement") + "\n\n" +
          (date   ? "**Date:** " + date + "  \n" : "") +
          (author ? "**From:** " + author + "\n" : "") +
          "\n---\n\n";
        const md = header + await toMdAssets(a.message, c);
        const filename = (date ? date + "-" : "") + slug(a.title || "announcement") + ".md";
        await postText(c.code, c.id, "announcements/" + filename, md);
        n++;
      } catch (e) {
        await logMsg("warning", c.code, "announcement " + a.id + ": " + e);
      }
    }
    return n;
  }

  // ---- Phase: files (fetched individually via module discovery) -------------
  // UniMelb disables /files bulk listing (403) â€” files discovered via scrapeModules.

  const DOWNLOADABLE_TYPES = new Set([
    "application/pdf",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  ]);

  async function fetchFile(c, fileId, displayName) {
    try {
      const infoR = await cfetch("/api/v1/files/" + fileId);
      if (!infoR.ok) return null;
      const info = await infoR.json();
      const ct = ((info["content-type"] || info.content_type || "")).split(";")[0].trim();
      if (!DOWNLOADABLE_TYPES.has(ct)) return null;
      if (info.size > MAX_FILE_BYTES) return null;

      const pubR = await cfetch("/api/v1/files/" + fileId + "/public_url");
      const pub = pubR.ok ? await pubR.json() : {};
      const dlUrl = pub.public_url || info.url;
      if (!dlUrl) return null;

      const name = (info.filename || info.display_name || displayName || (fileId + ".bin")).replace(/[/\\]/g, "_");
      const path = "files/" + name;
      await fetch(BASE + "/image-proxy?" + q({ course: c.code, subject_id: c.id, path, canvas_id: fileId }), {
        method: "POST",
        headers: { "Content-Type": "text/plain" },
        body: dlUrl,
      }).catch(() => {});
      return path;
    } catch (e) {
      await logMsg("warning", c.code, "file " + fileId + ": " + e);
      return null;
    }
  }

  async function scrapeFiles(c) { void c; void ALLOWED_FILE_TYPES; return 0; } // driven by scrapeModules

  // ---- Phase: modules (driver for pages + files) ----------------------------

  async function scrapeModules(c) {
    if (isCancelled()) return 0;
    const modules = await fetchAll(
      "/api/v1/courses/" + c.id + "/modules?include[]=items&per_page=100"
    );
    let pageCount = 0, fileCount = 0;
    const seenPages = new Set(), seenFiles = new Set();
    // Queues for orphaned links found inside pages (one level deep).
    const orphanPages = new Set(), orphanFiles = new Set();

    // Total syncable module items across ALL modules (SubHeaders excluded).
    // Each item counts as 1 even if it triggers nested page/file fetches.
    const totalItems = modules.reduce(
      (n, m) => n + (m.items || []).filter((it) => it.type !== "SubHeader").length, 0
    );
    let processed = 0;

    for (const mod of modules) {
      if (isCancelled()) break;
      const items = mod.items || [];
      const pad = String(mod.position || 0).padStart(2, "0");
      const modSlug = slug(mod.name || "module");
      const tocLines = ["# " + (mod.name || "Module") + "\n"];

      for (const item of items) {
        if (isCancelled()) break;
        const type = item.type;
        const title = item.title || "Untitled";
        const indentStr = "  ".repeat(item.indent || 0);

        if (type === "SubHeader") {
          tocLines.push(indentStr + "## " + escapeMd(title));
          continue;
        }

        // Count every real module item once, before any nested work.
        processed++;
        await progress({ done: processed, total: totalItems, course: c.code, phase: "modules", label: title });

        if (type === "Page" && item.page_url) {
          if (!seenPages.has(item.page_url)) {
            seenPages.add(item.page_url);
            const saved = await fetchPage(c, item.page_url, title, orphanPages, orphanFiles);
            if (saved) pageCount++;
            tocLines.push(indentStr + "- [" + escapeMd(title) + "](../pages/" + slug(title) + ".md)");
          } else {
            tocLines.push(indentStr + "- [" + escapeMd(title) + "](../pages/" + slug(title) + ".md)");
          }
          continue;
        }
        if (type === "File" && item.content_id) {
          const key = String(item.content_id);
          if (!seenFiles.has(key)) {
            seenFiles.add(key);
            const saved = await fetchFile(c, item.content_id, title);
            if (saved) { fileCount++; tocLines.push(indentStr + "- [" + escapeMd(title) + "](../" + saved + ")"); }
            else { tocLines.push(indentStr + "- " + escapeMd(title) + " _(file)_"); }
          } else {
            tocLines.push(indentStr + "- " + escapeMd(title) + " _(file)_");
          }
          seenFiles.add(String(item.content_id));
          continue;
        }
        if (type === "Assignment") {
          tocLines.push(indentStr + "- [" + escapeMd(title) + "](" + (item.html_url || "") + ") _(assignment)_");
          continue;
        }
        if (type === "Quiz") {
          tocLines.push(indentStr + "- [" + escapeMd(title) + "](" + (item.html_url || "") + ") _(quiz)_");
          continue;
        }
        if (type === "ExternalUrl") {
          tocLines.push(indentStr + "- [" + escapeMd(title) + "](" + (item.external_url || item.html_url || "") + ") _(external)_");
          continue;
        }
        const url = item.html_url || "";
        tocLines.push(url
          ? indentStr + "- [" + escapeMd(title) + "](" + url + ")"
          : indentStr + "- " + escapeMd(title));
      }

      await postText(c.code, c.id, "modules/" + pad + "-" + modSlug + ".md", tocLines.join("\n") + "\n");
    }

    // Scrape orphaned pages linked within pages but not in any module.
    // These are nested content — not counted as module items, no progress emit.
    for (const pageUrl of orphanPages) {
      if (isCancelled()) break;
      if (seenPages.has(pageUrl)) continue;
      seenPages.add(pageUrl);
      const saved = await fetchPage(c, pageUrl, pageUrl, null, null);
      if (saved) pageCount++;
    }
    // Download orphaned files linked within pages but not in any module.
    for (const fileId of orphanFiles) {
      if (isCancelled()) break;
      if (seenFiles.has(fileId)) continue;
      seenFiles.add(fileId);
      await fetchFile(c, fileId, null);
    }

    return pageCount + fileCount;
  }

  // ---- Orchestrator ---------------------------------------------------------

  async function scrapeCourse(c, idx, total) {
    const phase = (name, label = "") => progress({ done: idx, total, course: c.code, phase: name, label });
    await phase("home");          if (isCancelled()) return; await scrapeHome(c);
    await phase("announcements"); if (isCancelled()) return; await scrapeAnnouncements(c);
    await phase("modules");       if (isCancelled()) return; await scrapeModules(c);
    // scrapePages + scrapeFiles are driven by scrapeModules now; stubs kept for compat.
    void scrapePages; void scrapeAssignments; void scrapeFiles;
  }

  (async () => {
    console.log("[Oculus] scrape start:", SUBJECTS.length, "subjects");
    window.__oculus_cancel = false;
    let i = 0;
    for (const c of SUBJECTS) {
      if (isCancelled()) {
        await postJson("/scrape-done", { count: i, cancelled: true });
        console.log("[Oculus] cancelled after", i, "subjects");
        return;
      }
      try { await scrapeCourse(c, i, SUBJECTS.length); }
      catch (e) { await logMsg("error", c.code, String(e)); console.error("[Oculus]", c.code, e); }
      i++;
      await progress({ done: i, total: SUBJECTS.length, course: c.code, phase: "complete", label: "" });
    }
    await postJson("/scrape-done", { count: i, cancelled: false });
    console.log("[Oculus] scrape complete:", i);
  })();
})();
