#![allow(unused)]
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    iter,
    marker::PhantomData,
    path::Path,
};

use serde::{de::DeserializeOwned, Serialize};

const PAGE_SIZE: usize = 4096;

struct Page<const ROW_SIZE: usize = 64> {
    data: Vec<u8>,
}

pub enum DbError {
    Io(io::Error),
    Serialize(Box<dyn Error>),
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

    pub fn insert<S: Serialize>(&mut self, row: S) -> Result<(), DbError> {
        let serialized =
            bitcode::serialize(&row).map_err(|err| DbError::Serialize(Box::new(err)))?;
        let size = serialized.len() as u64;
        let size = size.to_be_bytes();

        self.data.write(&size).map_err(DbError::Io)?;
        self.data.write(&serialized).map_err(DbError::Io)?;
        self.data
            .write_all(&vec![0; ROW_SIZE - (serialized.len() + size.len())])
            .map_err(DbError::Io)?;

        Ok(())
    }

    pub fn rows(&self) -> impl Iterator<Item = &[u8]> + '_ {
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

pub struct Db<T, const ROW_SIZE: usize = 64> {
    current_page: Page<ROW_SIZE>,
    reader: File,
    writer: File,
    data: PhantomData<T>,
}

impl<const ROW_SIZE: usize, T: Serialize + DeserializeOwned> Db<T, ROW_SIZE> {
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new().write(true).create(true).open(&path)?;

        // TODO: ler o arquivo e iniciar a página corretamente
        Ok(Self {
            current_page: Page::new(),
            reader: File::open(&path)?,
            writer: file,
            data: PhantomData,
        })
    }

    pub fn insert(&mut self, row: T) -> Result<(), DbError> {
        self.current_page.insert(row);

        // TODO: fazer um único write
        self.writer.write_all(self.current_page.as_ref());
        self.writer
            .write_all(&vec![0; PAGE_SIZE - self.current_page.len()]);

        if self.current_page.available_rows() == 0 {
            self.current_page = Page::new();
        } else {
            self.writer.seek(io::SeekFrom::End(-(PAGE_SIZE as i64)));
        }

        Ok(())
    }

    fn pages(&mut self) -> impl Iterator<Item = Page<ROW_SIZE>> + '_ {
        let mut cursor = 0;
        iter::from_fn(move || {
            let offset = (cursor * PAGE_SIZE) as u64;

            if self.reader.seek(io::SeekFrom::Start(offset)).is_err() {
                return None;
            }

            let mut buf = vec![0; PAGE_SIZE];
            cursor += 1;
            match self.reader.read_exact(&mut buf) {
                Ok(()) => Some(Page::from_bytes(buf).unwrap()),
                Err(_) => None,
            }
        })
    }

    pub fn rows(&mut self) -> impl Iterator<Item = T> + '_ {
        self.pages().flat_map(|page| {
            page.rows()
                .filter_map(|row| bitcode::deserialize(row).ok())
                .collect::<Vec<_>>()
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_insert_into_page() {
        let mut page = Page::<1024>::new();
        assert_eq!(4, page.available_rows());
        page.insert(String::from("Rinha"));
        assert_eq!(3, page.available_rows());
        page.insert(String::from("de"));
        assert_eq!(2, page.available_rows());
        page.insert(2024 as u64);
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
    fn test_insert_into_db() {
        let tmp = tempdir().unwrap();
        let mut db = Db::<(i64, String)>::from_path(tmp.path().join("test.espora")).unwrap();

        db.insert((50, String::from("Primeira")));
        db.insert((-20, String::from("Segunda")));

        let mut rows = db.rows();
        assert_eq!((50, String::from("Primeira")), rows.next().unwrap());
        assert_eq!((-20, String::from("Segunda")), rows.next().unwrap());
        assert!(rows.next().is_none());
    }
}
