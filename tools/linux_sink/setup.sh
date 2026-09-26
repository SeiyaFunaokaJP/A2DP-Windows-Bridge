#!/bin/sh
# One-time setup of a2dpwb_sink (Ubuntu / Debian): decoder libraries, a Python
# virtual environment in .venv and the pinned requirements. Safe to run again.
#
#     sh setup.sh
#
# Then start the receiver with  sudo sh run.sh
set -e
cd "$(dirname "$0")"

# Python 3.11 or later
if ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))' 2>/dev/null; then
    echo "Python 3.11 or later is required (python3: $(python3 --version 2>&1))" >&2
    exit 1
fi

# System packages: venv support, and the decoders (optional: without them the
# stream is still measured, only frame headers are checked)
if command -v apt-get >/dev/null 2>&1; then
    missing=""
    for p in python3-venv libsbc1 libfreeaptx0 libfdk-aac2; do
        dpkg -s "$p" >/dev/null 2>&1 || missing="$missing $p"
    done
    if [ -n "$missing" ]; then
        echo "Installing:$missing"
        sudo apt-get update -qq || true
        for p in $missing; do
            # libfdk-aac2 is in Ubuntu's multiverse / Debian's non-free: may be missing
            sudo apt-get install -y -qq "$p" || echo "  $p not available: continuing without it"
        done
    fi
else
    echo "Not a Debian / Ubuntu system: install python3 venv support and, optionally," \
         "libsbc, libfreeaptx and libfdk-aac yourself"
fi

if [ ! -x .venv/bin/python ]; then
    echo "Creating virtual environment: $(pwd)/.venv"
    python3 -m venv .venv
fi
echo "Installing requirements..."
.venv/bin/python -m pip install --quiet --disable-pip-version-check -r requirements.txt
echo "Ready: Bumble $(.venv/bin/python -c 'import importlib.metadata as m; print(m.version("bumble"))')"

if command -v ufw >/dev/null 2>&1 && sudo -n ufw status 2>/dev/null | grep -q "Status: active"; then
    echo "The firewall (ufw) is active: allow the statistics port with  sudo ufw allow 51201"
fi
echo "Start the receiver with:  sudo sh run.sh   (options: sh run.sh --help)"
