# Builds the panels' Quickshell binary in a clean Debian trixie
# container, so the result links against stock trixie Qt (6.8.2) and can be
# published as a release asset. The recipe itself lives in build-quickshell.sh.
FROM debian:trixie

ARG QUICKSHELL_REF=v0.3.2

COPY build-quickshell.sh /build-quickshell.sh
RUN /build-quickshell.sh --install-deps --prefix /out --ref "$QUICKSHELL_REF" \
    && rm -rf /var/lib/apt/lists/*
