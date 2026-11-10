(function () {
  "use strict";

  var host = document.getElementById("vttyHost");
  var form = document.getElementById("bbForm");
  var input = document.getElementById("bbInput");
  if (!host || !form || !input) return;

  var term = document.createElement("pre");
  term.className = "term";
  host.replaceChildren(term);

  var history = [];
  var ESC = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };
  var esc = function (value) { return String(value).replace(/[&<>"]/g, function (ch) { return ESC[ch]; }); };
  var span = function (cls, text) { return '<span class="' + cls + '">' + esc(text) + "</span>"; };
  var cyan = function (text) { return span("cyan", text); };
  var amber = function (text) { return span("amber", text); };
  var muted = function (text) { return span("muted", text); };
  var red = function (text) { return span("red", text); };
  var bold = function (text) { return '<span class="bold">' + esc(text) + "</span>"; };
  var randomInt = function (min, max) {
    return Math.floor(Math.random() * (max - min + 1)) + min;
  };
  var spark = function (length) {
    var chars = "▁▂▃▅▇█";
    var out = "";
    for (var i = 0; i < length; i += 1) out += chars[randomInt(0, chars.length - 1)];
    return out;
  };
  var braille = function (length) {
    var chars = "⢀⣀⣤⣶⣿";
    var out = "";
    for (var i = 0; i < length; i += 1) out += chars[randomInt(0, chars.length - 1)];
    return out;
  };
  var meter = function (pct, width) {
    var filled = Math.max(1, Math.min(width - 1, Math.round(width * pct / 100)));
    return "━".repeat(filled - 1) + "╸" + "━".repeat(width - filled);
  };
  var cmd = function (text) { return span("cmd", text); };
  var num = function (text) { return span("num", text); };
  var label = function (text) { return span("label", text); };
  var chart = function (tone, text) { return span("chart-" + tone, text); };

  var prompt = function () {
    return cyan("jet@bebop") + " " + muted("~") + " " + amber("❯");
  };

  var intro = function () {
    return [
      amber("SESSION") + " / 2071.04.03",
      cyan("jet.fish") + " v0.0.0 / demo session"
    ];
  };

  var mock = function (command) {
    var name = command.trim().toLowerCase().split(/\s+/)[0];
    if (name === "help") {
      var row = function (key, description) {
        return "    " + cmd(key.padEnd(14)) + muted(description);
      };
      return [
        bold("jet.fish"),
        muted("A native terminal for the long way home"), "",
        bold("Commands:"),
        row("session", "Show session info"),
        row("bounty", "Show the bounty board"),
        row("logo", "Print the mark"),
        row("power", "Show vtty + logo"),
        row("whoami", "Print current user"),
        row("monitor", "Monitor the Bebop"),
        row("clear", "Clear the screen"),
        row("help", "Print this help")
      ];
    }
    if (name === "logo") {
      return [
        '<div class="term-logo-copy" data-term-logo></div>'
      ];
    }
    if (name === "bounty" || name === "manifest") {
      return [
        bold("BOUNTY BOARD"), "",
        amber("01") + "  the long way home       " + cyan("OPEN"),
        amber("02") + "  a fish out of water     " + cyan("OPEN"),
        amber("03") + "  signal from the edge    " + red("LOST")
      ];
    }
    if (name === "session") {
      var jsonLine = function (key, value, last) {
        return "  " + muted('"') + cmd(key) + muted('"') +
          muted(": ") + cyan('"' + value + '"') +
          (last ? "" : muted(","));
      };
      return [
        muted("{"),
        jsonLine("session", "2071.04.03"),
        jsonLine("vessel", "Bebop"),
        jsonLine("mode", "demo"),
        jsonLine("status", "open", true),
        muted("}")
      ];
    }
    if (name === "power") {
      return [
        '<div class="term-power"><a class="term-vtty-link" href="https://github.com/pindash-io/vtty" target="_blank" rel="noopener"><img class="term-vtty-logo" src="/assets/img/vtty-logo.svg" alt="vtty"></a></div><a class="term-link" href="https://github.com/pindash-io/vtty" target="_blank" rel="noopener">vtty</a> ' + muted("VT500 parser")
      ];
    }
    if (name === "monitor") {
      var authored = document.documentElement.getAttribute("data-monitor") === "authored";
      var core = randomInt(38, 92);
      var fuel = randomInt(24, 97);
      var bounty = randomInt(1, 9);
      var bountyFill = Math.round(17 * bounty / 10);
      var bountyBar = "█".repeat(bountyFill) + "▉".repeat(17 - bountyFill);
      var boxV = '<span class="muted box-v">│</span>';
      var monitorRow = function (name, body, value, tone) {
        var nameText = name.padEnd(9);
        var bodyText = value ? body.padEnd(23) : body.padEnd(27);
        var bodyHtml = chart(tone, bodyText);
        if (value) {
          return boxV + " " + label(nameText) + bodyHtml +
            num(String(value).padStart(4)) + " " + boxV;
        }
        return boxV + " " + label(nameText) + bodyHtml + " " + boxV;
      };
      var header = muted("╭─ ") + label("BEBOP") +
        muted(authored ? " ───────── " : " ───────────────── ") +
        num("2071.04.03") +
        (authored ? muted(" / ") + num("087.4") : "") +
        muted(" ─╮");
      var lines = [
        header,
        monitorRow("core", spark(20), core + "%", "core"),
        monitorRow("fuel", meter(fuel, 18), fuel + "%", "fuel"),
        monitorRow("signal", braille(13), "", "signal"),
        monitorRow("load", spark(17), "", "load"),
        monitorRow("bounty", bountyBar, String(bounty).padStart(2, "0"), "bounty"),
        muted("╰──────────────────────────────────────╯")
      ];
      return ['<div class="term-monitor">' + lines.join("\n") + "</div>"];
    }
    if (name === "whoami") return ["jet"];
    if (name === "clear") return ["__clear__"];
    if (!name) return [];
    return [red("command not found:") + " " + esc(command)];
  };

  var render = function () {
    var lines = intro();
    if (!history.length) {
      lines.push(
        "",
        muted("try ") + cmd("help") + muted(", ") + cmd("monitor") + muted(", or ") + cmd("logo")
      );
    }
    history.forEach(function (entry) { lines = lines.concat(entry); });
    term.innerHTML = lines.join("\n");
    var slot = term.querySelector("[data-term-logo]");
    var marks = document.querySelectorAll(".bs-brand .bs-mark");
    if (slot && marks.length) {
      marks.forEach(function (mark) { slot.appendChild(mark.cloneNode(true)); });
    }
    host.scrollTop = host.scrollHeight;
  };

  var push = function (command) {
    if (command === "__clear__") {
      history.length = 0;
      render();
      return;
    }
    var result = mock(command);
    if (result[0] === "__clear__") {
      history.length = 0;
      render();
      return;
    }
    history.push([prompt() + " " + cmd(command)].concat(result));
    render();
  };

  var measure = function () {
    var probe = document.createElement("span");
    probe.className = "vtty-probe";
    probe.textContent = "0";
    host.appendChild(probe);
    var rect = probe.getBoundingClientRect();
    host.removeChild(probe);
    document.documentElement.style.setProperty("--vtty-cell-w", (rect.width || 8.4) + "px");
    document.documentElement.style.setProperty("--vtty-cell-h", (rect.height || 16.8) + "px");
  };

  /* font-display: swap means the first measure can land on the fallback.
     Re-measure once the webfonts settle so the grid matches the visible face. */
  if (document.fonts && document.fonts.ready) {
    var metricsSynced = false;
    var syncMetrics = function () {
      if (metricsSynced) return;
      metricsSynced = true;
      measure();
      render();
    };
    document.fonts.ready.then(syncMetrics);
    if (document.fonts.addEventListener) {
      document.fonts.addEventListener("loadingdone", syncMetrics);
    }
  }

  form.addEventListener("submit", function (event) {
    event.preventDefault();
    var command = input.value.trim();
    input.value = "";
    push(command || "");
    input.focus();
  });

  host.addEventListener("click", function () { input.focus(); });

  window.addEventListener("jetfish:download", function (event) {
    history.push([amber("download") + " " + esc(event.detail || "") + " / demo"]);
    render();
  });

  window.addEventListener("jetfish:theme", function () { render(); });

  render();
  measure();
})();
