use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use rand::distributions::{Alphanumeric, DistString};
use rand::thread_rng;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot::Receiver;

use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::config::filesystem;
use crate::db_backend::DbBackend;

pub struct Filesystem {
    pub config: filesystem::Filesystem,
}

#[async_trait]
impl DbBackend for Filesystem {
    fn authenticated(&self) -> bool {
        true
    }

    async fn get_db_read(&self, user_info: &UserInfo) -> Result<Pin<Box<dyn AsyncRead + '_>>> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            Box::pin(File::open(
                path
            ).await?)
        )
    }

    async fn get_key_read(&self, user_info: &UserInfo) -> Option<Result<Pin<Box<dyn AsyncRead + '_>>>> {
        let mut path = None;
        if let Some(p) = &user_info.keyfile_location {
            path = Some(Path::new(p));
        } else if let Some(p) = &self.config.keyfile_location {
            path = Some(p.as_path())
        }

        // return key file only if the key file location was configured
        if let Some(loc) = path {
            return match File::open(loc).await {
                Ok(keyfile) => {
                    Some(Ok(Box::pin(keyfile)))
                }
                Err(err) => Some(Err(err.into())),
            };
        }

        None
    }

    async fn get_db_write(&mut self, user_info: &UserInfo) -> Result<(Pin<Box<dyn AsyncWrite + '_>>, Option<Receiver<Result<()>>>)> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            (
                Box::pin(AtomicFile::create(path).await?),
                None
            )
        )
    }

    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }
}

// File::open opens read-only, so writes used to fail with EBADF. Truncating the
// database in place would be just as bad: an interrupted write would leave a
// half written database and no copy of the previous one. Writes go to a
// temporary file next to the database instead and are renamed over it when the
// writer is shut down.
pub struct AtomicFile {
    tmp_path: PathBuf,
    target: PathBuf,
    state: State,
}

enum State {
    Writing(File),
    Finishing(Pin<Box<dyn Future<Output = io::Result<()>> + Send>>),
    Done,
}

impl AtomicFile {
    async fn create(target: &Path) -> Result<Self> {
        let file_name = target.file_name()
            .ok_or(anyhow!("database location has no file name: {}", target.display()))?;

        // a database is not world readable, and the mode of the file that is
        // replaced has to survive the replacement
        #[cfg(unix)]
        let mode = match tokio::fs::metadata(target).await {
            Ok(meta) => std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o7777,
            Err(_) => 0o600,
        };

        // a random name opened exclusively, so two writers of the same database
        // cannot end up sharing one temporary file
        for _ in 0..16 {
            let mut tmp_name = file_name.to_os_string();
            tmp_name.push(format!(".tmp{}", Alphanumeric.sample_string(&mut thread_rng(), 8)));
            let tmp_path = target.with_file_name(tmp_name);

            let file = match OpenOptions::new().write(true).create_new(true).open(&tmp_path).await {
                Ok(file) => file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err.into()),
            };

            #[cfg(unix)]
            tokio::fs::set_permissions(&tmp_path, std::os::unix::fs::PermissionsExt::from_mode(mode)).await?;

            return Ok(
                Self {
                    state: State::Writing(file),
                    tmp_path,
                    target: target.to_path_buf(),
                }
            );
        }

        bail!("failed to create a temporary file next to {}", target.display())
    }
}

impl Drop for AtomicFile {
    fn drop(&mut self) {
        // an abandoned write leaves the database alone, drop what was written
        if matches!(self.state, State::Writing(_)) {
            let _ = std::fs::remove_file(&self.tmp_path);
        }
    }
}

impl AsyncWrite for AtomicFile {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match &mut self.state {
            State::Writing(file) => Pin::new(file).poll_write(cx, buf),
            _ => Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.state {
            State::Writing(file) => Pin::new(file).poll_flush(cx),
            _ => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.state {
                State::Writing(_) => {
                    let State::Writing(mut file) = std::mem::replace(&mut self.state, State::Done) else {
                        unreachable!("state was just matched as writing")
                    };
                    let tmp_path = self.tmp_path.clone();
                    let target = self.target.clone();

                    self.state = State::Finishing(Box::pin(async move {
                        file.flush().await?;
                        // the rename is only worth anything once the contents
                        // reached the disk
                        file.sync_all().await?;
                        drop(file);

                        tokio::fs::rename(&tmp_path, &target).await
                    }));
                }
                State::Finishing(fut) => {
                    let result = std::task::ready!(fut.as_mut().poll(cx));
                    self.state = State::Done;

                    return Poll::Ready(result);
                }
                State::Done => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl Filesystem {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.filesystem.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    fn backend_for(path: &std::path::Path) -> Filesystem {
        let mut config = Config::default();
        config.filesystem.db_location = path.to_path_buf();

        Filesystem::new(&config)
    }

    #[tokio::test]
    async fn write_roundtrip() {
        let path = std::env::temp_dir().join("k4w-filesystem-write-test.kdbx");
        let _ = tokio::fs::remove_file(&path).await;

        let mut config = Config::default();
        config.filesystem.db_location = path.clone();
        let mut backend = Filesystem::new(&config);

        let data = b"some database content";
        {
            let (mut writer, rx) = backend.get_db_write(&UserInfo::default()).await.unwrap();
            writer.write_all(data).await.unwrap();
            writer.shutdown().await.unwrap();
            assert!(rx.is_none());
        }

        let mut reader = backend.get_db_read(&UserInfo::default()).await.unwrap();
        let mut read_back = vec![];
        reader.read_to_end(&mut read_back).await.unwrap();
        assert_eq!(read_back, data);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn the_database_is_replaced_only_once_the_write_finished() {
        let path = std::env::temp_dir().join("k4w-filesystem-atomic-test.kdbx");
        tokio::fs::write(&path, b"previous database").await.unwrap();
        let mut backend = backend_for(&path);

        let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
        writer.write_all(b"new database").await.unwrap();
        writer.flush().await.unwrap();

        // written but not shut down: the database still holds the old contents
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"previous database");

        writer.shutdown().await.unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"new database");

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn an_abandoned_write_leaves_the_database_and_no_leftovers() {
        let path = std::env::temp_dir().join("k4w-filesystem-abandoned-test.kdbx");
        tokio::fs::write(&path, b"previous database").await.unwrap();
        let mut backend = backend_for(&path);

        {
            let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
            writer.write_all(b"half a database").await.unwrap();
            writer.flush().await.unwrap();
        }

        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"previous database");

        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("k4w-filesystem-abandoned-test.kdbx.tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left behind: {:?}", leftovers);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_mode_of_the_replaced_database_is_kept() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join("k4w-filesystem-mode-test.kdbx");
        tokio::fs::write(&path, b"previous database").await.unwrap();
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await.unwrap();
        let mut backend = backend_for(&path);

        let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
        writer.write_all(b"new database").await.unwrap();
        writer.shutdown().await.unwrap();

        let mode = tokio::fs::metadata(&path).await.unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o600, "mode after replacement: {:o}", mode);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_new_database_is_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join("k4w-filesystem-new-mode-test.kdbx");
        let _ = tokio::fs::remove_file(&path).await;
        let mut backend = backend_for(&path);

        let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
        writer.write_all(b"new database").await.unwrap();
        writer.shutdown().await.unwrap();

        let mode = tokio::fs::metadata(&path).await.unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o600, "mode of a fresh database: {:o}", mode);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn concurrent_writers_do_not_share_a_temporary_file() {
        let path = std::env::temp_dir().join("k4w-filesystem-concurrent-test.kdbx");
        tokio::fs::write(&path, b"previous database").await.unwrap();

        let first = async {
            let mut backend = backend_for(&path);
            let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
            writer.write_all(&[b'a'; 4096]).await.unwrap();
            writer.shutdown().await.unwrap();
        };
        let second = async {
            let mut backend = backend_for(&path);
            let (mut writer, _) = backend.get_db_write(&UserInfo::default()).await.unwrap();
            writer.write_all(&[b'b'; 4096]).await.unwrap();
            writer.shutdown().await.unwrap();
        };
        tokio::join!(first, second);

        // whichever renamed last wins, but the result is one whole write
        let written = tokio::fs::read(&path).await.unwrap();
        assert!(
            written == vec![b'a'; 4096] || written == vec![b'b'; 4096],
            "database holds a mix of both writes ({} bytes)", written.len(),
        );

        let _ = tokio::fs::remove_file(&path).await;
    }
}
