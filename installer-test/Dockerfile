# Preliminary smoke test for install.sh.
#
# What this validates:
#   - All apt packages from Phase 3 + 9b exist in the Trixie ARM repo
#   - External download URLs are reachable (system-dashboard release,
#     argon vendor scripts)
#   - The five Rust crates compile cleanly via `cargo check`
#
# What it can't validate:
#   - Anything that needs real hardware (I2C, GPIO, DDC brightness, PWM fan)
#   - Anything that needs systemd-as-pid-1 (systemctl enable / set-default)
#   - The Tauri webview runtime (no display in the container)
#
# Usage:
#   docker build -f installer-test/Dockerfile -t sway-argon-test .
#   docker run --rm sway-argon-test
#
# Builds in ~10-15 minutes on a Pi 5 (cargo deps dominate).

FROM debian:trixie

ENV DEBIAN_FRONTEND=noninteractive

# Bare minimum to bootstrap the test: sudo + curl + ca-certificates + jq + git
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git jq sudo \
    && rm -rf /var/lib/apt/lists/*

# Mirror the install.sh contract: must run as a non-root user with passwordless sudo
RUN useradd -m -s /bin/bash pi \
    && echo "pi ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/pi

USER pi
WORKDIR /home/pi

# Pull the repo
RUN git clone --depth=1 https://github.com/jasonwitty/sway-argon-one-up.git
WORKDIR /home/pi/sway-argon-one-up

# Run the smoke test
COPY --chown=pi:pi installer-test/test.sh /home/pi/test.sh
RUN chmod +x /home/pi/test.sh

CMD ["/home/pi/test.sh"]
