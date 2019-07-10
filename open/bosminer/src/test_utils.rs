use crate::btc;
use crate::hal::{self, BitcoinJob};

use lazy_static::lazy_static;
use std::sync::Arc;

use bitcoin_hashes::{hex::FromHex, sha256d::Hash, Hash as HashTrait};
use byteorder::{ByteOrder, LittleEndian};

/// DummyJob to be used for tests
#[derive(Debug, Copy, Clone)]
pub struct DummyJob {
    hash: Hash,
    time: u32,
}

impl DummyJob {
    pub fn new(time: u32) -> Self {
        Self {
            hash: Hash::from_slice(&[0xffu8; 32]).unwrap(),
            time,
        }
    }

    pub fn next(&mut self) {
        self.time += 1;
    }
}

impl hal::BitcoinJob for DummyJob {
    fn version(&self) -> u32 {
        0
    }

    fn version_mask(&self) -> u32 {
        0
    }

    fn previous_hash(&self) -> &Hash {
        &self.hash
    }

    fn merkle_root(&self) -> &Hash {
        &self.hash
    }

    fn time(&self) -> u32 {
        self.time
    }

    fn bits(&self) -> u32 {
        0xffff_ffff
    }

    fn is_valid(&self) -> bool {
        true
    }
}

/// Real blocks used for tests
#[derive(Copy, Clone)]
pub struct TestBlock {
    pub hash: Hash,
    pub hash_str: &'static str,
    pub midstate: btc::Midstate,
    pub midstate_str: &'static str,
    version: u32,
    prev_hash: Hash,
    merkle_root: Hash,
    time: u32,
    bits: u32,
    pub nonce: u32,
    pub header_bytes: [u8; 80],
}

impl TestBlock {
    pub fn new(
        hash: &'static str,
        midstate: &'static str,
        version: u32,
        prev_hash: &str,
        merkle_root: &str,
        time: u32,
        bits: u32,
        nonce: u32,
        header_bytes: [u8; 80],
    ) -> Self {
        Self {
            hash: Hash::from_hex(hash).expect("parse hex"),
            hash_str: hash,
            midstate: btc::Midstate::from_hex(midstate).expect("parse hex"),
            midstate_str: midstate,
            version,
            prev_hash: Hash::from_hex(prev_hash).expect("parse hex"),
            merkle_root: Hash::from_hex(merkle_root).expect("parse hex"),
            time,
            bits,
            nonce,
            header_bytes,
        }
    }
}

impl std::fmt::Debug for TestBlock {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.hash)
    }
}

impl hal::BitcoinJob for TestBlock {
    fn version(&self) -> u32 {
        self.version
    }

    fn version_mask(&self) -> u32 {
        0
    }

    fn previous_hash(&self) -> &Hash {
        &self.prev_hash
    }

    fn merkle_root(&self) -> &Hash {
        &self.merkle_root
    }

    fn time(&self) -> u32 {
        self.time
    }

    fn bits(&self) -> u32 {
        self.bits
    }

    fn is_valid(&self) -> bool {
        true
    }
}

lazy_static! {
    pub static ref TEST_BLOCKS: Vec<TestBlock> = vec![
        // Block 171874 binary representation
        // https://blockchain.info/rawblock/00000000000004b64108a8e4168cfaa890d62b8c061c6b74305b7f6cb2cf9fda
        TestBlock::new(
            "00000000000004b64108a8e4168cfaa890d62b8c061c6b74305b7f6cb2cf9fda",
            "e48f544a9a3afa71451471134df6c35682b400254bfe0860c99876bf4679ba4e",
            1,
            "0000000000000488d0b6c4c05f24afe4817a122a1e1a5f009dd391fb0cc1aeb3",
            "ce22a72fa0e9f309830fdb3f75d6c95f051f23ef288a137693ab5c03f2bb6e7e",
            1332160020,
            436941447,
            2726756608,
            [ 0x01, 0x00, 0x00, 0x00, 0xb3, 0xae, 0xc1, 0x0c, 0xfb, 0x91, 0xd3, 0x9d, 0x00, 0x5f,
              0x1a, 0x1e, 0x2a, 0x12, 0x7a, 0x81, 0xe4, 0xaf, 0x24, 0x5f, 0xc0, 0xc4, 0xb6, 0xd0,
              0x88, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7e, 0x6e, 0xbb, 0xf2, 0x03, 0x5c,
              0xab, 0x93, 0x76, 0x13, 0x8a, 0x28, 0xef, 0x23, 0x1f, 0x05, 0x5f, 0xc9, 0xd6, 0x75,
              0x3f, 0xdb, 0x0f, 0x83, 0x09, 0xf3, 0xe9, 0xa0, 0x2f, 0xa7, 0x22, 0xce, 0x14, 0x26,
              0x67, 0x4f, 0x87, 0x32, 0x0b, 0x1a, 0x00, 0x01, 0x87, 0xa2,
            ],
        ),
        // Sample block from:
        // https://en.bitcoin.it/wiki/Block_hashing_algorithm
        // https://blockchain.info/rawblock/00000000000000001e8d6829a8a21adc5d38d0a473b144b6765798e61f98bd1d
        TestBlock::new(
            "00000000000000001e8d6829a8a21adc5d38d0a473b144b6765798e61f98bd1d",
            "9524c59305c5671316e669ba2d2810a007e86e372f56a9dacd5bce697a78da2d",
            1,
            "00000000000008a3a41b85b8b29ad444def299fee21793cd8b9e567eab02cd81",
            "2b12fcf1b09288fcaff797d71e950e71ae42b91e8bdb2304758dfcffc2b620e3",
            1305998791,
            440711666,
            2504433986,
            [ 0x01, 0x00, 0x00, 0x00, 0x81, 0xcd, 0x02, 0xab, 0x7e, 0x56, 0x9e, 0x8b, 0xcd, 0x93,
              0x17, 0xe2, 0xfe, 0x99, 0xf2, 0xde, 0x44, 0xd4, 0x9a, 0xb2, 0xb8, 0x85, 0x1b, 0xa4,
              0xa3, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe3, 0x20, 0xb6, 0xc2, 0xff, 0xfc,
              0x8d, 0x75, 0x04, 0x23, 0xdb, 0x8b, 0x1e, 0xb9, 0x42, 0xae, 0x71, 0x0e, 0x95, 0x1e,
              0xd7, 0x97, 0xf7, 0xaf, 0xfc, 0x88, 0x92, 0xb0, 0xf1, 0xfc, 0x12, 0x2b, 0xc7, 0xf5,
              0xd7, 0x4d, 0xf2, 0xb9, 0x44, 0x1a, 0x42, 0xa1, 0x46, 0x95,
            ],
        ),
        // Sample block v4:
        // https://blockchain.info/rawblock/00000000000000000024974128beb85f6f39d009538f4d92c64d4b82da8a2660
        TestBlock::new(
            "00000000000000000024974128beb85f6f39d009538f4d92c64d4b82da8a2660",
            "9a8378bb5dfc122384cf590facbb1c5af6eca129c32db4a840301c8a60f72b57",
            536870912,
            "000000000000000000262b17185b3c94dff2ab1c4ff6dacb884a80527ec1725d",
            "70ee9e04d1d030770c7c1fda029813067c9327f3b0bde8821666ecf94321ef14",
            1555576766,
            388761373,
            4115486663,
            [ 0x00, 0x00, 0x00, 0x20, 0x5d, 0x72, 0xc1, 0x7e, 0x52, 0x80, 0x4a, 0x88, 0xcb, 0xda,
              0xf6, 0x4f, 0x1c, 0xab, 0xf2, 0xdf, 0x94, 0x3c, 0x5b, 0x18, 0x17, 0x2b, 0x26, 0x00,
              0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0xef, 0x21, 0x43, 0xf9, 0xec,
              0x66, 0x16, 0x82, 0xe8, 0xbd, 0xb0, 0xf3, 0x27, 0x93, 0x7c, 0x06, 0x13, 0x98, 0x02,
              0xda, 0x1f, 0x7c, 0x0c, 0x77, 0x30, 0xd0, 0xd1, 0x04, 0x9e, 0xee, 0x70, 0xbe, 0x37,
              0xb8, 0x5c, 0x1d, 0x07, 0x2c, 0x17, 0xc7, 0x57, 0x4d, 0xf5,
            ],
        )
    ];
}

/// * `i` - unique identifier for the generated midstate
pub fn prepare_test_work(i: u64) -> hal::MiningWork {
    let job = Arc::new(DummyJob::new(0));
    let time = job.time();

    let mut midstate_bytes = [0u8; btc::SHA256_DIGEST_SIZE];
    LittleEndian::write_u64(&mut midstate_bytes, i);

    let mid = hal::Midstate {
        version: 0,
        state: midstate_bytes.into(),
    };

    hal::MiningWork {
        job,
        midstates: vec![mid],
        ntime: time,
    }
}
