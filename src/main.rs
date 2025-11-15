#![no_std]
#![no_main]

use core::fmt::Write;
use cyw43::Control;
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::info;
use embassy_executor::Spawner;
use embassy_net::DhcpConfig;
use embassy_net::{self, Config, Runner, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, I2c};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use heapless::String;
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

static NET_STACK: StaticCell<Stack> = StaticCell::new();
static NET_RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
static NET_RUNNER: StaticCell<Runner<cyw43::NetDriver<'static>>> = StaticCell::new();

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn network_task(runner: &'static mut Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

async fn blink(control: &mut Control<'_>, num_blinks: usize, delay_ms: u64) {
    let delay = Duration::from_millis(delay_ms);
    for _ in 1..num_blinks {
        // info!("led on!");
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        //info!("led off!");
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let sda = p.PIN_4;
    let scl = p.PIN_5;
    let mut i2c_cfg = i2c::Config::default();
    i2c_cfg.frequency = 400_000;
    i2c_cfg.sda_pullup = true;
    i2c_cfg.scl_pullup = true;
    let i2c = I2c::new_blocking(p.I2C0, scl, sda, i2c_cfg);
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();
    display.clear(BinaryColor::Off).unwrap();

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download ../../cyw43-firmware/43439A0.bin --binary-format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download ../../cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP2040 --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    spawner.spawn(cyw43_task(runner)).unwrap();

    control.init(clm).await;

    // blink 5 times in 0.5 seconds (100ms) just to show it started
    blink(&mut control, 5, 100).await;

    let config = Config::dhcpv4(DhcpConfig::default());

    let resources = NET_RESOURCES.init(StackResources::<2>::new());
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        resources,
        embassy_time::Instant::now().as_ticks(),
    );

    let _stack = NET_STACK.init(stack);
    let runner = NET_RUNNER.init(runner);

    spawner.spawn(network_task(runner)).unwrap();

    const SSID: &str = env!("WIFISSID");
    const PASSWORD: &str = env!("WIFIPASS");

    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let mut out: String<64> = String::new();
    write!(&mut out, "SSID: {}", SSID).unwrap();
    Text::new(&out, Point::new(0, 10), style)
        .draw(&mut display)
        .unwrap();
    Text::new("Connecting...", Point::new(0, 20), style)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    // blink 3 times in 1 second (333ms)
    blink(&mut control, 3, 333).await;

    if let Err(e) = control
        .join(SSID, cyw43::JoinOptions::new(PASSWORD.as_bytes()))
        .await
    {
        let mut msg: String<64> = String::new();
        write!(&mut msg, "{:?}", e).unwrap();

        Text::new(&msg, Point::new(0, 40), style)
            .draw(&mut display)
            .unwrap();
        display.flush().unwrap();

        // Pause for 5 sec so you can read the error message...
        embassy_time::Timer::after_millis(5000).await;

        panic!("wifi join failed: {:?}", e);
    }

    // Otherwise, success!
    // blink 10 times in 1 second (100ms)
    blink(&mut control, 10, 100).await;
    info!("wifi connected!");

    Text::new("Connected!", Point::new(0, 30), style)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let delay = Duration::from_millis(100);
    loop {
        info!("led on!");
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        info!("led off!");
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}

/*
at top:
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

then:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passes() {
        // These unit tests will run on the *host*, not the Pico W.
        // If you want integration tests, you should use probe-rs’s embedded test runner which
        // can extract defmt over RTT and drive an embedded unit test environment.
    }
}
*/
