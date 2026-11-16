#!/usr/bin/env bash
# System packages for IrisPhone (run as root: `wsl -u root` on Windows, sudo on Linux).
# Embedded in irisphone.exe. Targets Ubuntu/Debian.
set -euo pipefail

XTOOL_VERSION="${1:?xtool version required}"

command -v apt-get >/dev/null || { echo "error: only Ubuntu/Debian are supported for now (apt-get missing)." >&2; exit 1; }

export DEBIAN_FRONTEND=noninteractive
echo "Installing system packages (apt)..."
apt-get update -qq
# usbmuxd/libimobiledevice: talk to the iPhone. libfuse2: run the xtool AppImage.
# The rest: what the Swift toolchain needs on Ubuntu 24.04.
fuse_pkg=libfuse2t64
apt-cache show "$fuse_pkg" >/dev/null 2>&1 || fuse_pkg=libfuse2
# libstdc++/libgcc dev packages are versioned: pick the newest one this release has.
cxx=""
for v in 14 13 12 11; do
    if apt-cache show "libstdc++-$v-dev" >/dev/null 2>&1; then cxx="libstdc++-$v-dev libgcc-$v-dev"; break; fi
done
# shellcheck disable=SC2086
apt-get install -y -qq --no-install-recommends     usbmuxd libimobiledevice-utils "$fuse_pkg" rsync curl ca-certificates zip unzip     binutils git gnupg2 libc6-dev libcurl4-openssl-dev libedit2 libncurses-dev     libpython3-dev libsqlite3-0 libxml2-dev libz3-dev pkg-config tzdata zlib1g-dev $cxx >/dev/null

current="$(/usr/local/bin/xtool --version 2>/dev/null | awk '{print $NF}' || true)"
if [ "$current" != "$XTOOL_VERSION" ]; then
    echo "Installing xtool $XTOOL_VERSION..."
    curl -fsSL "https://github.com/xtool-org/xtool/releases/download/$XTOOL_VERSION/xtool-$(uname -m).AppImage" \
        -o /usr/local/bin/xtool.tmp
    chmod +x /usr/local/bin/xtool.tmp
    mv /usr/local/bin/xtool.tmp /usr/local/bin/xtool
fi
echo "xtool $(/usr/local/bin/xtool --version | awk '{print $NF}')"
