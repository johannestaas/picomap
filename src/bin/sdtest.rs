#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embedded_sdmmc::{TimeSource, Timestamp, VolumeManager};

use picomap::sd_spi::EmbassySpiDevice;
use {defmt_rtt as _, panic_probe as _};

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
    let p = embassy_rp::init(Default::default());
    info!("SD init test starting...");

    let sck = p.PIN_18;
    let mosi = p.PIN_19;
    let miso = p.PIN_16;
    let cs_p = p.PIN_17;

    let mut cfg = spi::Config::default();
    cfg.frequency = 100_000;

    let spi_dev = spi::Spi::new_blocking(p.SPI0, sck, mosi, miso, cfg);

    let cs = Output::new(cs_p, Level::High);

    let blockdev = EmbassySpiDevice::new(spi_dev, cs);
    blockdev.init().unwrap();

    /*
    let delay = embassy_time::Delay;
    let mut sd = SdCard::new(&mut blockdev, &mut delay);
    sd.init().unwrap();

    info!("SD card initialized!");

    let bytes = sd.num_bytes().unwrap();
    info!("Card size {} bytes", bytes);

    */
    let ts = DummyTime;
    // let mut volman = VolumeManager::new(sd, ts);
    let volman = VolumeManager::new(blockdev, ts);

    let volume = volman.open_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();
    info!("Volume 0 opened!");

    let root = volume.open_root_dir().unwrap();
    info!("Root directory opened!");

    let dir = root;
    let raw = dir.to_raw_directory();

    volman
        .iterate_dir(raw, |entry| {
            if entry.attributes.is_directory() {
                info!("DIR:  {}", entry.name);
            } else {
                info!("FILE: {} ({} bytes)", entry.name, entry.size);
            }
        })
        .unwrap();
}
