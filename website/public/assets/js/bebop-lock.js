(function () {
  "use strict";

  var host = document.getElementById("vttyHost");
  if (!host) return;
  var started = false;

  function boot() {
    if (started) return;
    var term = host.querySelector("pre.term");
    if (!term) return;
    started = true;

    var reduce = false;
    try {
      reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    } catch (error) {}

    var line = document.createElement("div");
    line.className = "bs-lockline";
    line.setAttribute("aria-hidden", "true");

    var label = document.createElement("span");
    label.className = "lock-label";
    label.textContent = "uplink";

    var bars = document.createElement("span");
    bars.className = "lock-bars";
    bars.textContent = "▁▁▁";

    var state = document.createElement("span");
    state.className = "lock-state";
    state.textContent = "linking";

    line.appendChild(label);
    line.appendChild(document.createTextNode("  "));
    line.appendChild(bars);
    line.appendChild(document.createTextNode("  "));
    line.appendChild(state);

    host.insertBefore(line, term);

    var input = document.getElementById("bbInput");
    var locked = false;

    function lock() {
      if (locked) return;
      locked = true;
      bars.textContent = "▂▄▆";
      state.textContent = "locked";
      line.classList.add("is-locked");
    }

    var frames = ["▂▁▁", "▂▄▁", "▂▄▆"];
    var i = 0;
    function step() {
      if (locked) return;
      if (i < frames.length) {
        bars.textContent = frames[i];
        i += 1;
        setTimeout(step, 150);
      } else {
        lock();
      }
    }

    function begin() {
      setTimeout(step, 160);
    }

    if (reduce) {
      lock();
    } else if (document.readyState === "complete") {
      begin();
    } else {
      window.addEventListener("load", begin, { once: true });
    }

    /* If the visitor starts before the handshake resolves, settle immediately. */
    if (input) input.addEventListener("input", lock, { once: true });
    host.addEventListener("click", lock, { once: true });

    /* Read the shell input before vtty-mock clears it. */
    var form = document.getElementById("bbForm");
    if (form) {
      form.addEventListener("submit", function () {
        var command = (input ? input.value.trim() : "").toLowerCase().split(/\s+/)[0];
        if (command === "clear") line.remove();
      }, true);
    }

    return true;
  }

  if (boot()) return;
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot, { once: true });
  } else {
    window.setTimeout(boot, 0);
  }
})();
