#!/usr/bin/env bash
# Linux side of IrisPhone. Embedded in irisphone.exe and written to the IrisPhone data
# directory at runtime; runs inside WSL (Windows) or directly on Linux.
# Drives xtool (https://github.com/xtool-org/xtool) to build, sign and install iOS apps.
set -euo pipefail

# On WSL, the iPhone USB service comes from the Windows "Apple Devices" app and listens on
# 127.0.0.1:27015. With networkingMode=mirrored, that localhost is shared with Linux.
if grep -qi microsoft /proc/version 2>/dev/null; then
    export USBMUXD_SOCKET_ADDRESS="${USBMUXD_SOCKET_ADDRESS:-127.0.0.1:27015}"
fi

# Builds run on the Linux filesystem: much faster than /mnt/c.
BUILD_ROOT="${IRISPHONE_BUILD_ROOT:-$HOME/.cache/irisphone/build}"

die() { echo "error: $*" >&2; exit 1; }

load_swift_env() {
    # Non-login shells (and fresh installs) don't have swiftly on PATH yet.
    if ! command -v swift >/dev/null 2>&1; then
        local env="${SWIFTLY_HOME_DIR:-$HOME/.local/share/swiftly}/env.sh"
        # shellcheck disable=SC1090
        [ -f "$env" ] && . "$env"
    fi
}

need_tools() {
    load_swift_env
    command -v swift >/dev/null || die "Swift not found. Run: irisphone setup"
    command -v xtool >/dev/null || die "xtool not found. Run: irisphone setup"
}

# Copy the project to the Linux disk, keeping the previous build cache.
# The cache key includes a hash of the source path so two apps with the same name don't collide.
sync_app() {
    local src="$1" name key
    [ -f "$src/xtool.yml" ] || die "$src is not an IrisPhone/xtool project (xtool.yml missing)."
    name="$(basename "$src")"
    key="$(printf '%s' "$src" | md5sum | cut -c1-8)"
    mkdir -p "$BUILD_ROOT/$name-$key"
    rsync -a --delete \
        --exclude .build --exclude xtool --exclude .swiftpm --exclude .bsp --exclude .git \
        "$src/" "$BUILD_ROOT/$name-$key/"
    echo "$BUILD_ROOT/$name-$key"
}

# Where xtool puts the built .app / .ipa (copied back next to the sources).
copy_back() {
    local dir="$1" src="$2"
    if [ -d "$dir/xtool" ]; then
        mkdir -p "$src/xtool"
        rsync -a --delete "$dir/xtool/" "$src/xtool/" 2>/dev/null || true
    fi
}

cmd="${1:-help}"
shift || true

case "$cmd" in
    install-swift)
        version="${1:-latest}"
        load_swift_env
        if ! command -v swiftly >/dev/null 2>&1; then
            echo "Installing swiftly..."
            tmp="$(mktemp -d)"
            curl -fsSL "https://download.swift.org/swiftly/linux/swiftly-$(uname -m).tar.gz" -o "$tmp/swiftly.tar.gz"
            tar -xzf "$tmp/swiftly.tar.gz" -C "$tmp"
            "$tmp/swiftly" init --assume-yes --quiet-shell-followup --skip-install
            rm -rf "$tmp"
            load_swift_env
            # shellcheck disable=SC1090
            . "${SWIFTLY_HOME_DIR:-$HOME/.local/share/swiftly}/env.sh"
        fi
        if swift --version 2>/dev/null | grep -q "Swift version $version"; then
            echo "Swift $version already installed."
        else
            swiftly install "$version" --use --assume-yes
        fi
        hash -r
        swift --version | head -1
        ;;
    sdk-install)
        need_tools
        xip="${1:?path to Xcode .xip required}"
        xtool sdk install "$xip"
        ;;
    sdk-status)
        need_tools
        xtool sdk status
        ;;
    login)
        need_tools
        if [ -n "${1:-}" ]; then
            xtool auth login --mode password --username "$1"
        else
            xtool auth login --mode password
        fi
        ;;
    logout)
        need_tools
        xtool auth logout
        ;;
    auth-status)
        need_tools
        xtool auth status
        ;;
    devices)
        need_tools
        xtool devices --usb --no-wait
        ;;
    pair)
        if idevicepair validate >/dev/null 2>&1; then
            echo "iPhone already paired with this computer."
            exit 0
        fi
        echo "Unlock the iPhone, tap 'Trust' and enter your passcode..."
        out=""
        for _ in $(seq 1 30); do
            out="$(idevicepair pair 2>&1)" && { echo "$out"; exit 0; }
            case "$out" in
                *denied*)
                    echo "The iPhone refused (the refusal is remembered). On the iPhone:"
                    echo "  Settings > General > Transfer or Reset iPhone > Reset > Reset Location & Privacy,"
                    echo "  then unplug and plug the cable back in."
                    exit 1 ;;
            esac
            sleep 2
        done
        echo "$out"; exit 1
        ;;
    build)
        need_tools
        src="${1:?project directory required}"; shift
        dir="$(sync_app "$src")"
        echo "@@irisphone-build-dir $dir"
        cd "$dir"
        xtool dev build "$@"
        copy_back "$dir" "$src"
        ;;
    deploy)
        need_tools
        src="${1:?project directory required}"; shift
        dir="$(sync_app "$src")"
        echo "@@irisphone-build-dir $dir"
        cd "$dir"
        xtool dev run --usb "$@"
        ;;
    launch)
        need_tools
        xtool launch --usb "${1:?bundle id required}"
        ;;
    uninstall)
        need_tools
        xtool uninstall --usb "${1:?bundle id required}"
        ;;
    logs)
        # logs <process name> [seconds]  (seconds=0: follow until Ctrl+C)
        process="${1:?process name required}"
        seconds="${2:-0}"
        command -v idevicesyslog >/dev/null || die "idevicesyslog missing. Run: irisphone setup"
        if [ "$seconds" = "0" ]; then
            exec idevicesyslog --no-colors --process "$process"
        else
            timeout "$seconds" idevicesyslog --no-colors --process "$process" || true
        fi
        ;;
    doctor)
        # Machine-readable: one key=value per line (parsed by irisphone.exe).
        load_swift_env
        echo "swift=$(swift --version 2>/dev/null | head -1 | sed -n 's/.*Swift version \([^ ]*\).*/\1/p')"
        echo "xtool=$(xtool --version 2>/dev/null | awk '{print $NF}')"
        if command -v idevice_id >/dev/null; then echo "usbmux_tools=ok"; else echo "usbmux_tools=missing"; fi
        if xtool sdk status 2>/dev/null | grep -q "is installed"; then echo "sdk=ok"; else echo "sdk=missing"; fi
        if [ -n "${USBMUXD_SOCKET_ADDRESS:-}" ]; then
            if timeout 3 bash -c "echo > /dev/tcp/${USBMUXD_SOCKET_ADDRESS%:*}/${USBMUXD_SOCKET_ADDRESS#*:}" 2>/dev/null; then
                echo "usbmux=ok"
            else
                echo "usbmux=unreachable"
            fi
        elif [ -S /var/run/usbmuxd ]; then
            echo "usbmux=ok"
        else
            echo "usbmux=unreachable"
        fi
        echo "auth=$(xtool auth status 2>/dev/null | head -1 | tr -d '\r')"
        devs="$(idevice_id -l 2>/dev/null | head -1 || true)"
        echo "device=${devs:-none}"
        if [ -n "$devs" ]; then
            if idevicepair validate >/dev/null 2>&1; then echo "paired=yes"; else echo "paired=no"; fi
            echo "device_name=$(ideviceinfo -k DeviceName 2>/dev/null || true)"
            echo "ios_version=$(ideviceinfo -k ProductVersion 2>/dev/null || true)"
            echo "developer_mode=$(ideviceinfo -q com.apple.security.mac.amfi -k DeveloperModeStatus 2>/dev/null || true)"
        fi
        ;;
    *)
        echo "Usage: irisphone.sh {doctor|install-swift|sdk-install <xip>|sdk-status|login|logout|auth-status|devices|pair|build <dir>|deploy <dir>|launch <id>|uninstall <id>|logs <process> [s]}"
        ;;
esac
