// Tab completion for a typed command line, shared by the site's player and
// the player in a downloaded game (so it's plain JavaScript, no imports).
// Tab finishes the word being typed from what the game knows; with several
// matches it puts in the first and shows them all under the line, and Tab
// again (Shift+Tab back) goes through them, as zsh and PowerShell do. Esc
// puts back what was typed. With nothing to finish, Tab moves focus as
// usual. The matches are only shown: they're typed, never clicked.
(function () {
  "use strict";
  /**
   * @param {HTMLInputElement} input
   * @param {(line: string) => string[]} complete words that finish the line's last word
   */
  function attachTab(input, complete) {
    var hint = document.createElement("div");
    hint.className = "tab-options";
    hint.setAttribute("aria-live", "polite");
    hint.hidden = true;
    input.after(hint);
    var state = null;
    function clear() {
      state = null;
      hint.hidden = true;
      hint.replaceChildren();
    }
    function show() {
      hint.replaceChildren();
      state.options.forEach(function (w, j) {
        var s = document.createElement("span");
        s.textContent = w;
        if (j === state.i) {
          s.className = "on";
          s.setAttribute("aria-current", "true");
        }
        hint.append(s, " ");
      });
      hint.hidden = false;
    }
    input.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && state) {
        input.value = state.typed;
        clear();
        return;
      }
      if (e.key !== "Tab" || e.altKey || e.ctrlKey || e.metaKey) return;
      if (state && input.value === state.value) {
        state.i = (state.i + (e.shiftKey ? -1 : 1) + state.options.length) % state.options.length;
      } else {
        if (e.shiftKey) return;
        var typed = input.value;
        var options = [];
        try {
          options = typed && !/\s$/.test(typed) ? complete(typed) : [];
        } catch (err) {
          options = [];
        }
        if (!Array.isArray(options) || !options.length) return;
        state = { typed: typed, base: typed.replace(/[^\s.,]*$/, ""), options: options, i: 0, value: "" };
      }
      e.preventDefault();
      var w = state.options[state.i];
      // The player's own capitals: TAKE LA, TAKE LANTERN.
      var word = state.typed.slice(state.base.length);
      if (/[A-Z]/.test(word) && word === word.toUpperCase()) w = w.toUpperCase();
      input.value = state.base + w + (state.options.length === 1 ? " " : "");
      state.value = input.value;
      if (state.options.length > 1) show();
      else clear();
    });
    input.addEventListener("input", function () {
      if (state && input.value !== state.value) clear();
    });
    if (input.form) input.form.addEventListener("submit", clear);
  }
  globalThis.eifgbTab = attachTab;
})();
