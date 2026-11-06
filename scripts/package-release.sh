#!/usr/bin/env bash
set -euo pipefail
usage() {
    echo 'usage: package-release.sh BUILD_DIR OUTPUT_DIR [--cups-runtime BUILD_RECEIPT] [--cups-ipp-backend BUILD_RECEIPT] [--desktop-niri BUILD_RECEIPT --desktop-frontend BUILD_RECEIPT [--extra-portals]]' >&2
}
if [[ $# -lt 2 ]]; then
    usage
    exit 2
fi
build_input=$1
output_input=$2
shift 2
cups_receipt=
backend_receipt=
niri_receipt=
frontend_receipt=
extra_portals=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --cups-runtime)
            if [[ $# -lt 2 || -n $cups_receipt ]]; then usage; exit 2; fi
            cups_receipt=$(realpath -- "$2")
            shift 2
            ;;
        --cups-ipp-backend)
            if [[ $# -lt 2 || -n $backend_receipt ]]; then usage; exit 2; fi
            backend_receipt=$(realpath -- "$2")
            shift 2
            ;;
        --desktop-niri)
            if [[ $# -lt 2 || -n $niri_receipt ]]; then usage; exit 2; fi
            niri_receipt=$(realpath -- "$2")
            shift 2
            ;;
        --desktop-frontend)
            if [[ $# -lt 2 || -n $frontend_receipt ]]; then usage; exit 2; fi
            frontend_receipt=$(realpath -- "$2")
            shift 2
            ;;
        --extra-portals) extra_portals=true; shift ;;
        *) usage; exit 2 ;;
    esac
done
if [[ -n $backend_receipt && -z $cups_receipt ]]; then
    echo '--cups-ipp-backend requires --cups-runtime' >&2
    exit 2
fi
if [[ -n $niri_receipt && -z $frontend_receipt || -z $niri_receipt && -n $frontend_receipt ]]; then
    echo '--desktop-niri and --desktop-frontend must be supplied together' >&2
    exit 2
fi
if [[ $extra_portals == true && -z $niri_receipt ]]; then
    echo '--extra-portals requires --desktop-niri and --desktop-frontend' >&2
    exit 2
fi
build_dir=$(realpath -- "$build_input")
mkdir -p -- "$output_input"
output_dir=$(realpath -- "$output_input")
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
bundle=canopa-x86_64-unknown-linux-gnu
dest="$stage/$bundle"
mkdir -p "$dest/bin" "$dest/share/canopa" "$dest/share/canopa-shell" "$dest/share/canopa-files" \
    "$dest/share/canopa-notify" "$dest/share/systemd/user" "$dest/share/man/man1" \
    "$dest/share/dbus-1/services" "$dest/share/xdg-desktop-portal/portals" "$dest/share/applications"
if [[ -n $cups_receipt ]]; then
    python3 scripts/package-print-runtime.py "$dest" --build-receipt "$cups_receipt"
fi
if [[ -n $backend_receipt ]]; then
    python3 scripts/package-print-backend.py "$dest" --build-receipt "$backend_receipt"
fi
install -m755 "$build_dir/canopa-shell" "$build_dir/canopa-wifi-provider" \
    "$build_dir/canopa-audio-provider" "$build_dir/canopa-media-provider" \
    "$build_dir/canopa-notify" "$build_dir/canopa-files" "$build_dir/canopa" \
    "$build_dir/xdg-desktop-portal-canopa" "$dest/bin/"
install -m755 scripts/portal-config.py "$dest/bin/canopa-portal-config"
install -m644 packaging/portal/*.portal "$dest/share/xdg-desktop-portal/portals/"
if [[ -n $niri_receipt ]]; then
    desktop_args=(--niri-build "$niri_receipt" --frontend-build "$frontend_receipt")
    if [[ $extra_portals == true ]]; then desktop_args+=(--extra-portals); fi
    python3 scripts/package-desktop-sharing.py "$dest" "${desktop_args[@]}"
fi
install -m644 packaging/portal/*.service "$dest/share/dbus-1/services/"
install -m644 packaging/file-manager/org.freedesktop.FileManager1.service "$dest/share/dbus-1/services/"
install -m644 packaging/systemd/xdg-desktop-portal-canopa.service "$dest/share/systemd/user/"
install -m644 packaging/applications/org.canopa.Files.desktop "$dest/share/applications/"
python3 scripts/package-documentation.py "$dest"
install -m644 packaging/systemd/canopa-session.target packaging/systemd/canopa-shell.service packaging/systemd/canopa-notify.service "$dest/share/systemd/user/"
install -m644 packaging/canopa-notify.1 "$dest/share/man/man1/"
python3 scripts/dependency-attributions.py \
    --output "$dest/share/canopa/dependency-attributions.md" \
    --notices "$dest/share/licenses/dependencies"
"$dest/bin/canopa-shell" --version
"$dest/bin/canopa-notify" --help > /dev/null
"$dest/bin/canopa-files" --help > /dev/null
"$dest/bin/canopa-files" --internal-check-copy
tar -C "$stage" -czf "$output_dir/$bundle.tar.gz" "$bundle"
