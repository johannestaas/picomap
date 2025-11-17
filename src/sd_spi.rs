use core::cell::UnsafeCell;
use embassy_rp::gpio::Output;
use embassy_rp::spi::Spi;
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};

#[derive(Debug, Clone, Copy)]
pub enum SdSpiError {
    SpiRead,
    SpiSendReadCommand,
    SpiWrite,
    SpiSendWriteCommand,
}

pub struct EmbassySpiDevice<'a, S: embassy_rp::spi::Instance> {
    spi: UnsafeCell<Spi<'a, S, embassy_rp::spi::Blocking>>,
    cs: UnsafeCell<Output<'a>>,
}

impl<'a, S> EmbassySpiDevice<'a, S>
where
    S: embassy_rp::spi::Instance,
{
    pub fn new(spi: Spi<'a, S, embassy_rp::spi::Blocking>, cs: Output<'a>) -> Self {
        Self {
            spi: UnsafeCell::new(spi),
            cs: UnsafeCell::new(cs),
        }
    }
}

impl<'a, S> BlockDevice for EmbassySpiDevice<'a, S>
where
    S: embassy_rp::spi::Instance,
{
    type Error = embedded_sdmmc::Error<SdSpiError>;

    fn read(&self, blocks: &mut [Block], start: BlockIdx) -> Result<(), Self::Error> {
        let spi = unsafe { &mut *self.spi.get() };
        let cs = unsafe { &mut *self.cs.get() };

        for (i, block) in blocks.iter_mut().enumerate() {
            cs.set_low();

            // CMD17 -> read one 512-byte block
            let block_index = start.0 + i as u32;
            let addr = block_index * 512;

            spi.blocking_write(&[
                0x11, // READ_SINGLE_BLOCK (CMD17, 0x11)
                (addr >> 24) as u8,
                (addr >> 16) as u8,
                (addr >> 8) as u8,
                (addr) as u8,
                0xFF, // dummy CRC
            ])
            .map_err(|_| embedded_sdmmc::Error::DeviceError(SdSpiError::SpiSendReadCommand))?;

            spi.blocking_read(&mut block.contents)
                .map_err(|_| embedded_sdmmc::Error::DeviceError(SdSpiError::SpiRead))?;

            cs.set_high();
        }

        Ok(())
    }

    fn write(&self, blocks: &[Block], start: BlockIdx) -> Result<(), Self::Error> {
        let spi = unsafe { &mut *self.spi.get() };
        let cs = unsafe { &mut *self.cs.get() };

        for (i, block) in blocks.iter().enumerate() {
            cs.set_low();

            // CMD24: write one 512-byte block
            let block_index = start.0 + i as u32;
            let addr = block_index * 512;

            spi.blocking_write(&[
                0x18, // WRITE_BLOCK (CMD24)
                (addr >> 24) as u8,
                (addr >> 16) as u8,
                (addr >> 8) as u8,
                (addr) as u8,
                0xFF, // dummy CRC
            ])
            .map_err(|_| embedded_sdmmc::Error::DeviceError(SdSpiError::SpiSendWriteCommand))?;

            spi.blocking_write(&block.contents)
                .map_err(|_| embedded_sdmmc::Error::DeviceError(SdSpiError::SpiWrite))?;

            cs.set_high();
        }

        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        Err(embedded_sdmmc::Error::Unsupported)
    }
}
