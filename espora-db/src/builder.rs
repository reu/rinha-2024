use std::{io, path::Path, time::Duration};

use serde::{de::DeserializeOwned, Serialize};

use crate::Db;

#[derive(Debug)]
pub struct Builder {
    sync_writes: Option<Duration>,
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            sync_writes: Some(Duration::from_secs(0)),
        }
    }
}

impl Builder {
    pub fn sync_writes(self, sync_writes: bool) -> Self {
        Self {
            sync_writes: if sync_writes {
                Some(Duration::from_secs(0))
            } else {
                None
            },
        }
    }

    pub fn sync_write_interval(self, interval: Duration) -> Self {
        Self {
            sync_writes: Some(interval),
        }
    }

    pub fn build<T: Serialize + DeserializeOwned, const ROW_SIZE: usize>(
        self,
        path: impl AsRef<Path>,
    ) -> io::Result<Db<T, ROW_SIZE>> {
        let mut db = Db::from_path(path)?;
        db.sync_writes = self.sync_writes;
        Ok(db)
    }

    #[cfg(feature = "tokio")]
    pub async fn build_tokio<T: Serialize + DeserializeOwned, const ROW_SIZE: usize>(
        self,
        path: impl AsRef<Path>,
    ) -> io::Result<crate::tokio::Db<T, ROW_SIZE>> {
        let mut db = crate::tokio::Db::from_path(path).await?;
        db.sync_writes = self.sync_writes;
        Ok(db)
    }
}
