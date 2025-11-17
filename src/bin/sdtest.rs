#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embedded_sdmmc::{TimeSource, Timestamp, VolumeManager};

use picomap::sd_spi::{EmbassySpiDevice, sfn_to_str};
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

    let ts = DummyTime;
    let volman = VolumeManager::new(blockdev, ts);

    let volume = volman.open_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();
    info!("Volume 0 opened!");

    let root = volume.open_root_dir().unwrap();
    info!("Root directory opened!");

    let raw = root.to_raw_directory();
    let mut filenames: heapless::Vec<heapless::String<13>, 64> = heapless::Vec::new();

    volman.iterate_dir(raw, |entry| {
        if entry.attributes.is_directory() {
            info!("saw dir: {}", sfn_to_str(&entry.name));
        } else {
            let s: heapless::String<13> = sfn_to_str(&entry.name);
            filenames.push(s).ok();
        }
    }).unwrap();

    for fname in &filenames {
        info!("Reading file: {}", fname);

        let handle = volman
            .open_file_in_dir(raw, fname.as_str(), embedded_sdmmc::Mode::ReadOnly)
            .unwrap();

        let mut buf = [0u8; 512];
        loop {
            let n = volman.read(handle, &mut buf).unwrap();
            if n == 0 {
                break; // EOF
            }

            let txt = core::str::from_utf8(&buf[..n]).unwrap_or("<binary>");
            info!("--> {}", txt);
        }

        volman.close_file(handle).unwrap();
    }

}
