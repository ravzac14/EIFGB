// Putting a game on another web page: a snippet of HTML with the player in
// a frame (play.html?embed=1), which anyone can paste into a blog, a site
// builder's "embed" box, or their own page.

/** The play address made to sit in a frame. */
export function embedUrl(playUrl: string): string {
  const u = new URL(playUrl);
  u.searchParams.set("embed", "1");
  return u.href;
}

const attr = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/** The snippet: a frame as wide as the page around it, and tall enough to read in. */
export function embedCode(playUrl: string, title: string): string {
  return `<iframe src="${attr(embedUrl(playUrl))}" title="${attr(title)}" width="100%" height="600" style="border:1px solid #8884;border-radius:8px;max-width:48rem" loading="lazy" allow="autoplay"></iframe>`;
}

/** A read-only box with the snippet and a Copy button. */
export function embedBox(playUrl: string, title: string): HTMLElement {
  const code = embedCode(playUrl, title);
  const box = Object.assign(document.createElement("textarea"), { readOnly: true, value: code, rows: 3, className: "embed-code" });
  box.setAttribute("aria-label", `HTML to put ${title} on your own page`);
  box.addEventListener("focus", () => box.select());
  const copy = Object.assign(document.createElement("button"), { type: "button", textContent: "Copy" });
  copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(code);
      copy.textContent = "Copied";
    } catch {
      box.select();
    }
  });
  const wrap = document.createElement("div");
  wrap.className = "embed";
  wrap.append(box, copy);
  return wrap;
}
