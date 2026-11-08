#!/bin/sh
# Install (or remove) the `rmd` command on macOS and Linux.
#
# The packaged macOS app and the AppImage bundle the rmd helper. Windows users
# get it from the installer; this script wires it into PATH on Unix.
#
# Usage:
#   scripts/install-rmd.sh                 # auto-detect an installed app / build
#   scripts/install-rmd.sh /path/to/rmd    # use a specific helper binary
#   scripts/install-rmd.sh /path/to/Rusty-Notepad.AppImage
#   scripts/install-rmd.sh --uninstall
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

pick_bindir() {
    if [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
        printf '%s\n' /usr/local/bin
    else
        printf '%s\n' "${HOME}/.local/bin"
    fi
}

if [ "${1:-}" = "--uninstall" ] || [ "${1:-}" = "-u" ]; then
    removed=0
    for candidate in "${HOME}/.local/bin/rmd" /usr/local/bin/rmd; do
        if [ -f "$candidate" ] || [ -L "$candidate" ]; then
            rm -f "$candidate" && echo "Removed $candidate" && removed=1
        fi
    done
    [ "$removed" -eq 1 ] || echo "Nothing to remove."
    exit 0
fi

target=${1:-}
if [ -z "$target" ]; then
    for candidate in \
        "/Applications/Rusty Notepad.app" \
        "${HOME}/Applications/Rusty Notepad.app" \
        "${script_dir}/../target/release/rmd" \
        "${script_dir}/../target/debug/rmd"; do
        if [ -e "$candidate" ]; then
            target=$candidate
            break
        fi
    done
fi

if [ -z "$target" ]; then
    echo "Could not find Rusty Notepad. Pass the app, AppImage or rmd path:" >&2
    echo "  $0 '/Applications/Rusty Notepad.app'" >&2
    exit 1
fi

if [ ! -e "$target" ]; then
    echo "Not found: $target" >&2
    exit 1
fi

# Accept the .app bundle directly.
case "$target" in
    *.app) target="${target}/Contents/MacOS/rmd" ;;
esac
if [ ! -e "$target" ]; then
    echo "No rmd helper inside: $target" >&2
    exit 1
fi

bindir=$(pick_bindir)
mkdir -p "$bindir"
launcher="${bindir}/rmd"

# An AppImage is a single file: invoke it with the hidden --rmd mode. A real
# rmd binary is executed directly.
case "$target" in
    *.AppImage | *.appimage)
        {
            echo '#!/bin/sh'
            printf 'exec "%s" --rmd "$@"\n' "$target"
        } >"$launcher"
        ;;
    *)
        {
            echo '#!/bin/sh'
            printf 'exec "%s" "$@"\n' "$target"
        } >"$launcher"
        ;;
esac
chmod +x "$launcher"

echo "Installed $launcher"
echo "  -> $target"

case ":${PATH}:" in
    *":${bindir}:"*) ;;
    *)
        echo
        echo "Add this folder to PATH (once):"
        echo "  export PATH=\"${bindir}:\$PATH\""
        echo "Put that line in ~/.zshrc (macOS) or ~/.bashrc (Linux)."
        ;;
esac
