#![allow(unused)]
use std::{
    error::Error,
    io::{self, Write}, iter,
};

use serde::Serialize;

const PAGE_SIZE: usize = 4096;
const ROW_SIZE: usize = 256;

struct Page {
    data: Vec<u8>,
}

pub enum DbError {
    Io(io::Error),
    Serialize(Box<dyn Error>),
}

impl Page {
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(PAGE_SIZE),
        }
    }

    pub fn insert<S: Serialize>(&mut self, row: S) -> Result<(), DbError> {
        let serialized =
            bitcode::serialize(&row).map_err(|err| DbError::Serialize(Box::new(err)))?;
        let size = serialized.len() as u64;
        let size = size.to_be_bytes();

        self.data.write(&size).map_err(DbError::Io)?;
        self.data.write(&serialized).map_err(DbError::Io)?;
        self.data
            .write(&vec![0; ROW_SIZE - (serialized.len() + size.len())])
            .map_err(DbError::Io)?;

        Ok(())
    }

    pub fn rows(&self) -> impl Iterator<Item = Vec<u8>> + '_ {
        let mut cursor = 0;
        iter::from_fn(move || {
            let offset = cursor * ROW_SIZE;
            if offset + ROW_SIZE > self.data.len() {
                return None
            }

            let row = &self.data[offset..offset + ROW_SIZE];
            let size = {
                let mut buf = [0; 8];
                buf.copy_from_slice(&row[0..8]);
                u64::from_be_bytes(buf) as usize
            };

            cursor += 1;
            Some(row[8..8 + size].to_vec())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_into_page() {
        let mut page = Page::new();
        page.insert(String::from("Rinha"));
        page.insert(String::from("de"));
        page.insert(String::from("Backend"));
        page.insert(2024 as u64);

        let mut rows = page.rows();
        assert_eq!("Rinha", bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap());
        assert_eq!("de", bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap());
        assert_eq!("Backend", bitcode::deserialize::<String>(&rows.next().unwrap()).unwrap());
        assert_eq!(2024, bitcode::deserialize::<u64>(&rows.next().unwrap()).unwrap());
        assert!(rows.next().is_none());
    }
}
