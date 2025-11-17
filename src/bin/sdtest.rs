#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embassy_rp::init;
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager};
use {panic_probe as _, defmt_rtt as _};

// use crate::sd_spi::EmbassySpiDevice;

struct DummyTime;

impl TimeSource for DummyTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 54,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = init(Default::default());

    info!("SD init test starting...");

    let sck  = p.PIN_18;
    let mosi = p.PIN_19;
    let miso = p.PIN_16;
    let cs_p = p.PIN_17;

    let mut cfg = spi::Config::default();
    cfg.frequency = 400_000;
    let _spi = spi::Spi::new_blocking(
        p.SPI0,
        sck,
        mosi,
        miso,
        cfg,
    );

    let _cs = Output::new(cs_p, Level::High);
    info!("finished up to here");
    // let mut spi_dev = EmbassySpiDevice { bus: &spi, cs };

    // sdcard_test(&mut spi_dev);
}

/*
fn sdcard_test(dev: &mut EmbassySpiDevice<'_, embassy_rp::peripherals::SPI0>) {
    let mut delay = embassy_time::Delay;
    let ts = DummyTime;

    let mut sd = SdCard::new(dev, &mut delay);

    sd.init().unwrap();

    let num_bytes = sd.num_bytes().unwrap();
    info!("Card size: {} bytes", num_bytes);

    let mut volman = VolumeManager::new(sd, ts);
    let vol0 = volman.open_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();

    info!("Volume 0 opened!");

    let root = vol0.open_root_dir().unwrap();

    info!("Root dir OK!");
}
*/
