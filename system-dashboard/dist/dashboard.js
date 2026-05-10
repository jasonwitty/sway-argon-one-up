// Dashboard renderer. Pulls a Snapshot from the Tauri backend every 2s
// while focused, every 10s while blurred. Click handlers shell out to
// existing scripts via Tauri commands. The theme stylesheet is reloaded
// when the backend emits "theme-stylesheet-changed".

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const POLL_FOCUS_MS = 2000;
const POLL_BLUR_MS = 10000;

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

let pollHandle = null;

async function refresh() {
  try {
    const s = await invoke("snapshot");
    renderBattery(s.battery);
    renderPower(s.power);
    renderIdle(s.idle);
    renderTheme(s.theme);
    renderFan(s.fan);
  } catch (e) {
    console.error("snapshot failed", e);
  }
}

function renderBattery(b) {
  const card = document.getElementById("card-battery");
  const value = document.getElementById("battery-value");
  const sub = document.getElementById("battery-sub");
  const bar = document.getElementById("battery-bar");

  value.textContent = `${b.percent}%`;
  bar.style.width = `${b.percent}%`;

  card.classList.remove("warn", "crit", "charging");
  if (b.charging) {
    card.classList.add("charging");
  } else if (b.percent < 20) {
    card.classList.add("crit");
  } else if (b.percent < 40) {
    card.classList.add("warn");
  }

  if (b.charging) {
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

function renderTheme(t) {
  document.getElementById("theme-value").textContent = pretty(t.name);
  applyThemeStylesheet(t.stylesheet);
  renderSwatches();
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

function renderSwatches() {
  const target = document.getElementById("theme-swatches");
  if (target.children.length > 0) return;
  const vars = [
    "--accent",
    "--power",
    "--idle",
    "--battery-good",
    "--fan-turbo",
  ];
  for (const v of vars) {
    const dot = document.createElement("span");
    dot.className = "swatch";
    dot.style.background = `var(${v})`;
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

(async () => {
  await listen("theme-stylesheet-changed", refresh);
  await refresh();
  startPolling(POLL_FOCUS_MS);
})();
