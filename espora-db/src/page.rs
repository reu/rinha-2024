use std::{io::Write, iter};

use serde::Serialize;

use crate::DbResult;

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Page<const ROW_SIZE: usize = 64> {
    data: Vec<u8>,
}

impl<const ROW_SIZE: usize> Page<ROW_SIZE> {
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(PAGE_SIZE),
        }
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, &'static str> {
        if data.len() != PAGE_SIZE {
            return Err("Invalid data size");
        }
        Ok(Self { data })
    }

    pub fn insert<S: Serialize>(&mut self, row: S) -> DbResult<()> {
        let serialized = bitcode::serialize(&row)?;
        let size = serialized.len() as u64;
        let size = size.to_be_bytes();

        self.data.write_all(&size)?;
        self.data.write_all(&serialized)?;
        self.data
            .write_all(&vec![0; ROW_SIZE - (serialized.len() + size.len())])?;

        Ok(())
    }

    pub fn rows(&self) -> impl Iterator<Item = &[u8]> {
        let mut cursor = 0;
        iter::from_fn(move || {
            let offset = cursor * ROW_SIZE;
            if offset + ROW_SIZE > self.data.len() {
                return None;
            }

            let row = &self.data[offset..offset + ROW_SIZE];
            let size = {
                let mut buf = [0; 8];
                buf.copy_from_slice(&row[0..8]);
                u64::from_be_bytes(buf) as usize
            };

            if size == 0 {
                return None;
            }

            cursor += 1;
            Some(&row[8..8 + size])
        })
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn available_rows(&self) -> usize {
        (PAGE_SIZE - self.data.len()) / ROW_SIZE
    }
}

impl<const ROW_SIZE: usize> AsRef<[u8]> for Page<ROW_SIZE> {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_into_page() {
        let mut page = Page::<1024>::new();
        assert_eq!(4, page.available_rows());
        page.insert(String::from("Rinha")).unwrap();
        assert_eq!(3, page.available_rows());
        page.insert(String::from("de")).unwrap();
        assert_eq!(2, page.available_rows());
        page.insert(2024 as u64).unwrap();
        assert_eq!(1, page.available_rows());

        let mut rows = page.rows();
        assert_eq!(
            "Rinha",
            bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap()
        );
        assert_eq!(
            "de",
            bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap()
        );
        assert_eq!(
            2024,
            bitcode::deserialize::<u64>(&rows.next().unwrap()).unwrap()
        );
        assert!(rows.next().is_none());
    }
}
