#!/bin/sh
# IrisPhone installer for Linux (Ubuntu/Debian).
#   curl -fsSL https://raw.githubusercontent.com/Iris-universe/IrisPhone/main/install.sh | sh
# Downloads the latest irisphone binary to ~/.local/bin, then runs `irisphone setup`.
# Set IRISPHONE_NO_SETUP=1 to skip the setup step.
set -eu

REPO="Iris-universe/IrisPhone"
case "$(uname -m)" in
    x86_64|amd64) ARCH=x64 ;;
    aarch64|arm64) ARCH=arm64 ;;
    *) echo "error: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
DIR="${IRISPHONE_INSTALL_DIR:-$HOME/.local/bin}"

echo "==> Downloading IrisPhone (linux-$ARCH)..."
mkdir -p "$DIR"
curl -fsSL "https://github.com/$REPO/releases/latest/download/irisphone-linux-$ARCH" -o "$DIR/irisphone.download"
chmod +x "$DIR/irisphone.download"
mv "$DIR/irisphone.download" "$DIR/irisphone"

case ":$PATH:" in
    *":$DIR:"*) ;;
    *) echo "    note: add $DIR to your PATH (e.g. in ~/.profile)"; PATH="$PATH:$DIR" ;;
esac

"$DIR/irisphone" --version
if [ "${IRISPHONE_NO_SETUP:-0}" != "1" ]; then
    # stdin is the pipe from curl: give setup the terminal so it can ask questions.
    if [ -r /dev/tty ]; then
        "$DIR/irisphone" setup < /dev/tty
    else
        "$DIR/irisphone" setup --yes
    fi
fi
