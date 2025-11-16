#!/bin/bash
set -eou pipefail

SCRIPTDIR=$(dirname ${BASH_SOURCE[0]})
pushd $SCRIPTDIR/..
REPODIR=$(pwd)
FWDIR="$REPODIR/cyw43-firmware"

if [[ ! -d "$FWDIR" ]] ; then
    mkdir -p "$FWDIR"
    wget https://github.com/embassy-rs/embassy/raw/4f7ac1946a43379306aa432961fb97bba1139a6e/cyw43-firmware/43439A0.bin -O "$FWDIR/43439A0.bin"
    wget https://github.com/embassy-rs/embassy/raw/4f7ac1946a43379306aa432961fb97bba1139a6e/cyw43-firmware/43439A0_clm.bin -O "$FWDIR/43439A0_clm.bin"
    sha256sum -c cyw43-firmware.sha256
else
    echo "Firmware already here, skipping."
fi
