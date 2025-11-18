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
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx, TimeSource, Timestamp};

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

pub fn crc7(bytes: &[u8]) -> u8 {
    let mut crc: u8 = 0;

    for &byte in bytes {
        let mut b = byte;
        for _ in 0..8 {
            let bit = ((b & 0x80) != 0) ^ ((crc & 0x40) != 0);
            crc = ((crc << 1) & 0x7F) ^ if bit { 0x09 } else { 0x00 };
            b <<= 1;
        }
    }

    crc & 0x7F
}

#[derive(Debug, Clone, Copy)]
pub enum SdSpiError {
    Spi,
    Timeout,
    BadResponse,
    BadToken,
}

pub struct DummyTime;

impl TimeSource for DummyTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
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

    fn send_cmd(&self, cmd: u8, arg: u32) -> Result<u8, SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };

        let header = [
            0x40 | cmd,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            arg as u8,
        ];

        let crc = (crc7(&header) << 1) | 1;

        let packet = [header[0], header[1], header[2], header[3], header[4], crc];

        spi.blocking_write(&packet).map_err(|_| SdSpiError::Spi)?;

        let mut rx = [0xFF];
        let tx = [0xFF];

        for i in 0..8 {
            spi.blocking_transfer(&mut rx, &tx)
                .map_err(|_| SdSpiError::Spi)?;
            if rx[0] != 0xFF {
                debug!("CMD{} R1[{}]={:#04x}", cmd, i, rx[0]);
                return Ok(rx[0]);
            }
        }

        error!(
            "timed out on send_cmd cmd=CMD{} arg={:#08x} crc={:#02x}",
            cmd, arg, crc
        );
        Err(SdSpiError::Timeout)
    }

    pub fn init(&self) -> Result<(), SdSpiError> {
        info!("Initializing sd_spi");
        let spi = unsafe { &mut *self.spi_ptr() };

        debug!("deselecting");
        self.deselect();

        /* //
        let mut b = [0x00];
        spi.blocking_transfer(&mut b, &[0xFF]).unwrap();
        debug!("MISO idle = {:#04x}", b[0]);
        */
        //

        // Provide initial clocks (>74)
        debug!("providing initial clocks");
        let _ = spi.blocking_write(&[0xFF; 200]);

        debug!("selecting");
        self.select();

        // give time for the card to notice CS low before CMD0
        debug!("extra idle clocks...");
        let _ = spi.blocking_write(&[0xFF; 2]);

        /*
        let mut test = [0x00];
        let tx = [0xFF];
        spi.blocking_transfer(&mut test, &tx).unwrap();
        debug!("Raw MISO now: {:#04x}", test[0]);
        */

        // CMD0: GO_IDLE_STATE
        debug!(
            "About to send CMD0: CS low? {}",
            unsafe { &*self.cs_ptr() }.is_set_low()
        );
        debug!("CMD0 GO_IDLE_STATE");
        let r = self.send_cmd(CMD0, 0)?;
        debug!("CMD0 set cmd");
        if r != 0x01 {
            debug!("r != 0x01, it was {}. deselecting...", r);
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }
        self.deselect();
        spi.blocking_write(&[0xFF]).ok();
        self.select();

        // CMD8: SEND_IF_COND
        debug!("CMD8 SEND_IF_COND");
        let r = self.send_cmd(CMD8, 0x1AA)?;
        if r & 0x04 != 0 {
            // illegal command (probably SDSC)
            warn!("illegal command, setting sdhc false");
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
        self.deselect();
        spi.blocking_write(&[0xFF]).ok(); // idle clocks
        self.select();

        // ACMD41 loop
        debug!("ACMD41 loop...");

        for i in 0..2000 {
            // CMD55 APP_CMD prefix
            self.select();
            let r55 = self.send_cmd(CMD55, 0)?;
            spi.blocking_write(&[0xFF]).ok();
            self.deselect();

            debug!("CMD55 iter {} -> {:#04x}", i, r55);

            // ACMD41 (SD_SEND_OP_COND)
            spi.blocking_write(&[0xFF]).ok();
            self.select();
            let r41 = self.send_cmd(ACMD41, 0x4000_0000)?; // HCS = 1
            spi.blocking_write(&[0xFF]).ok();
            self.deselect();
            spi.blocking_write(&[0xFF]).ok();

            debug!("ACMD41 iter {} -> {:#04x}", i, r41);

            if r41 == 0x00 {
                info!("ACMD41: initialization completed after {} iterations!", i);
                break;
            }

            if i == 1999 {
                error!("testing CRC7 CMD0 = {:#04x}", crc7(&[0x40, 0, 0, 0, 0]));
                error!("testing CRC7 CMD8 = {:#04x}", crc7(&[0x48, 0, 0, 1, 0xAA]));
                panic!("ACMD41 loop never got 0x00 result from r41: {:#02x}", r41);
            }
        }

        // CMD58: read OCR
        self.select();
        let r = self.send_cmd(CMD58, 0)?;
        if r != 0x00 {
            self.deselect();
            error!("bad response at CMD58");
            return Err(SdSpiError::BadResponse);
        }

        let mut ocr = [0xFF; 4];
        spi.blocking_read(&mut ocr).map_err(|_| SdSpiError::Spi)?;
        self.deselect(); // deselect *after* OCR read
        spi.blocking_write(&[0xFF]).ok();

        // bit 30 => SDHC/SDXC
        if (ocr[0] & 0x40) != 0 {
            self.set_sdhc(true);
        }

        // CMD16 (set block length = 512) only if SDSC
        if !self.is_sdhc() {
            self.select();
            let r = self.send_cmd(CMD16, 512)?;
            if r != 0x00 {
                self.deselect();
                error!("sd_spi bad response at CMD16 and not sdhc");
                return Err(SdSpiError::BadResponse);
            }
        }

        self.deselect();
        spi.blocking_write(&[0xFF]).ok();
        info!("sd_spi successfully initialized.");
        Ok(())
    }

    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), SdSpiError> {
        let spi = unsafe { &mut *self.spi_ptr() };
        let addr = if self.is_sdhc() { block } else { block * 512 };

        self.select();
        let r = self.send_cmd(CMD17, addr)?;
        if r != 0x00 {
            self.deselect();
            return Err(SdSpiError::BadResponse);
        }

        // wait for data token
        let mut found = false;
        for _ in 0..10000 {
            let mut b = [0xFF];
            spi.blocking_read(&mut b).map_err(|_| SdSpiError::Spi)?;
            if b[0] == TOKEN_DATA {
                found = true;
                break;
            }
        }
        if !found {
            self.deselect();
            error!("did not find token data with read_block");
            return Err(SdSpiError::Timeout);
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

        let r = self.send_cmd(CMD24, addr)?;
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
        spi.blocking_write(&[0xFF]).ok();
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
