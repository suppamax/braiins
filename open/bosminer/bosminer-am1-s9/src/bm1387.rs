// Copyright (C) 2019  Braiins Systems s.r.o.
//
// This file is part of Braiins Open-Source Initiative (BOSI).
//
// BOSI is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// Please, keep in mind that we may also license BOSI or any part thereof
// under a proprietary license. For more information on the terms and conditions
// of such proprietary license or if you have any other questions, please
// contact us at opensource@braiins.com.

pub mod i2c;

use crate::error::{self, ErrorKind};

use packed_struct::prelude::*;
use packed_struct_codegen::PackedStruct;
use packed_struct_codegen::{PrimitiveEnum_u16, PrimitiveEnum_u8};

use std::convert::TryInto;
use std::default::Default;
use std::fmt::Debug;
use std::mem::size_of;

#[allow(dead_code)]
pub const HASH_COUNTING_REG: u8 = 0x14;

/// Maximum supported baud rate clock divisor
const MAX_BAUD_CLOCK_DIV: usize = 26;

/// Basic divisor of the clock speed when calculating the value for the baud register
pub const CHIP_OSC_CLK_BASE_BAUD_DIV: usize = 8;

/// How many cores are on the chip
pub const NUM_CORES_ON_CHIP: usize = 114;

/// This enum is a bridge between chip address representation as we tend to
/// think about it (addresses `0..=62`) and how the hardware addresses them
/// (in increments of four).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChipAddress {
    All,
    /// Represents linear chip address 0..62
    One(usize),
}

impl ChipAddress {
    /// Return if address is a broadcast
    pub fn is_broadcast(&self) -> bool {
        match self {
            ChipAddress::All => true,
            ChipAddress::One(_) => false,
        }
    }

    /// Return hardware chip address or 0 if it's a broadcast
    fn to_hw_addr(&self) -> u8 {
        match self {
            ChipAddress::All => 0,
            ChipAddress::One(x) => ((*x) * 4)
                .try_into()
                .expect("chip address doesn't fit into a byte"),
        }
    }
}

/// This is scheme to address particular core on chain
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CoreAddress {
    chip: usize,
    core: usize,
}

impl CoreAddress {
    /// Every nonce returned by chip (except those sent by opencore) encoides address of the
    /// chip and core that computed it (by how they divide the search space).
    fn new(nonce: u32) -> Self {
        let nonce = nonce as usize;
        Self {
            chip: (nonce >> 2) & 0x3f,
            core: (nonce >> 24) & 0x7f,
        }
    }
}

/// Control or work command layout
#[derive(PackedStruct, Debug)]
#[packed_struct(size_bytes = "1", bit_numbering = "lsb0")]
pub struct Cmd {
    #[packed_field(bits = "0:3")]
    code: Integer<u8, packed_bits::Bits4>,
    #[packed_field(bits = "4")]
    to_all: bool,
    #[packed_field(bits = "5:7", ty = "enum")]
    cmd_type: CmdType,
}

impl Cmd {
    fn new(code: u8, to_all: bool) -> Self {
        Self {
            code: code.into(),
            to_all,
            cmd_type: CmdType::VilCtlCmd,
        }
    }
}

/// Command types
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
enum CmdType {
    /// Control command for the chip
    VilCtlCmd = 0x02,
}

#[derive(PackedStruct, Debug)]
pub struct CmdHeader {
    #[packed_field(element_size_bytes = "1")]
    cmd: Cmd,
    length: u8,
    hw_addr: u8,
}

impl CmdHeader {
    /// Create a new header with custom checksum_size
    ///
    /// * `length` - size of the command excluding checksum
    /// * `checksum_size` - Size of checksum needs to be known as it is accounted in the length
    /// field
    fn new_extended(
        code: u8,
        length: usize,
        chip_address: ChipAddress,
        checksum_size: usize,
    ) -> Self {
        Self {
            cmd: Cmd::new(code, chip_address.is_broadcast()),
            length: (length + checksum_size) as u8,
            hw_addr: chip_address.to_hw_addr(),
        }
    }

    /// Helper builder for control commands
    /// Control commands CRC5 checksum that fits into 1 byte
    /// * `length` - length of the command without checksum
    fn new(code: u8, length: usize, chip_address: ChipAddress) -> Self {
        Self::new_extended(code, length, chip_address, size_of::<u8>())
    }
}

/// Command response
#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "msb")]
pub struct CmdResponse {
    pub value: u32,
    _zero_in_bm1387_but_its_chip_address_in_bm1391: u8,
    _zero_in_bm1387_but_its_register_number_in_bm1391: u8,
}

/// Sets configuration register
#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "msb")]
pub struct SetConfigCmd {
    #[packed_field(element_size_bytes = "3")]
    pub header: CmdHeader,
    register: u8,
    value: u32,
}

impl SetConfigCmd {
    pub fn new(chip_address: ChipAddress, register: u8, value: u32) -> Self {
        // payload consists of 1 byte register address and 4 byte value
        let header = CmdHeader::new(0x08, Self::packed_bytes(), chip_address);
        Self {
            header,
            register,
            value,
        }
    }
}

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "msb")]
pub struct GetStatusCmd {
    #[packed_field(element_size_bytes = "3")]
    header: CmdHeader,
    register: u8,
}

impl GetStatusCmd {
    pub fn new(chip_address: ChipAddress, register: u8) -> Self {
        let header = CmdHeader::new(0x04, Self::packed_bytes(), chip_address);
        Self { header, register }
    }
}

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "msb")]
pub struct SetChipAddressCmd {
    #[packed_field(element_size_bytes = "3")]
    pub header: CmdHeader,
    _reserved: u8,
}

impl SetChipAddressCmd {
    pub fn new(chip_address: ChipAddress) -> Self {
        assert!(!chip_address.is_broadcast());
        let header = CmdHeader::new(0x01, Self::packed_bytes(), chip_address);
        Self {
            header,
            _reserved: 0,
        }
    }
}

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "msb")]
pub struct InactivateFromChainCmd {
    #[packed_field(element_size_bytes = "3")]
    header: CmdHeader,
    _reserved: u8,
}

impl InactivateFromChainCmd {
    pub fn new() -> Self {
        let header = CmdHeader::new(0x05, Self::packed_bytes(), ChipAddress::All);
        Self {
            header,
            _reserved: 0,
        }
    }
}

/// `Register` trait represents register on chip. Register:
///
/// * supports being serialized from/to register format (`from_reg`/`to_reg`)
/// * register is identified by address on chip (`REG_NUM`)
/// * is 4 bytes long (one "word")
///
/// Chip registers can be read with `GetStatusCmd` and written with  `SetConfigCmd`.
pub trait Register: PackedStruct<[u8; 4]> + Send + Sync + PartialEq + Debug {
    const REG_NUM: u8;

    /// Take register and unpack (as big endian)
    fn from_reg(reg: u32) -> Self {
        Self::unpack(&reg.to_be_bytes()).expect("unpacking error")
    }
    /// Pack into big-endian register
    fn to_reg(&self) -> u32 {
        u32::from_be_bytes(self.pack())
    }
}

#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(endian = "msb", size_bytes = "4")]
pub struct HashrateReg {
    // hashrate in 2^24 hash units
    pub hashrate24: u32,
}

impl HashrateReg {
    pub fn hashrate(&self) -> u64 {
        (self.hashrate24 as u64) << 24
    }
}

impl Register for HashrateReg {
    const REG_NUM: u8 = 0x08;
}

#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(size_bytes = "1", bit_numbering = "lsb0")]
pub struct I2cControlFlags {
    /// I2C controller is busy flag
    #[packed_field(bits = "7")]
    pub busy: bool,
    /// Initiate I2C transaction flag
    #[packed_field(bits = "0")]
    pub do_command: bool,
}

#[derive(PackedStruct, Debug, Clone, PartialEq)]
#[packed_struct(endian = "msb", size_bytes = "4")]
pub struct I2cControlReg {
    /// I2C controller status/control
    #[packed_field(element_size_bytes = "1")]
    pub flags: I2cControlFlags,
    /// I2C address (8-bit format, use odd address for writing)
    pub addr: u8,
    /// Register number
    pub reg: u8,
    /// For read: data that were read, for write: data to write
    pub data: u8,
}

impl Register for I2cControlReg {
    const REG_NUM: u8 = 0x20;
}

#[derive(PackedStruct, Debug, Clone, PartialEq, Default)]
#[packed_struct(endian = "msb", size_bytes = "4")]
pub struct GetAddressReg {
    #[packed_field(ty = "enum", element_size_bytes = "2")]
    pub chip_rev: ChipRev,
    _reserved1: u8,
    pub addr: u8,
}

impl Register for GetAddressReg {
    const REG_NUM: u8 = 0x00;
}

/// Describes recognized chip revisions
#[derive(PrimitiveEnum_u16, Clone, Copy, Debug, PartialEq)]
pub enum ChipRev {
    Bm1387 = 0x1387,
}

impl Default for ChipRev {
    fn default() -> ChipRev {
        ChipRev::Bm1387
    }
}

/// This register represents ASIC difficulty
///
/// The chip will provide only solutions that are <= target based on this difficulty
#[derive(PackedStruct, Debug, PartialEq)]
#[packed_struct(size_bytes = "4", endian = "msb")]
pub struct TicketMaskReg {
    /// stores difficulty - 1
    diff: u32,
}

impl TicketMaskReg {
    /// Builds ticket mask register instance and verifies the specified difficulty is correct
    pub fn new(diff: u32) -> error::Result<Self> {
        if diff == 0 {
            Err(ErrorKind::General(format!(
                "Asic difficulty must be at least 1!",
            )))?
        }
        Ok(Self { diff: diff - 1 })
    }
}

impl Register for TicketMaskReg {
    const REG_NUM: u8 = 0x18;
}

/// TF pin selector
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum TfSelector {
    /// Chip is hashing
    HashDoing = 0, // name from bm1387 datasheet
    UartReceiving = 1,
    UartTransmitting = 2,
    /// Required for I2C
    SCL0 = 3,
}

/// RF pin selector
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum RfSelector {
    OpenDrain = 0,
    /// Required for I2c
    SDA0 = 1,
}

/// Names of I2C buses connected to bm1387
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum I2cBusSelect {
    Bottom = 0,
    Middle = 1,
}

/// Core register that configures the most important aspects of the mining chip like:
///
/// - baud rate/communication speed
/// - multi-midstate processing (AsicBoost)
///
/// All the fields below have been identified in bmminer-mix sources. Meaning of some of them may
/// still be a bit unclear.
///
/// TODO: research set_baud_with_addr() in bmminer-mix as there seems to be some magic setting
/// I2C interface of the chip or something like that
#[derive(PackedStruct, Clone, Debug, PartialEq)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "4", endian = "msb")]
pub struct MiscCtrlReg {
    /// Exact meaning of this field is unknown, when setting baud rate, it is 0, when
    /// initializing the chain it is 1
    #[packed_field(bits = "30")]
    pub not_set_baud: bool,

    /// Invert clock pin -> used on S9's
    #[packed_field(bits = "21")]
    pub inv_clock: bool,

    /// Selects on which I2C bus to communicate
    /// This info was gathered from bmminer
    /// This field (23:16) is called "addr" in 1387 datasheet
    #[packed_field(bits = "16", ty = "enum")]
    pub i2c_bus: I2cBusSelect,

    /// This field causes all blocks of the hashing chip to ignore any incoming
    /// work and allows enabling the blocks one-by-one when a mining work with bit[0] set to 1
    /// arrives
    #[packed_field(bits = "15")]
    pub gate_block: bool,

    /// RF pin function
    /// Info from bm1387 datasheet
    #[packed_field(bits = "14", ty = "enum")]
    pub rfs: RfSelector,

    /// baudrate divisor - maximum divisor is 26. To calculate the divisor:
    /// baud_div = min(OSC/8*baud - 1, 26)
    /// Oscillator frequency is 25 MHz
    ///
    /// **Note**: This field has to be always set to correct UART baud rate,
    /// no matter what value you set to `not_set_baud` (this was found out
    /// experimentally).
    #[packed_field(bits = "12:8")]
    pub baud_div: Integer<u8, packed_bits::Bits5>,

    /// Enable multi midstate processing = "AsicBoost"
    #[packed_field(bits = "7")]
    pub mmen: bool,

    #[packed_field(bits = "5:6", ty = "enum")]
    pub tfs: TfSelector,
}

impl MiscCtrlReg {
    /// Builds register instance and sanity checks the divisor for the baud rate generator
    pub fn new(
        not_set_baud: bool,
        inv_clock: bool,
        baud_div: usize,
        gate_block: bool,
        mmen: bool,
    ) -> error::Result<Self> {
        if baud_div > MAX_BAUD_CLOCK_DIV {
            Err(ErrorKind::BaudRate(format!(
                "divisor {} is out of range, maximum allowed is {}",
                baud_div, MAX_BAUD_CLOCK_DIV
            )))?
        }
        Ok(Self {
            not_set_baud,
            inv_clock,
            baud_div: (baud_div as u8).into(),
            gate_block,
            mmen,
            tfs: TfSelector::HashDoing,
            rfs: RfSelector::OpenDrain,
            i2c_bus: I2cBusSelect::Bottom,
        })
    }

    /// Alter the value of MiscCtrl register to enable I2C
    ///
    /// When we enable/disable I2C on chip, we want to leave the rest of the settings
    /// as they are. This is why this call alters the register - it is intended
    /// to be a part of read-modify-write cycle.
    ///
    /// `i2c_bus` selects the bus or disables the I2C controller (when `None`)
    pub fn set_i2c(&mut self, i2c_bus: Option<I2cBusSelect>) {
        // These two are meaningful only during initialization so we
        // should better clear them.
        self.not_set_baud = true;
        self.gate_block = false;

        if let Some(i2c_bus) = i2c_bus {
            self.tfs = TfSelector::SCL0;
            self.rfs = RfSelector::SDA0;
            self.i2c_bus = i2c_bus;
        } else {
            self.tfs = TfSelector::HashDoing;
            self.rfs = RfSelector::OpenDrain;
            self.i2c_bus = I2cBusSelect::Bottom;
        }
    }
}

impl Register for MiscCtrlReg {
    const REG_NUM: u8 = 0x1c;
}

/// Structure representing settings of chip PLL divider
/// It can serialize itself right to register settings
#[derive(PackedStruct, Debug, PartialEq, Clone)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "4", endian = "msb")]
pub struct PllReg {
    /// Range: 60..=320, but in datasheet table: 32..=128
    #[packed_field(bits = "23:16")]
    fbdiv: u8,
    /// Range: 1..=63, but in datasheet always 2
    #[packed_field(bits = "11:8")]
    refdiv: u8,
    /// Range: 1..=7
    #[packed_field(bits = "7:4")]
    postdiv1: u8,
    /// Range: 1..=7, but in datasheet always 1
    /// Also must hold: postdiv2 <= postdiv1
    #[packed_field(bits = "3:0")]
    postdiv2: u8,
}

impl PllReg {
    /// Minimum and maximum supported frequency
    const MIN_FREQ: usize = 100_000_000;
    const MAX_FREQ: usize = 1_200_000_000;

    /// Simulate divider/PLL and calculate target frequency
    pub fn calc(&self, xtal_freq: usize) -> usize {
        // we have to do the arithmetic in u64 (at least) to be sure
        // there wouldn't be an overflow
        (xtal_freq as u64 * self.fbdiv as u64
            / self.refdiv as u64
            / self.postdiv1 as u64
            / self.postdiv2 as u64) as usize
    }

    /// Find error between target frequency and computed frequency
    fn calculate_error(&self, xtal_freq: usize, target_freq: usize) -> usize {
        (self.calc(xtal_freq) as i64 - target_freq as i64).abs() as usize
    }

    /// Find divider to approximate `target_freq`
    fn find_divider(xtal_freq: usize, target_freq: usize) -> Self {
        let mut best = PllReg {
            fbdiv: 0,
            refdiv: 1,
            postdiv1: 1,
            postdiv2: 1,
        };

        // range of `fbdiv` is supposed to be 60..320, but:
        // - there are pre-computed entries with `fbdiv` as low as 32
        // - there are not precomputed entries with `fbdiv` higher than 128
        // - refdiv and postdiv2 are in tables always fixed
        for fbdiv in 32..128 {
            for postdiv1 in 1..=7 {
                let pll = PllReg {
                    fbdiv,
                    refdiv: 2,
                    postdiv1,
                    postdiv2: 1,
                };
                if pll.calculate_error(xtal_freq, target_freq)
                    < best.calculate_error(xtal_freq, target_freq)
                {
                    best = pll;
                }
            }
        }
        best
    }

    /// Try to calculate PLL divider settings to approximate target frequency
    ///
    /// * `xtal_freq` - frequency (in MHz) of crystal (that is to be multiplied/divided by PLL)
    /// * `target_freq` - frequency (in MHz) that is to be approximated
    ///
    /// This function always returns some PLL divider if frequency is in range.
    pub fn try_pll_from_freq(xtal_freq: usize, target_freq: usize) -> error::Result<Self> {
        if target_freq < Self::MIN_FREQ || target_freq > Self::MAX_FREQ {
            Err(ErrorKind::PLL(format!(
                "Requested frequency {} out of range!",
                target_freq
            )))?
        }
        let pll = Self::find_divider(xtal_freq, target_freq);
        Ok(pll)
    }
}

impl Register for PllReg {
    const REG_NUM: u8 = 0x0c;
}

#[cfg(test)]
mod test {
    use super::*;

    /// Default S9 clock frequency
    const DEFAULT_XTAL_FREQ: usize = 25_000_000;

    /// Test chip address contstruction
    #[test]
    fn test_chip_address() {
        let all = ChipAddress::All;
        assert!(all.is_broadcast());
        assert_eq!(all.to_hw_addr(), 0);

        let one = ChipAddress::One(9);
        assert!(!one.is_broadcast());
        assert_eq!(one.to_hw_addr(), 0x24);
    }

    #[test]
    #[should_panic]
    fn test_chip_address_too_big() {
        // address is too big to fit in a u8
        ChipAddress::One(0x40).to_hw_addr();
    }

    /// Builds a sample set_config command (here the PLL register @ 0x0c with a value of
    /// 0x00680221 that corresponds to
    /// and verifies correct serialization
    #[test]
    fn build_set_config_cmd_pll() {
        let cmd = SetConfigCmd::new(ChipAddress::One(9), PllReg::REG_NUM, 0x680221);
        let expected_cmd_with_padding =
            [0x48u8, 0x09, 0x24, PllReg::REG_NUM, 0x00, 0x68, 0x02, 0x21];
        let cmd_bytes = cmd.pack();
        assert_eq!(
            cmd_bytes, expected_cmd_with_padding,
            "Incorrectly composed command:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            cmd, cmd_bytes, expected_cmd_with_padding
        );
    }

    /// Verify serialization of SetConfig(TICKET_MASK(0x3f)) command
    #[test]
    fn build_set_config_ticket_mask() {
        let reg = TicketMaskReg::new(64).expect("Cannot build difficulty register");
        let cmd = SetConfigCmd::new(ChipAddress::All, TicketMaskReg::REG_NUM, reg.to_reg());
        let expected_cmd_with_padding = [0x58u8, 0x09, 0x00, 0x18, 0x00, 0x00, 0x00, 0x3f];
        let cmd_bytes = cmd.pack();
        assert_eq!(cmd_bytes, expected_cmd_with_padding);
    }

    /// Verify serialization of SetConfig(MISC_CONTROL(...)) command
    #[test]
    fn build_set_config_misc_control() {
        let reg = MiscCtrlReg {
            not_set_baud: true,
            inv_clock: true,
            baud_div: 26.into(),
            gate_block: true,
            mmen: true,
            tfs: TfSelector::HashDoing,
            rfs: RfSelector::OpenDrain,
            i2c_bus: I2cBusSelect::Bottom,
        };
        let cmd = SetConfigCmd::new(ChipAddress::All, MiscCtrlReg::REG_NUM, reg.to_reg());
        let expected_cmd_with_padding = [0x58u8, 0x09, 0x00, 0x1c, 0x40, 0x20, 0x9a, 0x80];
        let cmd_bytes = cmd.pack();
        assert_eq!(cmd_bytes, expected_cmd_with_padding);
        // MiscCtrlReg constructor should build the same structure
        assert_eq!(
            reg,
            MiscCtrlReg::new(true, true, 26, true, true).expect("invalid divisor")
        );
    }

    /// Verify serialization of SetConfig(MISC_CONTROL(...)) command for I2C
    #[test]
    fn build_set_config_misc_control_i2c() {
        let reg = MiscCtrlReg {
            not_set_baud: true,
            inv_clock: true,
            baud_div: 26.into(),
            gate_block: false,
            tfs: TfSelector::SCL0,
            rfs: RfSelector::SDA0,
            i2c_bus: I2cBusSelect::Bottom,
            mmen: true,
        };
        let cmd = SetConfigCmd::new(ChipAddress::All, MiscCtrlReg::REG_NUM, reg.to_reg());
        let expected_cmd_with_padding = [0x58u8, 0x09, 0x00, 0x1c, 0x40, 0x20, 0x5a, 0xe0];
        let cmd_bytes = cmd.pack();
        assert_eq!(cmd_bytes, expected_cmd_with_padding);
        // MiscCtrlReg constructor should build the same structure
        let mut misc_reg = MiscCtrlReg::new(true, true, 26, false, true).expect("invalid divisor");
        misc_reg.set_i2c(Some(I2cBusSelect::Bottom));
        assert_eq!(reg, misc_reg,);
    }

    /// Builds a get status command to read chip address of all chips
    #[test]
    fn build_get_status_cmd() {
        let cmd = GetStatusCmd::new(ChipAddress::All, GetAddressReg::REG_NUM);
        let expected_cmd_with_padding = [0x54u8, 0x05, 0x00, 0x00];

        let cmd_bytes = cmd.pack();
        assert_eq!(
            cmd_bytes, expected_cmd_with_padding,
            "Incorrectly composed command:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            cmd, cmd_bytes, expected_cmd_with_padding
        );
    }

    #[test]
    fn build_inactivate_from_chain_cmd() {
        let cmd = InactivateFromChainCmd::new();
        let expected_cmd_with_padding = [0x55u8, 0x05, 0x00, 0x00];

        let cmd_bytes = cmd.pack();
        assert_eq!(
            cmd_bytes, expected_cmd_with_padding,
            "Incorrectly composed command:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            cmd, cmd_bytes, expected_cmd_with_padding
        );
    }

    #[test]
    fn build_set_chip_address_cmd() {
        let cmd = SetChipAddressCmd::new(ChipAddress::One(1));
        let expected_cmd_with_padding = [0x41u8, 0x05, 0x04, 0x00];

        let cmd_bytes = cmd.pack();
        assert_eq!(
            cmd_bytes, expected_cmd_with_padding,
            "Incorrectly composed command:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            cmd, cmd_bytes, expected_cmd_with_padding
        );
    }

    #[test]
    fn build_chip_addr_reg() {
        let reg = GetAddressReg {
            chip_rev: ChipRev::Bm1387,
            _reserved1: 0x90,
            addr: 0x00,
            ..Default::default()
        };
        let expected_reg = [0x13u8, 0x87, 0x90, 0x00];

        let reg_bytes = reg.pack();
        assert_eq!(
            reg_bytes, expected_reg,
            "Incorrectly composed register:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            reg, reg_bytes, expected_reg
        );
    }

    #[test]
    fn test_broken_chip_addr_value() {
        // intentionally specify incorrect/unsupported chip version
        let broken_reg_bytes = [0x13u8, 0x86, 0x90, 0x04, 0x00, 0x00];
        let reg = GetAddressReg::unpack_from_slice(&broken_reg_bytes);
        assert!(
            reg.is_err(),
            "Unpacking should have failed due to incompatible chip version \
             parsed: {:?}, sliced view: {:#04x?}",
            reg,
            broken_reg_bytes
        );
    }

    #[test]
    fn build_misc_control_reg() {
        let reg = MiscCtrlReg {
            not_set_baud: true,
            inv_clock: true,
            baud_div: 26.into(),
            gate_block: true,
            mmen: true,
            tfs: TfSelector::HashDoing,
            rfs: RfSelector::OpenDrain,
            i2c_bus: I2cBusSelect::Bottom,
        };
        let expected_reg_msb = [0x40u8, 0x20, 0x9a, 0x80];
        let reg_bytes = reg.pack();

        assert_eq!(
            reg_bytes, expected_reg_msb,
            "Incorrectly composed register:{:#04x?} sliced view: {:#04x?} expected view: \
             {:#04x?}",
            reg, reg_bytes, expected_reg_msb
        );
    }

    #[test]
    fn test_misc_control_reg_to_u32() {
        let reg = MiscCtrlReg {
            not_set_baud: true,
            inv_clock: true,
            baud_div: 26.into(),
            gate_block: true,
            mmen: true,
            tfs: TfSelector::HashDoing,
            rfs: RfSelector::OpenDrain,
            i2c_bus: I2cBusSelect::Bottom,
        };
        let expected_reg_value = 0x40209a80u32;
        let reg_value: u32 = reg.to_reg();
        assert_eq!(
            reg_value, expected_reg_value,
            "Misc Control Register 32-bit value  doesn't match: V:{:#010x} E:{:#010x}",
            reg_value, expected_reg_value
        );
    }

    #[test]
    fn test_invalid_ticket_mask_reg() {
        let res = TicketMaskReg::new(0);
        assert_eq!(res.is_ok(), false, "Diff 0 should be reported as error!");
    }

    #[test]
    fn test_ticket_mask_reg_to_u32() {
        let reg = TicketMaskReg::new(64).expect("Cannot build difficulty register");

        let expected_reg_value = 0x3fu32;
        let reg_value: u32 = reg.to_reg();
        assert_eq!(
            reg_value, expected_reg_value,
            "Ticket mask register 32-bit value  doesn't match: V:{:#010x} E:{:#010x}",
            reg_value, expected_reg_value
        );
    }

    #[test]
    fn test_hashrate_reg() {
        let reg = HashrateReg { hashrate24: 0x23 };

        assert_eq!(reg.pack(), [0x00, 0x00, 0x00, 0x23]);
        assert_eq!(reg.to_reg(), 0x23);
        assert_eq!(reg.hashrate(), 0x23000000);
    }

    /// Test serialization and evaluation of PLL divider
    fn try_one_divider(freq: usize, reg: u32, fbdiv: u8, refdiv: u8, postdiv1: u8, postdiv2: u8) {
        let pll = PllReg {
            fbdiv,
            refdiv,
            postdiv1,
            postdiv2,
        };
        let xin = DEFAULT_XTAL_FREQ;
        assert_eq!(pll.calc(xin), freq);
        assert_eq!(pll.calculate_error(xin, freq - 500), 500);
        assert_eq!(pll.to_reg(), reg);
    }

    #[test]
    fn test_pll_computation() {
        try_one_divider(100_000_000, 0x200241, 0x20, 2, 4, 1);
        try_one_divider(375_000_000, 0x780241, 0x78, 2, 4, 1);
        try_one_divider(431_250_000, 0x450221, 0x45, 2, 2, 1);
        try_one_divider(466_666_666, 0x700231, 0x70, 2, 3, 1);
        try_one_divider(500_000_000, 0x500221, 0x50, 2, 2, 1);
        try_one_divider(593_750_000, 0x5f0221, 0x5f, 2, 2, 1);
        try_one_divider(650_000_000, 0x680221, 0x68, 2, 2, 1);
        try_one_divider(718_750_000, 0x730221, 0x73, 2, 2, 1);
        try_one_divider(1000_000_000, 0x500211, 0x50, 2, 1, 1);
        try_one_divider(1175_000_000, 0x5e0211, 0x5e, 2, 1, 1);
    }

    #[test]
    fn test_pll_search() {
        // should fail: too low
        assert!(PllReg::try_pll_from_freq(DEFAULT_XTAL_FREQ, 50_000_000).is_err());
        // should fail: too high
        assert!(PllReg::try_pll_from_freq(DEFAULT_XTAL_FREQ, 2_000_000_000).is_err());
        // ok
        assert_eq!(
            PllReg::try_pll_from_freq(DEFAULT_XTAL_FREQ, 650_000_000).expect("pll is ok"),
            PllReg {
                fbdiv: 0x34,
                refdiv: 2,
                postdiv1: 1,
                postdiv2: 1
            }
        );
    }

    #[test]
    fn test_core_address() {
        assert_eq!(
            CoreAddress::new(0xffffffff),
            CoreAddress {
                chip: 0x3f,
                core: 0x7f
            }
        );
        assert_eq!(
            CoreAddress::new(0x2a105d5d),
            CoreAddress { chip: 23, core: 42 }
        );
        assert_eq!(
            CoreAddress::new(0xd25738d3),
            CoreAddress { chip: 52, core: 82 }
        );
        assert_eq!(
            CoreAddress::new(0x47268d19),
            CoreAddress { chip: 6, core: 71 }
        );
        assert_eq!(
            CoreAddress::new(0xa5e09223),
            CoreAddress { chip: 8, core: 37 }
        );
        assert_eq!(
            CoreAddress::new(0xd57c1ce4),
            CoreAddress { chip: 57, core: 85 }
        );
        assert_eq!(
            CoreAddress::new(0x40e55650),
            CoreAddress { chip: 20, core: 64 }
        );
    }
}
