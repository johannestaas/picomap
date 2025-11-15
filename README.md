PicoMap
=======

PicoMap is a network scanning firmware built with Rust and Embassy.

Tested and working on the Pico W microcontroller (but can likely work with Pico W2 with tweaks).

Licensed: GPLv3

Installation
------------

You'll need elf2uf2-rs to generate the UF2 firmware. The `runner` will push the firmware blob to any
connected Raspberry Pico chipsets using `elf2uf2-rs -d`

You should probably set up [picotool, here](https://github.com/raspberrypi/picotool).
Follow the instructions and ensure you have appropriate perms (group `dialout` for USB).

Run ./download_firmware.sh to download proprietary cyw43 firmware blobs for the Pico W.
If it fails, the hashes didn't match (which would be suspicious since I set it to an explicit git commit hash).

Configuration
-------------

It connects to 2.4GHz wifi, and literally builds a firmware image with the SSID and WPA credentials...
You didn't expect to just connect to it with a bluetooth keyboard to configure it, did you? (Not yet at least)

Create an `.env` file in the repo root, and set these two variables:

    WIFISSID="Your WIFI SSID (the normal name of it)"
    WIFIPASS="The text password (assuming WPA2/3)"

Those are built into the firmware image at build time like this:

    const SSID: &str = env!("WIFISSID");
    const PASSWORD: &str = env!("WIFIPASS");

Eventually I'll allow post-build configuration, but for now, just know that your UF2 is built with and includes
your wifi creds.


Wiring
------

Right now, it expects an [SSD1306 OLED display](https://www.amazon.com/Hosyond-Display-Self-Luminous-Compatible-Raspberry/dp/B09T6SJBV5/r) wired up accordingly:

SDA -> GP4
SCL -> GP5
GND -> GND
VCC -> +3V3
