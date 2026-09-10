use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result};

/// Read a 4-byte little-endian length prefixed JSON frame.
pub async fn read_frame<R: AsyncRead + Unpin, T: DeserializeOwned>(reader: &mut R) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;

    if len > 10 * 1024 * 1024 {
        return Err(Error::Parse("IPC frame exceeded maximum size (10MB)".into()));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    let msg = serde_json::from_slice(&buf)?;
    Ok(msg)
}

/// Write a 4-byte little-endian length prefixed JSON frame.
pub async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(writer: &mut W, msg: &T) -> Result<()> {
    let bytes = serde_json::to_vec(msg)?;
    let len = (bytes.len() as u32).to_le_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(windows)]
pub fn connect_client(pipe_name: &str) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new()
        .open(pipe_name)
        .map_err(Error::Io)
}

#[cfg(windows)]
pub fn create_server(pipe_name: &str, first_instance: bool) -> Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    tokio::net::windows::named_pipe::ServerOptions::new()
        .first_pipe_instance(first_instance)
        .create(pipe_name)
        .map_err(Error::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let msg = ("test_key".to_string(), 12345u32);
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).await.unwrap();

        let mut cursor = Cursor::new(buf);
        let recovered: (String, u32) = read_frame(&mut cursor).await.unwrap();
        assert_eq!(msg, recovered);
    }
}
