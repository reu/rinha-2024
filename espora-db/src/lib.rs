use std::{
    error, fmt,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    iter,
    marker::PhantomData,
    path::Path,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::{
    builder::Builder,
    page::{Page, PAGE_SIZE},
};

pub mod builder;
mod page;
#[cfg(feature = "tokio")]
pub mod tokio;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Serialization(Box<dyn error::Error + Send + Sync>),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Serialization(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<bitcode::Error> for Error {
    fn from(err: bitcode::Error) -> Self {
        Error::Serialization(Box::new(err))
    }
}

pub(crate) type DbResult<T> = Result<T, Error>;

pub struct Db<T, const ROW_SIZE: usize> {
    current_page: Page<ROW_SIZE>,
    reader: File,
    writer: File,
    pub(crate) sync_write: bool,
    data: PhantomData<T>,
}

impl<const ROW_SIZE: usize, T: Serialize + DeserializeOwned> Db<T, ROW_SIZE> {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = OpenOptions::new().write(true).create(true).open(&path)?;
        file.seek(io::SeekFrom::End(0))?;

        // TODO: ler o arquivo e iniciar a página corretamente
        Ok(Self {
            current_page: Page::new(),
            reader: File::open(&path)?,
            writer: file,
            sync_write: true,
            data: PhantomData,
        })
    }

    pub fn insert(&mut self, row: T) -> DbResult<()> {
        self.current_page.insert(row)?;

        self.writer.write_all(
            &[
                self.current_page.as_ref(),
                &vec![0; PAGE_SIZE - self.current_page.len()],
            ]
            .concat(),
        )?;

        if self.sync_write {
            self.writer.sync_data()?;
        }

        if self.current_page.available_rows() == 0 {
            self.current_page = Page::new();
        } else {
            self.writer.seek(io::SeekFrom::End(-(PAGE_SIZE as i64)))?;
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

    fn pages_reverse(&mut self) -> impl Iterator<Item = Page<ROW_SIZE>> + '_ {
        let mut cursor = 1;
        iter::from_fn(move || {
            let offset = (cursor * PAGE_SIZE) as i64;

            if self.reader.seek(io::SeekFrom::End(-offset)).is_err() {
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

    pub fn rows(&mut self) -> impl Iterator<Item = DbResult<T>> + '_ {
        self.pages().flat_map(|page| {
            page.rows()
                .map(|row| bitcode::deserialize(row).map_err(|err| err.into()))
                .collect::<Vec<_>>()
        })
    }

    pub fn rows_reverse(&mut self) -> impl Iterator<Item = DbResult<T>> + '_ {
        self.pages_reverse().flat_map(|page| {
            page.rows()
                .map(|row| bitcode::deserialize(row).map_err(|err| err.into()))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_db_rows() {
        let tmp = tempdir().unwrap();
        let mut db = Db::<i64, 2048>::from_path(tmp.path().join("test.espora")).unwrap();

        db.insert(1).unwrap();
        db.insert(2).unwrap();
        db.insert(3).unwrap();
        db.insert(4).unwrap();
        db.insert(5).unwrap();

        let rows = db.rows().collect::<DbResult<Vec<_>>>().unwrap();
        assert_eq!(vec![1, 2, 3, 4, 5], rows);
    }

    #[test]
    fn test_db_rows_reverse() {
        let tmp = tempdir().unwrap();
        let mut db = Db::<i64, 2048>::from_path(tmp.path().join("test.espora")).unwrap();

        db.insert(1).unwrap();
        db.insert(2).unwrap();
        db.insert(3).unwrap();
        db.insert(4).unwrap();
        db.insert(5).unwrap();

        let rows = db.rows_reverse().collect::<DbResult<Vec<_>>>().unwrap();
        assert_eq!(vec![5, 4, 3, 2, 1], rows);
    }
}
