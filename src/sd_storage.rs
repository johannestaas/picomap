use crate::sd_spi::{DummyTime, EmbassySpiDevice};
use embassy_rp::gpio::Output;
use embedded_sdmmc::{Mode, VolumeIdx, VolumeManager};

use crate::fat_utils::str_to_sfn;
use crate::sd_spi::SdSpiError;

pub struct SdStorage<'d, S: embassy_rp::spi::Instance> {
    volman: VolumeManager<EmbassySpiDevice<'d, S>, DummyTime>,
}

fn flatten_err<E: core::fmt::Debug>(
    err: embedded_sdmmc::Error<embedded_sdmmc::Error<E>>,
) -> embedded_sdmmc::Error<E> {
    match err {
        embedded_sdmmc::Error::DeviceError(inner) => inner,
        embedded_sdmmc::Error::FormatError(f) => embedded_sdmmc::Error::FormatError(f),
        embedded_sdmmc::Error::FilenameError(f) => embedded_sdmmc::Error::FilenameError(f),
        embedded_sdmmc::Error::BadBlockSize(b) => embedded_sdmmc::Error::BadBlockSize(b),
        embedded_sdmmc::Error::BadHandle => embedded_sdmmc::Error::BadHandle,
        embedded_sdmmc::Error::NotFound => embedded_sdmmc::Error::NotFound,
        embedded_sdmmc::Error::EndOfFile => embedded_sdmmc::Error::EndOfFile,
        embedded_sdmmc::Error::DiskFull => embedded_sdmmc::Error::DiskFull,
        embedded_sdmmc::Error::AllocationError => embedded_sdmmc::Error::AllocationError,
        embedded_sdmmc::Error::NoSuchVolume => embedded_sdmmc::Error::NoSuchVolume,
        embedded_sdmmc::Error::TooManyOpenVolumes => embedded_sdmmc::Error::TooManyOpenVolumes,
        embedded_sdmmc::Error::TooManyOpenDirs => embedded_sdmmc::Error::TooManyOpenDirs,
        embedded_sdmmc::Error::TooManyOpenFiles => embedded_sdmmc::Error::TooManyOpenFiles,
        embedded_sdmmc::Error::VolumeAlreadyOpen => embedded_sdmmc::Error::VolumeAlreadyOpen,
        embedded_sdmmc::Error::DirAlreadyOpen => embedded_sdmmc::Error::DirAlreadyOpen,
        embedded_sdmmc::Error::FileAlreadyOpen => embedded_sdmmc::Error::FileAlreadyOpen,
        embedded_sdmmc::Error::VolumeStillInUse => embedded_sdmmc::Error::VolumeStillInUse,
        embedded_sdmmc::Error::DirAlreadyExists => embedded_sdmmc::Error::DirAlreadyExists,
        embedded_sdmmc::Error::FileAlreadyExists => embedded_sdmmc::Error::FileAlreadyExists,
        embedded_sdmmc::Error::OpenedDirAsFile => embedded_sdmmc::Error::OpenedDirAsFile,
        embedded_sdmmc::Error::OpenedFileAsDir => embedded_sdmmc::Error::OpenedFileAsDir,
        embedded_sdmmc::Error::DeleteDirAsFile => embedded_sdmmc::Error::DeleteDirAsFile,
        embedded_sdmmc::Error::Unsupported => embedded_sdmmc::Error::Unsupported,
        embedded_sdmmc::Error::BadCluster => embedded_sdmmc::Error::BadCluster,
        embedded_sdmmc::Error::ConversionError => embedded_sdmmc::Error::ConversionError,
        embedded_sdmmc::Error::NotEnoughSpace => embedded_sdmmc::Error::NotEnoughSpace,
        embedded_sdmmc::Error::UnterminatedFatChain => embedded_sdmmc::Error::UnterminatedFatChain,
        embedded_sdmmc::Error::ReadOnly => embedded_sdmmc::Error::ReadOnly,
        embedded_sdmmc::Error::InvalidOffset => embedded_sdmmc::Error::InvalidOffset,
        embedded_sdmmc::Error::LockError => embedded_sdmmc::Error::LockError,
    }
}

impl<'d, S> SdStorage<'d, S>
where
    S: embassy_rp::spi::Instance,
{
    pub fn new(
        spi_dev: embassy_rp::spi::Spi<'d, S, embassy_rp::spi::Blocking>,
        cs: Output<'d>,
    ) -> Result<Self, SdSpiError> {
        let dev = EmbassySpiDevice::new(spi_dev, cs);
        dev.init()?;
        let ts = DummyTime;
        let volman = VolumeManager::new(dev, ts);

        Ok(SdStorage { volman })
    }

    pub fn log_ip(&mut self, ip: &str) -> Result<(), embedded_sdmmc::Error<SdSpiError>> {
        let volume = self.volman.open_volume(VolumeIdx(0)).map_err(flatten_err)?;
        let root = volume.open_root_dir().map_err(flatten_err)?;
        let raw = root.to_raw_directory();
        let handle = self
            .volman
            .open_file_in_dir(raw, str_to_sfn("NETWORK.LOG"), Mode::ReadWriteAppend)
            .map_err(flatten_err)?;
        self.volman
            .write(handle, ip.as_bytes())
            .map_err(flatten_err)?;
        self.volman.write(handle, b"\n").map_err(flatten_err)?;
        self.volman.flush_file(handle).ok();
        self.volman.close_dir(raw).ok();
        self.volman.close_file(handle).ok();
        volume.close().map_err(flatten_err)?;
        Ok(())
    }

    pub fn read_file<const N: usize>(
        &mut self,
        name: &str,
    ) -> Result<heapless::String<N>, embedded_sdmmc::Error<SdSpiError>> {
        let mut out = heapless::String::<N>::new();

        let volume = self.volman.open_volume(VolumeIdx(0)).map_err(flatten_err)?;
        let root = volume.open_root_dir().map_err(flatten_err)?;
        let raw = root.to_raw_directory();

        let handle = self
            .volman
            .open_file_in_dir(raw, str_to_sfn(name), Mode::ReadOnly)
            .map_err(flatten_err)?;

        let mut buf = [0u8; 512];

        loop {
            let n = self.volman.read(handle, &mut buf).map_err(flatten_err)?;
            if n == 0 {
                break;
            }

            let chunk = core::str::from_utf8(&buf[..n]).unwrap_or("<binary>");
            out.push_str(chunk).ok();
        }

        self.volman.close_file(handle).ok();
        self.volman.close_dir(raw).ok();
        volume.close().map_err(flatten_err)?;

        Ok(out)
    }
}
