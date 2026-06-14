use super::packet::{PacketHeader, PacketType};
use super::socket::{get_microsecond_timestamp, SocketState, UtpSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

impl AsyncRead for UtpSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut inner = match self.inner.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        if inner.state == SocketState::Closed && inner.read_buf.is_empty() {
            return Poll::Ready(Ok(()));
        }

        if inner.read_buf.is_empty() {
            inner.read_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let to_read = std::cmp::min(buf.remaining(), inner.read_buf.len());
        let drained: Vec<u8> = inner.read_buf.drain(0..to_read).collect();
        buf.put_slice(&drained);

        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for UtpSocket {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut inner = match self.inner.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        if inner.state == SocketState::Closed {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "uTP socket is closed",
            )));
        }

        // For simplicity in the starting implementation, we immediately send the packet over UDP.
        // In a full implementation, we would queue it and send it according to the LEDBAT cwnd.
        let timestamp_us = get_microsecond_timestamp();
        let timestamp_difference_us = inner.last_measured_delay_us;
        let mut data_buf = vec![0u8; PacketHeader::LEN + buf.len()];
        let header = PacketHeader {
            packet_type: PacketType::Data,
            version: 1,
            extension: 0,
            connection_id: inner.conn_id_send,
            timestamp_us,
            timestamp_difference_us,
            wnd_size: 1048576,
            seq_nr: inner.seq_nr,
            ack_nr: inner.ack_nr,
        };
        header.serialize(&mut data_buf[..PacketHeader::LEN]);
        data_buf[PacketHeader::LEN..].copy_from_slice(buf);

        let udp = Arc::clone(&self.udp);
        let len = buf.len();

        // Spawn sending as a task to prevent blocking the poll
        tokio::spawn(async move {
            let _ = udp.send(&data_buf).await;
        });

        inner.seq_nr = inner.seq_nr.wrapping_add(1);

        Poll::Ready(Ok(len))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut inner = match self.inner.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        inner.state = SocketState::Closed;

        let udp = Arc::clone(&self.udp);
        let conn_id = inner.conn_id_send;
        let seq = inner.seq_nr;
        let ack = inner.ack_nr;
        let timestamp_us = get_microsecond_timestamp();
        let timestamp_difference_us = inner.last_measured_delay_us;

        tokio::spawn(async move {
            let mut fin_buf = [0u8; PacketHeader::LEN];
            let header = PacketHeader {
                packet_type: PacketType::Fin,
                version: 1,
                extension: 0,
                connection_id: conn_id,
                timestamp_us,
                timestamp_difference_us,
                wnd_size: 1048576,
                seq_nr: seq,
                ack_nr: ack,
            };
            header.serialize(&mut fin_buf);
            let _ = udp.send(&fin_buf).await;
        });

        Poll::Ready(Ok(()))
    }
}
