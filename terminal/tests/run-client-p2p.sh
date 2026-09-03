#!/bin/bash

cd "$(dirname "$0")" || exit
./run.sh \
    --config-file "$PWD/config-client-p2p.toml" \
    "$@"
