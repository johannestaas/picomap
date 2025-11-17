//! sd_spi
//! ------
//!
//! Full SD-over-SPI block device for embedded_sdmmc, and owns SPI + CS.
//!
//! Compatible with embassy-rp (blocking SPI) and embedded-sdmmc 0.9.0.

use core::cell::UnsafeCell;
use defmt::{debug, error, info, warn};
use embassy_rp::gpio::Output;
use embassy_rp::spi::Spi;
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};

// SD command constants

const CMD0: u8 = 0; // GO_IDLE_STATE
const CMD8: u8 = 8; // SEND_IF_COND
const CMD16: u8 = 16; // SET_BLOCKLEN (not needed for SDHC)
const CMD17: u8 = 17; // READ_SINGLE_BLOCK
const CMD24: u8 = 24; // WRITE_BLOCK
const CMD55: u8 = 55; // PREFIX for ACMD
const CMD58: u8 = 58; // READ_OCR
const ACMD41: u8 = 41; // SD_SEND_OP_COND

const TOKEN_DATA: u8 = 0xFE;

#[derive(Debug, Clone, Copy)]
pub enum SdSpiError {
    Spi,
    Timeout,
    BadResponse,
    BadToken,
}

// Owned SPI + CS block-device

pub struct EmbassySpiDevice<'d, S: embassy_rp::spi::Instance> {
    spi: UnsafeCell<Spi<'d, S, embassy_rp::spi::Blocking>>,
    cs: UnsafeCell<Output<'d>>,
    sdhc: UnsafeCell<bool>,
}

impl<'d, S> EmbassySpiDevice<'d, S>
where
    S: embassy_rp::spi::Instance,
{
    pub fn new(spi: Spi<'d, S, embassy_rp::spi::Blocking>, mut cs: Output<'d>) -> Self {
        cs.set_high();
        Self {
            spi: UnsafeCell::new(spi),
            cs: UnsafeCell::new(cs),
            sdhc: UnsafeCell::new(false),
        }
    }

    unsafe fn spi_ptr(&self) -> *mut Spi<'d, S, embassy_rp::spi::Blocking> {
        self.spi.get()
    }

    unsafe fn cs_ptr(&self) -> *mut Output<'d> {
        self.cs.get()
    }

    fn is_sdhc(&self) -> bool {
        unsafe { *self.sdhc.get() }
    }

    fn set_sdhc(&self, v: bool) {
        unsafe {
            *self.sdhc.get() = v;
        }
    }

    fn select(&self) {
        unsafe { &mut *self.cs_ptr() }.set_low();
    }

    fn deselect(&self) {
        let cs = unsafe { &mut *self.cs_ptr() };
        let spi = unsafe { &mut *self.spi_ptr() };
        cs.set_high();
        let _ = spi.blocking_write(&[0xFF; 2]); // 16 clocks
    }

    fn send_cmd(&self, cmd: u8, arg: u32, crc: u8) -> Result<u8, SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };

        let packet = [
            0x40 | cmd,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            (arg) as u8,
            crc | 0x01,
        ];

        debug!("Sending cmd with blocking_write");
        spi.blocking_write(&packet).map_err(|_| SdSpiError::Spi)?;

        let mut rx = [0xFF];
        let tx = [0xFF];

        for i in 0..8 {
            debug!("blocking transfer [{}]", i);
            spi.blocking_transfer(&mut rx, &tx)
                .map_err(|_| SdSpiError::Spi)?;
            debug!("transfer {} transfered, R1={:#04x}", i, rx[0]);
            if rx[0] != 0xFF {
                debug!("R1={:#04x}", rx[0]);
                return Ok(rx[0]);
            }
        }

        debug!(
            "timed out on send_cmd cmd={:#02x} arg={:#08x} crc={:#02x}",
            cmd, arg, crc
        );
        Err(SdSpiError::Timeout)
    }

    pub fn init(&self) -> Result<(), SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };

        debug!("deselecting");
        self.deselect();

        // Provide initial clocks (>74)
        debug!("providing initial clocks");
        let _ = spi.blocking_write(&[0xFF; 10]);

        debug!("selecting");
        self.select();

        // CMD0: GO_IDLE_STATE
        debug!("CMD0 GO_IDLE_STATE");
        let r = self.send_cmd(CMD0, 0, 0x95)?;
        debug!("CMD0 set cmd");
        if r != 0x01 {
            debug!("r != 0x01, it was {}. deselecting...", r);
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        // CMD8: SEND_IF_COND
        debug!("CMD8 SEND_IF_COND");
        let r = self.send_cmd(CMD8, 0x1AA, 0x87)?;
        if r & 0x04 != 0 {
            // illegal command (probably SDSC)
            debug!("illegal command, setting sdhc false");
            self.set_sdhc(false);
        } else {
            // read rest of R7 response (4 bytes)
            debug!("reading rest of R7 response");
            let mut buf = [0xFF; 4];
            spi.blocking_read(&mut buf).map_err(|_| SdSpiError::Spi)?;
            if buf[2] != 0x01 || buf[3] != 0xAA {
                self.deselect();
                return Err(SdSpiError::BadResponse);
            }
        }

        // ACMD41 loop
        debug!("ACMD41 loop...");
        for i in 0..2000 {
            let r55 = self.send_cmd(CMD55, 0, 0x01)?;
            defmt::info!("CMD55: i={}  R1={:#04x}", i, r55);

            let r41 = self.send_cmd(ACMD41, 0x40000000, 0x01)?;
            defmt::info!("ACMD41: i={}  R1={:#04x}", i, r41);

            if r41 == 0x00 {
                defmt::info!("ACMD41: initialization completed after {} iterations!", i);
                break;
            }
        }

        // CMD58: read OCR
        let r = self.send_cmd(CMD58, 0, 0x01)?;
        if r != 0x00 {
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        let mut ocr = [0xFF; 4];
        spi.blocking_read(&mut ocr).map_err(|_| SdSpiError::Spi)?;

        // bit 30 => SDHC/SDXC
        if (ocr[0] & 0x40) != 0 {
            self.set_sdhc(true);
        }

        // CMD16 (set block length = 512) only if SDSC
        if !self.is_sdhc() {
            let r = self.send_cmd(CMD16, 512, 0x01)?;
            if r != 0x00 {
                self.deselect();
                return Err(SdSpiError::BadResponse);
            }
        }

        self.deselect();
        Ok(())
    }

    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };
        let addr = if self.is_sdhc() { block } else { block * 512 };

        self.select();
        let r = self.send_cmd(CMD17, addr, 0x01)?;
        if r != 0x00 {
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        // wait for data token
        for _ in 0..10000 {
            let mut b = [0xFF];
            spi.blocking_read(&mut b).map_err(|_| SdSpiError::Spi)?;
            if b[0] == TOKEN_DATA {
                break;
            }
        }

        // read block
        spi.blocking_read(buf).map_err(|_| SdSpiError::Spi)?;

        // discard CRC
        let mut crc = [0xFF; 2];
        spi.blocking_read(&mut crc).map_err(|_| SdSpiError::Spi)?;

        self.deselect();
        Ok(())
    }

    fn write_block(&self, block: u32, buf: &[u8; 512]) -> Result<(), SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };
        let addr = if self.is_sdhc() { block } else { block * 512 };

        self.select();

        let r = self.send_cmd(CMD24, addr, 0x01)?;
        if r != 0x00 {
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        // data token
        spi.blocking_write(&[TOKEN_DATA])
            .map_err(|_| SdSpiError::Spi)?;

        // write block
        spi.blocking_write(buf).map_err(|_| SdSpiError::Spi)?;

        // dummy CRC
        spi.blocking_write(&[0xFF, 0xFF])
            .map_err(|_| SdSpiError::Spi)?;

        // read data response
        let mut resp = [0xFF];
        spi.blocking_read(&mut resp).map_err(|_| SdSpiError::Spi)?;
        if (resp[0] & 0x1F) != 0x05 {
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        // wait until not busy
        for _ in 0..30000 {
            let mut b = [0x00];
            spi.blocking_read(&mut b).ok();
            if b[0] == 0xFF {
                break;
            }
        }

        self.deselect();
        Ok(())
    }
}

impl<'d, S> BlockDevice for EmbassySpiDevice<'d, S>
where
    S: embassy_rp::spi::Instance,
{
    type Error = embedded_sdmmc::Error<SdSpiError>;

    fn read(&self, blocks: &mut [Block], start: BlockIdx) -> Result<(), Self::Error> {
        for (i, blk) in blocks.iter_mut().enumerate() {
            self.read_block(start.0 + i as u32, &mut blk.contents)
                .map_err(embedded_sdmmc::Error::DeviceError)?;
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], start: BlockIdx) -> Result<(), Self::Error> {
        for (i, blk) in blocks.iter().enumerate() {
            self.write_block(start.0 + i as u32, &blk.contents)
                .map_err(embedded_sdmmc::Error::DeviceError)?;
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        // You *can* implement CMD9 to read CSD and extract card size.
        // For now, embedded-sdmmc doesn't require this if volume 0 contains a valid FAT.
        Err(embedded_sdmmc::Error::Unsupported)
    }
}
