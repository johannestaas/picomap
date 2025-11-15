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

Flashing Your Pico
------------------

Run `cargo build --release` to build the release binary. It should produce an ELF file.
The "runner" will actually run elf2uf2-rs and deploy for you! Assuming you have configured and wired everything
correctly, do this:

1. Unplug your pico from power
2. Hold down the BOOTSEL button on the board
3. While holding down BOOTSEL, plug the PC USB back in.
4. You should see some new USB drive pop up "RPI-RP2"
5. Now run: `cargo run --release`

You should see something like this:

    warning: unused config key `build.default-target` in `/home/$USER/picomap/.cargo/config.toml`
       Compiling picomap v0.1.0 (/home/$USER/picomap)
        Finished `release` profile [optimized] target(s) in 0.64s
         Running `/home/$USER/picomap/./flash.sh target/thumbv6m-none-eabi/release/picomap`
    Flashing with SSID: "Your SSID Here"
    Found pico uf2 disk /media/$USER/RPI-RP2
    Transfering program to pico
    851.50 KB / 851.50 KB [=========================================================================] 100.00 % 158.83 KB/s

The Pico W should restart automatically then display this on the OLED:

    SSID: "Your Configured SSID here"
    Connecting...

Eventually it should show that it connected and blink the on-board LED.
Otherwise, might have trouble connecting... or I'd check your LAN connected devices and see if
it just isn't displaying it connected.

Troubleshooting
---------------

Make sure it's a 2.4GHz Wifi network and not 6 GHz! Pretty sure the Pico W can't connect to 6 GHz.

Also, you'll want to set up SWD debugging for development.
You'll likely need a RPi probe for this and need to connect to the 3-pin JST-SH header.

If you see this:

    Error: "Unable to find mounted pico"

Unplug it, hold BOOTSEL, then plug it back in.
