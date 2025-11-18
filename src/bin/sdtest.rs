#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embedded_sdmmc::VolumeManager;

use picomap::sd_spi::{DummyTime, EmbassySpiDevice};
use picomap::fat_utils::{append_line, sfn_to_str, str_to_sfn};
use {defmt_rtt as _, panic_probe as _};

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
    let mut volman = VolumeManager::new(blockdev, ts);

    let mut filenames: heapless::Vec<heapless::String<13>, 64> = heapless::Vec::new();

    // borrowing rules :D
    {
        let volume = volman.open_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();
        info!("Volume 0 opened!");
        let root = volume.open_root_dir().unwrap();
        info!("Root directory opened!");
        let raw = root.to_raw_directory();
        volman.iterate_dir(raw, |entry| {
            if entry.attributes.is_directory() {
                info!("saw dir: {}", sfn_to_str(&entry.name));
            } else {
                let s: heapless::String<13> = sfn_to_str(&entry.name);
                filenames.push(s).ok();
            }
        }).unwrap();
        volman.close_dir(raw).unwrap();
        volume.close().unwrap();
    }

    {
        let volume = volman.open_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();
        info!("Volume 0 opened!");
        let root = volume.open_root_dir().unwrap();
        info!("Root directory opened!");
        let raw = root.to_raw_directory();
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
        volman.close_dir(raw).unwrap();
        volume.close().unwrap();
    }

    let mut s = heapless::String::new();
    s.push_str("please work PLEASE fiNAlTest!\n").unwrap();
    {
        let raw_vol = volman.open_raw_volume(embedded_sdmmc::VolumeIdx(0)).unwrap();
        let raw_root = volman.open_root_dir(raw_vol).unwrap();
        // Truncate will zero it out if it exists
        /* Notes on modes:
                ReadOnly: Open only if exists. No writing.
                ReadWrite: Open only if exists. Can read/write.
                ReadWriteCreate: Create new, _error if exists_.
                ReadWriteAppend: Open or _create_, seek to EOF.
                ReadWriteTruncate: Create new _or overwrite existing file to 0 bytes_.
        */

        let handle = volman.open_file_in_dir(raw_root, str_to_sfn("OUTPUT.TXT"), embedded_sdmmc::Mode::ReadWriteTruncate).unwrap();
        append_line(&mut volman, handle, &s);
        volman.flush_file(handle).unwrap();
        volman.close_file(handle).unwrap();
        volman.close_dir(raw_root).unwrap();
        volman.close_volume(raw_vol).unwrap();
    }
    info!("We are done here!");
}
