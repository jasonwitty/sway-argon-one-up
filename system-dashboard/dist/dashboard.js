// Dashboard renderer. Pulls a Snapshot from the Tauri backend every 2s
// while focused, every 10s while blurred. Click handlers shell out to
// existing scripts via Tauri commands. The theme stylesheet is reloaded
// when the backend emits "theme-stylesheet-changed".

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const POLL_FOCUS_MS = 2000;
const POLL_BLUR_MS = 10000;

const BATTERY_GLYPH_DISCHARGE = {
  100: "\u{F0079}",
  90: "\u{F0082}",
  80: "\u{F0081}",
  70: "\u{F0080}",
  60: "\u{F007F}",
  50: "\u{F007E}",
  40: "\u{F007D}",
  30: "\u{F007C}",
  20: "\u{F007B}",
  10: "\u{F007A}",
  0: "\u{F008E}", // battery-outline (near-empty)
};

const BATTERY_GLYPH_CHARGING = "\u{F0084}"; // battery-charging

function batteryIcon(percent, charging, full) {
  if (full) return BATTERY_GLYPH_DISCHARGE[100];
  if (charging) return BATTERY_GLYPH_CHARGING;
  const tier = Math.min(100, Math.max(0, Math.floor(percent / 10) * 10));
  return BATTERY_GLYPH_DISCHARGE[tier] || BATTERY_GLYPH_DISCHARGE[10];
}

const FAN_GLYPH = {
  silent: "\u{F0A9F}",
  normal: "\u{F0210}",
  turbo: "\u{F01FB}",
  full: "\u{F1A88}",
  offline: "\u{F020E}",
};

const IDLE_GLYPH = {
  on: "\u{F033E}", // lock
  off: "\u{F033F}", // lock-open
  "claude-active": "\u{F05DA}", // brain (working)
  "claude-idle": "\u{F04B2}", // sleep (idle)
};

const POWER_LABEL = {
  performance: "Performance",
  ondemand: "Balanced",
  powersave: "Powersave",
};

const POWER_GLYPH = {
  performance: "\u{F140B}", // lightning-bolt
  ondemand: "\u{F1F85}",    // gauge / balanced
  powersave: "\u{F032A}",   // leaf / battery-save
};

let pollHandle = null;

async function refresh() {
  try {
    const s = await invoke("snapshot");
    renderBattery(s.battery);
    renderPower(s.power);
    renderIdle(s.idle);
    renderTheme(s.theme);
    renderFan(s.fan);
    renderBrightness(s.brightness);
    renderVolume(s.volume);
    renderTrackpad(s.trackpad);
  } catch (e) {
    console.error("snapshot failed", e);
  }
}

function renderBattery(b) {
  const card = document.getElementById("card-battery");
  const value = document.getElementById("battery-value");
  const sub = document.getElementById("battery-sub");
  const bar = document.getElementById("battery-bar");
  const icon = document.getElementById("battery-icon");

  const full = b.percent >= 100;

  value.textContent = `${b.percent}%`;
  bar.style.width = `${b.percent}%`;
  icon.textContent = batteryIcon(b.percent, b.charging, full);

  card.classList.remove("warn", "crit", "charging");
  if (full) {
    // Fully charged: stay on the default green/good styling regardless
    // of the charging flag (CW2217 still reports charging=true on AC).
  } else if (b.charging) {
    card.classList.add("charging");
  } else if (b.percent < 20) {
    card.classList.add("crit");
  } else if (b.percent < 40) {
    card.classList.add("warn");
  }

  if (full) {
    sub.textContent = b.charging ? "Fully charged" : "Full";
  } else if (b.charging) {
    sub.textContent = "Charging";
  } else if (b.time_remaining_min != null) {
    const h = Math.floor(b.time_remaining_min / 60);
    const m = b.time_remaining_min % 60;
    sub.textContent = h > 0 ? `${h}h ${m}m remaining` : `${m}m remaining`;
  } else {
    sub.textContent = "On battery";
  }
}

function renderPower(p) {
  document.getElementById("power-value").textContent =
    POWER_LABEL[p.governor] || p.governor;
  document.getElementById("power-sub").textContent =
    `${p.governor} · ${p.on_ac ? "on AC" : "on battery"}`;
  const icon = POWER_GLYPH[p.governor];
  if (icon) document.getElementById("power-icon").textContent = icon;
}

function renderIdle(i) {
  const card = document.getElementById("card-idle");
  const value = document.getElementById("idle-value");
  const sub = document.getElementById("idle-sub");
  const icon = document.getElementById("idle-icon");

  card.classList.remove("on", "off", "claude");

  switch (i.mode) {
    case "on":
      card.classList.add("on");
      icon.textContent = IDLE_GLYPH.on;
      value.textContent = "Auto-lock on";
      sub.textContent = "Lock at 5m · display off at 10m";
      break;
    case "off":
      card.classList.add("off");
      icon.textContent = IDLE_GLYPH.off;
      value.textContent = "Disabled";
      sub.textContent = "Screen will not auto-lock";
      break;
    case "claude":
      card.classList.add("claude");
      if (i.claude_state === "active") {
        icon.textContent = IDLE_GLYPH["claude-active"];
        value.textContent = "Auto (Claude working)";
        sub.textContent = "Lock inhibited while Claude streams";
      } else {
        icon.textContent = IDLE_GLYPH["claude-idle"];
        value.textContent = "Auto (Claude idle)";
        sub.textContent = "Locks normally; pauses when Claude works";
      }
      break;
    default:
      value.textContent = i.mode || "unknown";
      sub.textContent = "";
  }
}

let lastThemeName = null;

function renderTheme(t) {
  document.getElementById("theme-value").textContent = pretty(t.name);
  applyThemeStylesheet(t.stylesheet);
  // Rebuild swatches from the actual stylesheet on every theme change so
  // the dots reflect whatever palette is currently rendered (not the
  // fallback baked into the bundled CSS).
  if (t.name !== lastThemeName) {
    renderSwatches(t.stylesheet);
    lastThemeName = t.name;
  }
}

function pretty(name) {
  if (!name) return "—";
  return name
    .split("-")
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}

function applyThemeStylesheet(css) {
  let el = document.getElementById("theme-stylesheet-inline");
  if (!el) {
    el = document.createElement("style");
    el.id = "theme-stylesheet-inline";
    document.head.appendChild(el);
  }
  el.textContent = css || "";
}

function renderSwatches(stylesheet) {
  const target = document.getElementById("theme-swatches");
  target.innerHTML = "";
  const vars = [
    "--accent",
    "--power",
    "--idle",
    "--battery-good",
    "--fan-turbo",
  ];
  for (const v of vars) {
    const re = new RegExp(`${v}:\\s*(#[0-9a-fA-F]+)`);
    const m = stylesheet && stylesheet.match(re);
    const color = m ? m[1] : null;
    const dot = document.createElement("span");
    dot.className = "swatch";
    dot.style.background = color || `var(${v})`;
    target.appendChild(dot);
  }
}

function renderFan(f) {
  const card = document.getElementById("card-fan");
  const icon = document.getElementById("fan-icon");
  const value = document.getElementById("fan-value");
  const sub = document.getElementById("fan-sub");

  card.classList.remove("silent", "normal", "turbo", "full", "offline");
  card.classList.add(f.mode);

  icon.textContent = FAN_GLYPH[f.mode] || FAN_GLYPH.offline;

  if (!f.online) {
    value.textContent = "Offline";
    sub.textContent = "argon-fan daemon not running";
    return;
  }

  value.textContent = pretty(f.mode);
  sub.textContent = `${f.temp_c.toFixed(0)}°C · PWM ${f.pwm}/255 (${f.pwm_pct}%)`;
}

function renderBrightness(b) {
  const slider = document.getElementById("brightness-slider");
  if (!sliderIsBeingDragged(slider)) slider.value = String(b.percent);
  document.getElementById("brightness-value").textContent = `${b.percent}%`;
}

function renderVolume(v) {
  const card = document.getElementById("card-volume");
  const slider = document.getElementById("volume-slider");
  const icon = document.getElementById("volume-icon");
  card.classList.toggle("muted", !!v.muted);
  if (!sliderIsBeingDragged(slider)) slider.value = String(v.percent);
  document.getElementById("volume-value").textContent =
    v.muted ? "Muted" : `${v.percent}%`;
  icon.textContent = v.muted
    ? "\u{F075F}" // volume-mute
    : v.percent < 33
      ? "\u{F057F}" // volume-low
      : v.percent < 66
        ? "\u{F0580}" // volume-medium
        : "\u{F057E}"; // volume-high
}

function renderTrackpad(t) {
  const card = document.getElementById("card-trackpad");
  const sw = document.getElementById("trackpad-switch");
  const value = document.getElementById("trackpad-value");
  const sub = document.getElementById("trackpad-sub");
  const slider = document.getElementById("gate-slider");
  card.classList.toggle("on", t.active);
  card.classList.toggle("off", !t.active);
  // Don't fight the user mid-toggle — if they just clicked the switch, the
  // optimistic state is reflected in `dataset.pending` until the next poll.
  if (sw.dataset.pending !== "1") sw.checked = t.active;
  value.textContent = t.active ? "On" : "Off";
  sub.textContent = t.active
    ? "Disabling touchpad while typing"
    : "Always-on touchpad (no DWT)";

  if (!sliderIsBeingDragged(slider)) slider.value = String(t.typing_gate_ms);
  document.getElementById("gate-value").textContent = gateLabel(
    t.typing_gate_ms,
  );
  // The gate only does anything while the guard is running.
  slider.disabled = !t.active;
}

function gateLabel(ms) {
  return ms === 0 ? "Off" : `${ms} ms`;
}

// While the user is dragging the slider, don't fight them by overwriting
// the value from the poll loop.
function sliderIsBeingDragged(slider) {
  return slider.dataset.dragging === "1";
}

function bindSlider(id, action) {
  const slider = document.getElementById(id);
  let pending = null;
  const send = (value) => {
    pending = null;
    invoke(action, { level: value }).catch((e) =>
      console.error(`${action} failed`, e),
    );
  };
  slider.addEventListener("pointerdown", () => (slider.dataset.dragging = "1"));
  slider.addEventListener("pointerup", () => {
    slider.dataset.dragging = "0";
    // Force a refresh shortly after the user releases so the displayed
    // value re-syncs with the device-reported value.
    setTimeout(refresh, 200);
  });
  slider.addEventListener("input", () => {
    const value = Number(slider.value);
    document.getElementById(`${id.split("-")[0]}-value`).textContent =
      `${value}%`;
    // Coalesce: only send the most recent value every ~80ms while dragging.
    if (pending) clearTimeout(pending);
    pending = setTimeout(() => send(value), 80);
  });
}

function startPolling(intervalMs) {
  if (pollHandle) clearInterval(pollHandle);
  pollHandle = setInterval(refresh, intervalMs);
}

window.addEventListener("focus", () => startPolling(POLL_FOCUS_MS));
window.addEventListener("blur", () => startPolling(POLL_BLUR_MS));

document.querySelectorAll(".clickable").forEach((card) => {
  card.addEventListener("click", () => {
    const action = card.dataset.action;
    if (!action) return;
    invoke(action).catch((e) => console.error(`${action} failed`, e));
  });
});

bindSlider("brightness-slider", "set_brightness");
bindSlider("volume-slider", "set_volume");

// The gate slider is deliberately NOT bound with bindSlider: that helper sends
// on every `input` tick (coalesced to ~80ms), which is right for brightness and
// volume but wrong here — each send writes a root-owned config file through
// `sudo tee`, so a single drag would spawn dozens of sudo processes. This fires
// once, on release.
(function bindGateSlider() {
  const slider = document.getElementById("gate-slider");
  const label = document.getElementById("gate-value");
  const release = () => (slider.dataset.dragging = "0");
  slider.addEventListener("pointerdown", () => (slider.dataset.dragging = "1"));
  // Clear the drag flag even if the value never changed (a click that lands on
  // the current value fires no `change`), or the poll loop would stop
  // re-syncing this slider for the rest of the session.
  slider.addEventListener("pointerup", release);
  slider.addEventListener("pointercancel", release);
  slider.addEventListener("input", () => {
    label.textContent = gateLabel(Number(slider.value));
  });
  slider.addEventListener("change", () => {
    invoke("set_typing_gate", { level: Number(slider.value) })
      .catch((e) => console.error("set_typing_gate failed", e))
      .finally(() => {
        release();
        // The daemon re-reads the file on its next 1s tick; re-sync after that
        // so the displayed value is what the daemon actually adopted (it
        // clamps out-of-range values rather than rejecting them).
        setTimeout(refresh, 1200);
      });
  });
})();

(function bindTrackpadSwitch() {
  const sw = document.getElementById("trackpad-switch");
  sw.addEventListener("change", () => {
    sw.dataset.pending = "1";
    invoke("toggle_trackpad_guard")
      .catch((e) => {
        console.error("toggle_trackpad_guard failed", e);
        sw.checked = !sw.checked; // revert
      })
      .finally(() => {
        // Re-sync from backend shortly after; systemd takes ~100-300ms.
        setTimeout(() => {
          sw.dataset.pending = "0";
          refresh();
        }, 400);
      });
  });
})();

// Register the theme-change listener but don't block startup on it.
listen("theme-stylesheet-changed", refresh).catch((e) =>
  console.error("listen failed", e),
);

// Start polling first so the dashboard catches up even if the very first
// refresh fires before Tauri's IPC bridge is ready.
startPolling(POLL_FOCUS_MS);
refresh();
