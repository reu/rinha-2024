use std::{io, path::Path};

use serde::{de::DeserializeOwned, Serialize};

use crate::Db;

#[derive(Debug)]
pub struct Builder {
    sync_write: bool,
}

impl Default for Builder {
    fn default() -> Self {
        Builder { sync_write: true }
    }
}

impl Builder {
    pub fn sync_write(self, sync_write: bool) -> Self {
        Self { sync_write }
    }

    pub fn build<T: Serialize + DeserializeOwned, const ROW_SIZE: usize>(
        self,
        path: impl AsRef<Path>,
    ) -> io::Result<Db<T, ROW_SIZE>> {
        let mut db = Db::from_path(path)?;
        db.sync_write = self.sync_write;
        Ok(db)
    }

    #[cfg(feature = "tokio")]
    pub async fn build_tokio<T: Serialize + DeserializeOwned, const ROW_SIZE: usize>(
        self,
        path: impl AsRef<Path>,
    ) -> io::Result<crate::tokio::Db<T, ROW_SIZE>> {
        let mut db = crate::tokio::Db::from_path(path).await?;
        db.sync_write = self.sync_write;
        Ok(db)
    }
}
