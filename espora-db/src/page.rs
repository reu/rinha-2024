use std::{
    io::{Cursor, Seek, Write},
    iter,
};

use serde::Serialize;

use crate::DbResult;

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Page<const ROW_SIZE: usize> {
    data: Vec<u8>,
    free: usize,
}

impl<const ROW_SIZE: usize> Page<ROW_SIZE> {
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(PAGE_SIZE),
            free: PAGE_SIZE,
        }
    }

    pub fn from_bytes(data: Vec<u8>) -> Self {
        let free = {
            let mut cursor = 0;
            let last_page_offset = iter::from_fn(|| {
                let offset = cursor * ROW_SIZE;
                if offset + ROW_SIZE > data.len() {
                    return None;
                }
                cursor += 1;

                if data[offset..offset + 8] != [0; 8] {
                    Some(offset + ROW_SIZE)
                } else {
                    None
                }
            })
            .last()
            .unwrap_or(0);
            PAGE_SIZE - last_page_offset
        };

        Self { data, free }
    }

    pub fn insert<S: Serialize>(&mut self, row: S) -> DbResult<()> {
        let serialized = bitcode::serialize(&row)?;
        let size = serialized.len() as u64;
        let size = size.to_be_bytes();

        let mut cursor = Cursor::new(&mut self.data);
        cursor.seek(std::io::SeekFrom::Start((PAGE_SIZE - self.free) as u64))?;

        self.free -= cursor.write(&size)?;
        self.free -= cursor.write(&serialized)?;
        self.free -= cursor.write(&vec![0; ROW_SIZE - (serialized.len() + size.len())])?;

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

    #[test]
    fn test_initialize() {
        let page = Page::<1024>::new();
        assert_eq!(0, page.len());
        assert_eq!(PAGE_SIZE, page.free);
    }

    #[test]
    fn test_from_empty_bytes() {
        let page = Page::<1024>::from_bytes(vec![]);
        assert_eq!(0, page.len());
        assert_eq!(PAGE_SIZE, page.free);
    }

    #[test]
    fn test_from_bytes() {
        let mut page = Page::<1024>::from_bytes(vec![]);
        page.insert(1).unwrap();
        page.insert(2).unwrap();

        let new_page = Page::<1024>::from_bytes(page.as_ref().to_vec());
        assert_eq!(page.len(), new_page.len());
        assert_eq!(page.available_rows(), new_page.available_rows());
        assert_eq!(page.free, new_page.free);
    }

    #[test]
    fn test_update_existing_page() {
        let mut page = Page::<1024>::from_bytes(vec![]);
        page.insert("Rinha").unwrap();
        page.insert("de").unwrap();

        let mut page = Page::<1024>::from_bytes(page.as_ref().to_vec());
        page.insert("Backend").unwrap();
        page.insert("2024").unwrap();

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
            "Backend",
            bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap()
        );
        assert_eq!(
            "2024",
            bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap()
        );
        assert!(rows.next().is_none());
    }
}
