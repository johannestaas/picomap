//! fat_utils
//! ---------
//!
//! Just some helpers to read and write out to files, assuming we are
//! using SD+SPI.
use crate::sd_spi::{DummyTime, EmbassySpiDevice};
use embedded_sdmmc::{ShortFileName, VolumeManager};

pub fn str_to_sfn(name: &str) -> ShortFileName {
    ShortFileName::create_from_str(name).unwrap()
}

pub fn sfn_to_str(sfn: &ShortFileName) -> heapless::String<13> {
    let mut out: heapless::String<13> = heapless::String::new();

    let base = core::str::from_utf8(sfn.base_name()).unwrap();
    let ext = core::str::from_utf8(sfn.extension()).unwrap();

    out.push_str(base).unwrap();

    if !ext.is_empty() {
        out.push('.').unwrap();
        out.push_str(ext).unwrap();
    }

    out
}

// Seeks to the end, then writes the bytes.
// Caller should flush and close.
pub fn append_line<S>(
    volman: &mut VolumeManager<EmbassySpiDevice<S>, DummyTime>,
    handle: embedded_sdmmc::RawFile,
    line: &heapless::String<128>,
) where
    S: embassy_rp::spi::Instance,
{
    volman
        .file_seek_from_end(handle, 0)
        .expect("seek to end failed");

    let bytes = line.as_bytes();

    volman.write(handle, bytes).expect("write failed");
}
