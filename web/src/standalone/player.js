// The player inside a game downloaded as a single web page (see
// ../standalone.ts, which builds the page). Plain JavaScript, run as an
// ordinary script, so the page works opened straight from a disk, with no
// server and no internet. The page holds, in <script> elements: the
// game's files (#eifgb-game, JSON) and the engine's WebAssembly (#eifgb-wasm,
// base64); the engine's loader has already set window.createEifgb.
//
// Like the site's player, every action is typed. Progress is kept in this
// browser, so closing the page and opening it again carries on; SAVE and
// RESTORE keep one saved place too.
(function () {
  "use strict";
  var $ = function (id) {
    return document.getElementById(id);
  };
  var log = $("log");
  var form = $("prompt");
  var line = $("line");
  var statusLine = $("status");
  var history = [];
  var back = 0;
  var engine = null;
  var key = "eifgb-page:";
  var gameFiles = {};

  function say(text, cls) {
    var p = document.createElement("p");
    p.textContent = text;
    if (cls) p.className = cls;
    log.append(p);
    return p;
  }

  // A picture: from the game's own files (kept inside this page) or the web.
  function picture(l) {
    var f = document.createElement("figure");
    f.className = "picture";
    var file = l.picture || "";
    var src = /^https:\/\//i.test(file) ? file : (gameFiles[file] || "").indexOf("data:image/") === 0 ? gameFiles[file] : null;
    if (src) {
      var img = document.createElement("img");
      img.src = src;
      img.alt = l.text;
      f.append(img);
    } else {
      var p = document.createElement("p");
      p.className = "notice";
      p.textContent = "[Picture: " + l.text + "]";
      f.append(p);
    }
    log.append(f);
    return f;
  }

  // Kept in this browser; private windows and some local files can't keep
  // anything, and then the game just starts afresh each time.
  function remember(name, value) {
    try {
      if (value === null) localStorage.removeItem(key + name);
      else localStorage.setItem(key + name, value);
    } catch (e) {
      /* nothing to do */
    }
  }
  function recall(name) {
    try {
      return localStorage.getItem(key + name);
    } catch (e) {
      return null;
    }
  }

  function show(o) {
    var first = null;
    for (var i = 0; i < o.lines.length; i++) {
      var l = o.lines[i];
      var p = l.kind === "picture" ? picture(l) : say(l.text, l.kind === "text" ? "" : l.kind);
      first = first || p;
    }
    var s = o.status;
    var parts = [s.room];
    if (s.max_score !== null || s.score) parts.push(s.max_score !== null ? "Score " + s.score + "/" + s.max_score : "Score " + s.score);
    if (s.time) parts.push(s.time);
    parts.push("Turn " + s.turns);
    statusLine.textContent = parts.filter(Boolean).join(" · ");
    return first;
  }

  // Scrolls to show what's new, but never past where it starts.
  function reveal(start) {
    if (!start) return;
    var top = start.getBoundingClientRect().top + window.scrollY - 12;
    var bottom = document.documentElement.scrollHeight - window.innerHeight;
    window.scrollTo({ top: Math.max(0, Math.min(top, bottom)) });
  }

  function handle(o) {
    if (o.save) remember("save", o.save);
    if (o.restore_requested) {
      var saved = recall("save");
      if (saved) show(engine.restore(saved));
      else say("There's no saved game yet. Type SAVE to keep your place.", "notice");
    }
    // Finished games start afresh next time; anything else carries on.
    remember("auto", o.ended || o.quit_requested ? null : engine.save());
  }

  function start(fresh) {
    log.replaceChildren();
    gameFiles = JSON.parse($("eifgb-game").textContent);
    var loaded = engine.load(gameFiles, "game.yaml", Math.floor(Math.random() * 4294967296));
    if (!loaded.output) {
      say("This game has mistakes and can't be played:", "error");
      loaded.diagnostics.forEach(function (d) {
        if (d.severity === "error") say((d.path ? d.path + ": " : "") + d.message, "error");
      });
      return false;
    }
    var meta = loaded.meta;
    key = "eifgb-page:" + meta.id + ":";
    document.title = meta.title;
    $("title").textContent = meta.title;
    var look = meta.presentation;
    if (look && look.theme) document.documentElement.dataset.theme = look.theme;
    if (look && look.accent) document.documentElement.style.setProperty("--accent", look.accent);
    var auto = fresh ? null : recall("auto");
    if (auto) {
      say("Carrying on where you left off. (Type RESTART to begin again.)", "notice");
      show(engine.restore(auto));
    } else show(loaded.output);
    return true;
  }

  form.addEventListener("submit", function (e) {
    e.preventDefault();
    var text = line.value.trim();
    line.value = "";
    if (text) {
      history.push(text);
      back = 0;
    }
    var echo = say(text, "cmd");
    var o = engine.input(text);
    show(o);
    handle(o);
    reveal(echo);
  });
  // Tab finishes a word (see ../tab.js, inlined before this script).
  window.eifgbTab(line, function (text) {
    return engine ? engine.complete(text) : [];
  });
  line.addEventListener("keydown", function (e) {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    back = Math.max(0, Math.min(history.length, back + (e.key === "ArrowUp" ? 1 : -1)));
    line.value = back ? history[history.length - back] : "";
  });
  log.addEventListener("click", function () {
    var sel = window.getSelection ? String(window.getSelection()) : "";
    if (!sel && !form.hidden) line.focus({ preventScroll: true });
  });

  var wasm = $("eifgb-wasm").textContent.trim();
  var bin = atob(wasm);
  var bytes = new Uint8Array(bin.length);
  for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  // The engine's own loader would fetch the WebAssembly; here it's handed over.
  var instantiateWasm = function (imports, done) {
    WebAssembly.instantiate(bytes, imports).then(function (r) {
      done(r.instance, r.module);
    });
    return {};
  };
  window
    .createEifgb({ instantiateWasm: instantiateWasm })
    .then(function (m) {
      var call = function (name, types, args) {
        return JSON.parse(m.UTF8ToString(m.ccall(name, "number", types, args)));
      };
      engine = {
        load: function (files, entry, seed) {
          return call("eifgb_load", ["string", "string", "number"], [JSON.stringify(files), entry, seed]);
        },
        input: function (text) {
          return call("eifgb_input", ["string"], [text]);
        },
        save: function () {
          return call("eifgb_save", [], []);
        },
        restore: function (save) {
          return call("eifgb_restore", ["string"], [save]);
        },
        complete: function (text) {
          return call("eifgb_complete", ["string"], [text]);
        },
      };
      if (start(false)) {
        form.hidden = false;
        line.focus();
      }
    })
    .catch(function (e) {
      log.replaceChildren();
      say("This game couldn't start in this browser: " + e, "error");
    });
})();
