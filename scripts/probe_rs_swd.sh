#!/bin/bash
set -euo pipefail
probe-rs run --chip rp2040 target/thumbv6m-none-eabi/debug/picomap
