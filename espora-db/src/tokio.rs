use std::{marker::PhantomData, path::Path};

use async_stream::stream;
use futures::{stream, Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::{
    builder::Builder,
    page::{Page, PAGE_SIZE},
    DbResult,
};

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

    pub async fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .await?;

        let current_page = if file
            .seek(io::SeekFrom::End(-(PAGE_SIZE as i64)))
            .await
            .is_ok()
        {
            let mut buf = vec![0; PAGE_SIZE];
            file.read_exact(&mut buf).await?;
            file.seek(io::SeekFrom::End(-(PAGE_SIZE as i64))).await?;
            Page::from_bytes(buf)
        } else {
            file.seek(io::SeekFrom::End(0)).await?;
            Page::new()
        };

        Ok(Self {
            current_page,
            reader: File::open(&path).await?,
            writer: file,
            sync_write: true,
            data: PhantomData,
        })
    }

    pub async fn insert(&mut self, row: T) -> DbResult<()> {
        self.current_page.insert(row)?;

        self.writer
            .write_all(
                &[
                    self.current_page.as_ref(),
                    &vec![0; PAGE_SIZE - self.current_page.len()],
                ]
                .concat(),
            )
            .await?;

        if self.sync_write {
            self.writer.sync_data().await?;
        }

        if self.current_page.available_rows() == 0 {
            self.current_page = Page::new();
        } else {
            self.writer
                .seek(io::SeekFrom::End(-(PAGE_SIZE as i64)))
                .await?;
        }

        Ok(())
    }

    fn pages(&mut self) -> impl Stream<Item = Page<ROW_SIZE>> + '_ {
        let mut cursor = 0;
        stream! {
            loop {
                let offset = (cursor * PAGE_SIZE) as u64;

                if self.reader.seek(io::SeekFrom::Start(offset)).await.is_err() {
                    break;
                }

                let mut buf = vec![0; PAGE_SIZE];
                cursor += 1;
                match self.reader.read_exact(&mut buf).await {
                    Ok(n) if n > 0 => yield Page::<ROW_SIZE>::from_bytes(buf),
                    _ => break,
                }
            }
        }
    }

    fn pages_reverse(&mut self) -> impl Stream<Item = Page<ROW_SIZE>> + '_ {
        let mut cursor = 1;
        stream! {
            loop {
                let offset = (cursor * PAGE_SIZE) as i64;

                if self.reader.seek(io::SeekFrom::End(-offset)).await.is_err() {
                    break;
                }

                let mut buf = vec![0; PAGE_SIZE];
                cursor += 1;
                match self.reader.read_exact(&mut buf).await {
                    Ok(n) if n > 0 => yield Page::<ROW_SIZE>::from_bytes(buf),
                    _ => break,
                }
            }
        }
    }

    pub fn rows(&mut self) -> impl Stream<Item = DbResult<T>> + '_ {
        self.pages().flat_map(|page| {
            stream::iter(
                page.rows()
                    .map(|row| bitcode::deserialize(row).map_err(|err| err.into()))
                    .collect::<Vec<_>>(),
            )
        })
    }

    pub fn rows_reverse(&mut self) -> impl Stream<Item = DbResult<T>> + '_ {
        self.pages_reverse().flat_map(|page| {
            stream::iter(
                page.rows()
                    .map(|row| bitcode::deserialize(row).map_err(|err| err.into()))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_db_rows() {
        let tmp = tempdir().unwrap();
        let mut db = Db::<i64, 2048>::from_path(tmp.path().join("test.espora"))
            .await
            .unwrap();

        db.insert(1).await.unwrap();
        db.insert(2).await.unwrap();
        db.insert(3).await.unwrap();
        db.insert(4).await.unwrap();
        db.insert(5).await.unwrap();

        let rows = db.rows().try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(vec![1, 2, 3, 4, 5], rows);
    }

    #[tokio::test]
    async fn test_db_rows_reverse() {
        let tmp = tempdir().unwrap();
        let mut db = Db::<i64, 2048>::from_path(tmp.path().join("test.espora"))
            .await
            .unwrap();

        db.insert(1).await.unwrap();
        db.insert(2).await.unwrap();
        db.insert(3).await.unwrap();
        db.insert(4).await.unwrap();
        db.insert(5).await.unwrap();

        let rows = db.rows_reverse().try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(vec![5, 4, 3, 2, 1], rows);
    }
}
