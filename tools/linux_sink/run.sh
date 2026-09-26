#!/bin/sh
# Start the receiver with the environment made by setup.sh (as root: the
# adapter is driven through the HCI user channel). Options go to a2dpwb_sink.py.
#
#     sudo sh run.sh [--mtu 679] [--csv stats.csv] ...
cd "$(dirname "$0")"
if [ ! -x .venv/bin/python ]; then
    echo "Run  sh setup.sh  first" >&2
    exit 1
fi
exec .venv/bin/python -u a2dpwb_sink.py "$@"
