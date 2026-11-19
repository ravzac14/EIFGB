// The feedback form: reading the Google Form's pre-filled link, what gets
// sent, and leaving the game's lines out. Run: node test/feedback.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register({ url: "https://example.org/EIFGB/play.html?example=lighthouse", width: 1000, height: 700 });
const { feedbackContext, feedbackForm, formTarget, report } = await import("../src/feedback.ts");

// The pre-filled link names each field by its answer.
const link =
  "https://docs.google.com/forms/d/e/1FAIpQLSabc/viewform?usp=pp_url&entry.11=TYPE&entry.22=MESSAGE&entry.33=CONTACT&entry.44=PAGE&entry.55=VERSIONS&entry.66=CONTEXT";
assert.deepEqual(formTarget(link), {
  action: "https://docs.google.com/forms/d/e/1FAIpQLSabc/formResponse",
  fields: { type: "entry.11", message: "entry.22", contact: "entry.33", page: "entry.44", versions: "entry.55", context: "entry.66" },
});
assert.equal(formTarget(""), null);
assert.equal(formTarget("https://example.org/not-a-form"), null);

// What's sent besides the message: where, which versions, and context.
feedbackContext({
  info: () => ({ game: "The Last Light", where: "Lamp Room, turn 6" }),
  versions: () => ({ game: "lighthouse 1.0" }),
  log: () => "> look\nLamp Room",
});
window.dispatchEvent(new ErrorEvent("error", { message: "boom", filename: "https://example.org/EIFGB/assets/play.js", lineno: 3 }));
let r = report(true);
assert.equal(r.page, "/play.html?example=lighthouse");
assert.match(r.versions, /game: lighthouse 1\.0/);
assert.match(r.context, /game: The Last Light\nwhere: Lamp Room, turn 6/);
assert.match(r.context, /browser: /);
assert.match(r.context, /settings: auto, medium, serif, normal, medium/);
assert.match(r.context, /recent errors:\n\S+ boom \(play\.js:3\)/);
assert.match(r.context, /last lines of the game:\n> look\nLamp Room/);
assert.doesNotMatch(report(false).context, /last lines/);

// The form: the game's lines can be left out; Send posts the answers.
const sent = [];
let closed = null;
const form = feedbackForm((s) => (closed = s), async (target, values) => sent.push(values), null);
document.body.append(form);
assert.match(form.querySelector(".feedback-context").textContent, /last lines of the game/);
const box = form.querySelector("#feedback-log");
box.checked = false;
box.dispatchEvent(new Event("change"));
assert.doesNotMatch(form.querySelector(".feedback-context").textContent, /last lines/);
// No message yet: nothing is sent.
form.requestSubmit();
await new Promise((r) => setTimeout(r));
assert.equal(sent.length, 0);
// Not set up (no form link): it offers to copy instead.
assert.equal(form.querySelector("button[type=submit]").textContent, "Copy the report");

// Set up: Send posts every answer, with the kind in words.
const form2 = feedbackForm((s) => (closed = s), async (target, values) => sent.push({ target, values }), formTarget(link));
document.body.append(form2);
form2.querySelector("#feedback-kind").value = "gameplay";
form2.querySelector("#feedback-message").value = "  The lamp should flicker.  ";
form2.querySelector("#feedback-contact").value = "me@example.org";
form2.requestSubmit();
await new Promise((r) => setTimeout(r, 10));
assert.equal(sent.length, 1);
const { target, values } = sent[0];
assert.equal(target.action, "https://docs.google.com/forms/d/e/1FAIpQLSabc/formResponse");
assert.equal(values.type, "An idea for playing games");
assert.equal(values.message, "The lamp should flicker.");
assert.equal(values.contact, "me@example.org");
assert.equal(values.page, "/play.html?example=lighthouse");
assert.match(values.versions, /game: lighthouse 1\.0/);
assert.match(values.context, /last lines of the game/);
assert.match(form2.textContent, /Thank you/);
form2.querySelector("button").click();
assert.equal(closed, true);
// This site's form is set up.
assert.match(formTarget()?.action ?? "", /^https:\/\/docs\.google\.com\/forms\/d\/e\/[\w-]+\/formResponse$/);
assert.deepEqual(Object.keys(formTarget().fields).sort(), ["contact", "context", "message", "page", "type", "versions"]);
console.log("ok   feedback");
