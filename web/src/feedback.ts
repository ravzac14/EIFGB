// Feedback from anywhere on the site, sent to a Google Form (see
// feedback-config.ts). Along with what the person writes, it sends where
// they are, which versions they're seeing, and easy debugging context: the
// browser, their display settings, recent errors, and (when playing) the
// last lines of their game, which they can see and leave out. Nothing is
// sent until they press Send.
import { FEEDBACK_FORM } from "./feedback-config.ts";
import { getGameLook, settings } from "./store.ts";

declare const __BUILD__: { commit: string; built: string; engine: string } | undefined;
/** This build of the site: set by vite.config.ts, missing when tests run the sources directly. */
const BUILD = typeof __BUILD__ === "undefined" ? { commit: "dev", built: "", engine: "" } : __BUILD__;

export const KINDS: [string, string][] = [
  ["bug", "Something's broken"],
  ["confusing", "Something was confusing"],
  ["gameplay", "An idea for playing games"],
  ["editor", "An idea for the editor"],
  ["website", "An idea for the website"],
  ["other", "Something else"],
];

type Info = Record<string, string | number | boolean | null | undefined>;

/** The page's own context (a game's room and turn, the editor's selection…). */
let pageInfo: () => Info = () => ({});
/** The page's version lines beyond the site's (a game's version…). */
let pageVersions: () => Info = () => ({});
/** The last lines of the game being played, if any. */
let recentLog: () => string = () => "";
/** Where the person came from, for the stand-alone feedback page. */
let place: () => string = () => location.pathname.replace(/^.*\//, "/") + location.search;

export function feedbackContext(opts: { info?: () => Info; versions?: () => Info; log?: () => string; place?: () => string }) {
  if (opts.info) pageInfo = opts.info;
  if (opts.versions) pageVersions = opts.versions;
  if (opts.log) recentLog = opts.log;
  if (opts.place) place = opts.place;
}

// The last few errors on the page: the most useful thing in a bug report.
const errors: string[] = [];
const noteError = (what: string) => {
  errors.push(`${new Date().toISOString().slice(11, 19)} ${what}`.slice(0, 400));
  if (errors.length > 5) errors.shift();
};
if (typeof window !== "undefined") {
  window.addEventListener("error", (e) => noteError(`${e.message}${e.filename ? ` (${e.filename.replace(/^.*\//, "")}:${e.lineno})` : ""}`));
  window.addEventListener("unhandledrejection", (e) => noteError(`unhandled: ${e.reason instanceof Error ? e.reason.message : String(e.reason)}`));
}

const lines = (info: Info) =>
  Object.entries(info)
    .filter(([, v]) => v !== undefined && v !== null && v !== "")
    .map(([k, v]) => `${k}: ${v}`)
    .join("\n");

function storageWorks() {
  try {
    localStorage.setItem("eifgb:probe", "1");
    localStorage.removeItem("eifgb:probe");
    return true;
  } catch {
    return false;
  }
}

/** Everything sent besides what the person writes. */
export function report(withLog: boolean) {
  const s = settings.get();
  const look = getGameLook();
  const versions = lines({ site: `${BUILD.commit}${BUILD.built ? ` (built ${BUILD.built})` : ""}`, engine: BUILD.engine, ...pageVersions() });
  const context = [
    lines({
      ...pageInfo(),
      browser: navigator.userAgent,
      language: navigator.language,
      window: `${innerWidth}×${innerHeight}`,
      screen: `${screen.width}×${screen.height} @${devicePixelRatio}x`,
      "dark mode": matchMedia("(prefers-color-scheme: dark)").matches,
      settings: `${s.theme}, ${s.size}, ${s.face}, ${s.spacing}, ${s.width}`,
      "game look": look ? [look.theme, look.accent].filter(Boolean).join(", ") : "",
      storage: storageWorks() ? "ok" : "unavailable",
      online: navigator.onLine,
    }),
    errors.length ? `recent errors:\n${errors.join("\n")}` : "",
    withLog && recentLog() ? `last lines of the game:\n${recentLog()}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");
  return { page: place(), versions, context };
}

/** The form's address and field names, from the pre-filled link in feedback-config.ts. */
export function formTarget(link = FEEDBACK_FORM): { action: string; fields: Record<string, string> } | null {
  if (!link) return null;
  try {
    const u = new URL(link);
    const id = u.pathname.match(/\/forms\/d\/e\/([^/]+)/)?.[1];
    if (!id) return null;
    const fields: Record<string, string> = {};
    for (const [k, v] of u.searchParams) if (k.startsWith("entry.")) fields[v.trim().toLowerCase()] = k;
    if (!fields.type || !fields.message) return null;
    return { action: `https://docs.google.com/forms/d/e/${id}/formResponse`, fields };
  } catch {
    return null;
  }
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: (Node | string)[]): HTMLElementTagNameMap[K] {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

/**
 * The form. `done` is called after sending (or Cancel), with whether it was sent.
 * `send` and `target` are how and where it's sent: a POST to the Google
 * Form in feedback-config.ts, unless a test swaps them.
 */
export function feedbackForm(done: (sent: boolean) => void, send = postToForm, target = formTarget()): HTMLFormElement {
  const form = el("form", { className: "feedback" });
  const kind = el("select", { id: "feedback-kind" });
  for (const [v, t] of KINDS) kind.append(el("option", { value: v, textContent: t }));
  if (/editor/.test(location.pathname)) kind.value = "editor";
  const message = el("textarea", { id: "feedback-message", rows: 5, required: true, placeholder: "What happened, or what would you like? For something broken: what you did, what you expected, and what happened instead." });
  const contact = el("input", { id: "feedback-contact", type: "text", autocomplete: "email", placeholder: "An email address or GitHub name, if you'd like a reply" });
  const hasLog = !!recentLog();
  const withLog = el("input", { type: "checkbox", checked: hasLog, id: "feedback-log" });
  const shown = el("pre", { className: "feedback-context" });
  const refresh = () => {
    const r = report(withLog.checked);
    shown.textContent = `page: ${r.page}\n\n${r.versions}\n\n${r.context}`;
  };
  withLog.addEventListener("change", refresh);
  refresh();
  const status = el("p", { className: "dim", role: "status" });
  const cancel = el("button", { type: "button", textContent: "Cancel" });
  cancel.addEventListener("click", () => done(false));
  const submit = el("button", { type: "submit", className: "primary", textContent: target ? "Send" : "Copy the report" });

  form.append(
    el("label", { htmlFor: "feedback-kind", textContent: "What is it?" }),
    kind,
    el("label", { htmlFor: "feedback-message", textContent: "Tell us about it" }),
    message,
    el("label", { htmlFor: "feedback-contact", textContent: "How to reach you (optional)" }),
    contact,
    ...(hasLog ? [el("label", { className: "check" }, withLog, " Include the last lines of your game (shown below), to help find the problem")] : []),
    el("details", {}, el("summary", { textContent: "What else gets sent" }), el("p", { className: "dim", textContent: "This helps find and fix problems. Nothing else about you is sent." }), shown),
    el(
      "p",
      { className: "dim" },
      target
        ? "Sent to the site's author with Google Forms when you press Send, and only then."
        : "Sending isn't set up on this copy of the site: copy the report, and send it to whoever runs it.",
    ),
    el("div", { className: "row" }, submit, cancel),
    status,
  );

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!message.value.trim()) {
      status.textContent = "Please say what it's about first.";
      message.focus();
      return;
    }
    const r = report(withLog.checked);
    const values: Record<string, string> = {
      type: KINDS.find(([v]) => v === kind.value)?.[1] ?? kind.value,
      message: message.value.trim(),
      contact: contact.value.trim(),
      page: r.page,
      versions: r.versions,
      context: r.context,
    };
    const text = Object.entries(values)
      .filter(([, v]) => v)
      .map(([k, v]) => `${k.toUpperCase()}\n${v}`)
      .join("\n\n");
    if (!target) {
      await copy(text, status);
      return;
    }
    submit.disabled = true;
    status.textContent = "Sending…";
    try {
      await send(target, values);
      form.replaceChildren(el("p", { textContent: "Thank you! Your feedback was sent." }), el("div", { className: "row" }, Object.assign(el("button", { type: "button", className: "primary", textContent: "Done" }), { onclick: () => done(true) })));
    } catch {
      submit.disabled = false;
      status.textContent = "It couldn't be sent (are you online?). ";
      const b = el("button", { type: "button", className: "link", textContent: "Copy the report instead" });
      b.addEventListener("click", () => copy(text, status));
      status.append(b);
    }
  });
  return form;
}

async function copy(text: string, status: HTMLElement) {
  try {
    await navigator.clipboard.writeText(text);
    status.textContent = "Copied. Paste it wherever you're sending it.";
  } catch {
    status.textContent = "Couldn't copy it automatically. Open “What else gets sent” and copy it from there.";
  }
}

/**
 * Posts the answers to the Google Form. Google doesn't let pages read its
 * reply, so this can only tell whether the request went out.
 */
async function postToForm(target: { action: string; fields: Record<string, string> }, values: Record<string, string>) {
  const body = new URLSearchParams();
  for (const [name, value] of Object.entries(values)) {
    const field = target.fields[name];
    // Google Forms caps an answer's length; keep well inside it.
    if (field && value) body.append(field, value.slice(0, 20000));
  }
  await fetch(target.action, { method: "POST", mode: "no-cors", body });
}

/** Opens the form in a dialog when `opener` is clicked. */
export function feedbackButton(opener: HTMLElement) {
  opener.addEventListener("click", () => {
    let dialog = document.getElementById("feedback") as HTMLDialogElement | null;
    if (!dialog) {
      dialog = el("dialog", { id: "feedback" });
      dialog.setAttribute("aria-labelledby", "feedback-title");
      document.body.append(dialog);
    }
    const d = dialog;
    d.replaceChildren(el("h2", { id: "feedback-title", textContent: "Send feedback" }), feedbackForm(() => d.close()));
    d.showModal();
    d.querySelector<HTMLTextAreaElement>("#feedback-message")?.focus();
  });
}
