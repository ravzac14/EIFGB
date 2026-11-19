// The stand-alone feedback page, for pages without the feedback dialog
// (the docs link here with ?from=<the page they were on>).
import { settingsDialog } from "./settings-ui.ts";
import { feedbackContext, feedbackForm } from "./feedback.ts";

settingsDialog(document.getElementById("settings") as HTMLDialogElement, document.getElementById("settings-open")!);
const from = new URLSearchParams(location.search).get("from");
// Only a path on this site: anything else is ignored.
if (from && /^[\w./#?=&%-]+$/.test(from) && !from.includes("//")) feedbackContext({ place: () => `/${from.replace(/^\/+/, "")}` });
const main = document.getElementById("feedback-page")!;
const back = () => {
  if (from && history.length > 1) history.back();
  else location.href = "./";
};
main.append(feedbackForm((sent) => (sent ? (location.href = "./") : back())));
