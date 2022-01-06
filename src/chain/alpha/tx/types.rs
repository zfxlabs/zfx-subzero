use byteorder::BigEndian;
use zerocopy::{
    byteorder::U64, AsBytes, FromBytes, Unaligned,
};

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Key {
    hash: [u8; 32],
}

impl Key {
    pub fn new(h: [u8; 32]) -> Key {
	Key { hash: h }
    }
}
