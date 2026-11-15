# Installation Guide

This guide walks you through installing the Sway desktop on an Argon ONE UP CM5 laptop, from a blank NVMe drive to a fully working system. No prior Linux experience is assumed — just follow each step in order.

## What You Need

- **Argon ONE UP CM5 Laptop** with a Raspberry Pi Compute Module 5 installed
- **NVMe drive** (any M.2 2230 or 2242 NVMe drive works)
- **NVMe USB enclosure** (recommended) — a cheap USB enclosure lets you flash the drive from another computer, which is the easiest method
- **Another computer** to flash the drive and follow this guide
- **WiFi or USB ethernet adapter** — you'll need internet access during setup

---

## Step 1: Flash Raspberry Pi OS Lite to the NVMe Drive

1. Install [Raspberry Pi Imager](https://www.raspberrypi.com/software/) on your other computer (Windows, Mac, or Linux).
2. Insert your NVMe drive into the USB enclosure and connect it to your computer.
3. Open Raspberry Pi Imager:
   - Click **Choose Device** and select your Pi model (Raspberry Pi 5 / CM5).
   - Click **Choose OS**, scroll to **Raspberry Pi OS (other)**, and select **Raspberry Pi OS Lite (64-bit)**. This is the minimal image with no desktop — exactly what we need.
   - Click **Choose Storage** and select your NVMe drive.
4. You do not need to change any configuration settings in Imager — you'll set everything up on first boot.
5. Click **Next**, then **Yes** to write the image. Wait for it to finish.
6. Remove the NVMe drive from the enclosure and install it in the Argon ONE UP case (the M.2 slot is under the heatsink plate).
7. Close the case, connect the charger, and power on.

---

## Step 2: First Boot Setup

When the Pi boots for the first time, it will walk you through initial configuration at the command line:

1. **Keyboard layout** — select **English (US)** (or your preferred layout).
2. **Create your user account** — choose a username and password. Remember these — you'll use them to log in.
3. **Network** — if you have a USB ethernet adapter plugged in, it should connect automatically. For WiFi, the system will prompt you to connect. If you have trouble with WiFi at this stage, just plug in a USB ethernet adapter for now — the desktop includes a graphical WiFi tool you can use after installation.

---

## Step 3: Update the System

Once logged in, run:

```bash
sudo apt update && sudo apt full-upgrade -y
```

This installs all available security and package updates. It may take a few minutes.

---

## Step 4: Configure System Settings

Run the Raspberry Pi configuration tool:

```bash
sudo raspi-config
```

Change the following settings:

| Setting | Location in raspi-config | What to set |
|---------|-------------------------|-------------|
| **Hostname** | System Options > Hostname | Choose a name for your laptop (e.g., `argon`) |
| **I2C** | Interface Options > I2C | Enable — required for battery monitoring and brightness control |
| **WLAN Country** | Localisation Options > WLAN Country | Set to your country code (e.g., US) |
| **PCIe Speed** | Advanced Options > PCIe Speed | Set to **PCIe Gen 3** for faster NVMe performance |
| **Expand Filesystem** | Advanced Options > Expand Filesystem | Expand to use the full NVMe drive |

After making all changes, select **Finish** and reboot when prompted:

```bash
sudo reboot
```

---

## Step 5: Run the Installer

After rebooting, log in and run:

```bash
curl -fsSL https://raw.githubusercontent.com/jasonwitty/sway-argon-one-up/main/install.sh | bash
```

The installer will:

1. **Run preflight checks** — verifies you're on RPi OS Lite, have internet, and enough disk space.
2. **Ask about optional packages** — all prompts happen up front before anything is installed. You'll be asked about each of the following:

| Package | What it is | Recommendation |
|---------|-----------|----------------|
| **Brave Origin** | The debloated build of Brave: same privacy-focused browser with built-in ad blocking, without Rewards, Wallet, VPN, Leo AI and the other extras. Fully integrated with the theme switcher — colors update live on theme change. | Recommended as your daily browser |
| **Chromium** | Has the best Wayland screen-sharing support. Also theme-integrated. | Install if you need screen sharing (Slack, Teams) |
| **WebApps** | Pin websites as standalone app windows with their own icons. Useful on ARM where not every app has a native package. | Install if you use web-based tools like Slack or Teams |
| **Flatpak + Bazaar** | Flatpak runs sandboxed apps from Flathub. Bazaar is a graphical app store for browsing them. | Install if you want access to more desktop apps |
| **Claude Code** | AI coding assistant in the terminal. Integrated with Mod+C shortcut and wofi quick-prompt popup. | Install if you do development work |

3. **Install everything** — packages, fonts, shell tools, Argon hardware drivers, theme engine, login screen, and your config. This takes 15–30 minutes depending on your internet speed and which optional packages you selected.

---

## Step 6: Reboot and Log In

When the installer finishes, restart:

```bash
sudo reboot
```

You'll see the login screen (gtkgreet) with a wallpaper. Enter your username and password to launch the Sway desktop.

---

## Upgrading

To upgrade an existing install, run the installer again (same command as Step 5). A re-run:

- rebuilds and reinstalls the Rust daemons and downloads the current system-dashboard, then restarts the services that were running;
- replaces the configs and scripts with the versions from the repository, after copying your existing ones to `~/.config/sway-argon-backup-<date>-<time>/` — compare against that folder to bring back any edits of your own;
- keeps the theme you had selected;
- removes the retired `trackpad-guard` service (typing protection is now libinput's own disable-while-typing);
- if you have the regular Brave browser from an earlier version, asks whether to migrate to Brave Origin (your profile is copied across and the original left untouched) or keep your current Brave.

Reboot afterwards, as for a first install.

---

## Next Steps

- Press **Mod+T** to pick a theme
- Press **Mod+Shift+H** to see all keyboard shortcuts
- Read the [Usage Manual](Usage_Manual.md) for a complete reference

If something isn't working, check the [Troubleshooting Guide](Troubleshooting.md).
