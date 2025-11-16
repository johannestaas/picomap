#!/usr/bin/env bash
set -e

if [ -f .env ]; then
    source .env
else
    echo "You should write a .env file with a working $$WIFISSID and $$WIFIPASS"
fi

echo "Flashing with SSID: $WIFISSID"

elf2uf2-rs -d "$@"
