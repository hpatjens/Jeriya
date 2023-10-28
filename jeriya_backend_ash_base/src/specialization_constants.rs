use std::{
    io::{self, Cursor},
    mem,
};

use ash::vk;
use jeriya_shared::byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

pub trait PushSpecializationConstant {
    /// Writes the specialization constant to the given vector and returns the offset in the vector
    fn push(&self, target: &mut Vec<u8>) -> u32;

    /// Returns the size of the specialization constant in bytes
    fn byte_size() -> usize;
}

impl PushSpecializationConstant for u32 {
    fn push(&self, target: &mut Vec<u8>) -> u32 {
        let offset = target.len();
        target
            .write_u32::<LittleEndian>(*self)
            .expect("failed to write u32 to specialization constant buffer");
        offset as u32
    }

    fn byte_size() -> usize {
        mem::size_of::<Self>()
    }
}

#[derive(Debug, Default, Clone)]
pub struct SpecializationConstants {
    map_entries: Vec<vk::SpecializationMapEntry>,
    data: Vec<u8>,
}

impl SpecializationConstants {
    /// Creates an empty set of specialization constants
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the specialization constants to the `SpecializationConstants`
    pub fn push<T: PushSpecializationConstant>(&mut self, constant_id: u32, value: T) -> &mut Self {
        // Push the value to the data `Vec`
        let offset = value.push(&mut self.data);

        // Push the map entry
        self.map_entries.push(
            vk::SpecializationMapEntry::builder()
                .constant_id(constant_id)
                .offset(offset)
                .size(T::byte_size())
                .build(),
        );

        self
    }

    /// Returns the number of specialization constants
    pub fn len(&self) -> usize {
        self.map_entries.len()
    }

    /// Returns `true` if the specialization constants are empty
    pub fn is_empty(&self) -> bool {
        self.map_entries.is_empty()
    }

    // Returns the value of the specialization constant with the given ID
    //
    // This function returns `None` if the specialization constant with the given ID does not exist and an `Err` might be returned if the data is corrupted.
    pub fn read_u32(&self, constant_id: u32) -> Option<io::Result<u32>> {
        self.map_entries.iter().find(|entry| entry.constant_id == constant_id).map(|entry| {
            let offset = entry.offset as usize;
            let end = offset + entry.size;
            let bytes = &self.data[offset..end];
            let mut cursor = Cursor::new(&bytes);
            cursor.read_u32::<LittleEndian>()
        })
    }

    /// Returns the map entries
    pub fn map_entries(&self) -> &[vk::SpecializationMapEntry] {
        &self.map_entries
    }

    /// Returns the data
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut specialization_constants = SpecializationConstants::new();
        assert_eq!(specialization_constants.len(), 0);
        assert_eq!(specialization_constants.is_empty(), true);

        specialization_constants.push(0, 73u32);
        assert_eq!(specialization_constants.len(), 1);
        assert_eq!(specialization_constants.is_empty(), false);

        specialization_constants.push(2, 12u32);
        assert_eq!(specialization_constants.len(), 2);
        assert_eq!(specialization_constants.is_empty(), false);

        specialization_constants.push(1, 5u32);
        assert_eq!(specialization_constants.len(), 3);
        assert_eq!(specialization_constants.is_empty(), false);

        assert_eq!(specialization_constants.read_u32(1).unwrap().unwrap(), 5);
        assert_eq!(specialization_constants.read_u32(0).unwrap().unwrap(), 73);
        assert_eq!(specialization_constants.read_u32(2).unwrap().unwrap(), 12);
    }
}
