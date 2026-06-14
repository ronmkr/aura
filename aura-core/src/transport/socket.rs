//! Async uTP Socket and Listener implementations over Tokio UDP.

use super::ledbat::LedbatController;
use super::packet::{PacketHeader, PacketType};
use crate::Result;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Instant;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// State of the uTP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    SynSent,
    Connected,
    Closed,
}

/// The uTP Socket implementing AsyncRead and AsyncWrite.
pub struct UtpSocket {
    udp: Arc<UdpSocket>,
    inner: Arc<Mutex<SocketStateInner>>,
}

#[allow(dead_code)]
struct SocketStateInner {
    peer_addr: SocketAddr,
    state: SocketState,
    conn_id_send: u16,
    conn_id_recv: u16,
    seq_nr: u16,
    ack_nr: u16,
    ledbat: LedbatController,
    read_buf: VecDeque<u8>,
    read_waker: Option<Waker>,
    write_buf: VecDeque<u8>,
    write_waker: Option<Waker>,
    last_recv_time: Instant,
}

impl UtpSocket {
    pub async fn connect(peer_addr: SocketAddr) -> Result<Self> {
        let local_addr: SocketAddr = if peer_addr.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let udp = Arc::new(UdpSocket::bind(local_addr).await?);
        udp.connect(peer_addr).await?;

        let conn_id_recv: u16 = rand::random();
        let conn_id_send = conn_id_recv.wrapping_add(1);

        let state = Arc::new(Mutex::new(SocketStateInner {
            peer_addr,
            state: SocketState::SynSent,
            conn_id_send,
            conn_id_recv,
            seq_nr: 1,
            ack_nr: 0,
            ledbat: LedbatController::new(),
            read_buf: VecDeque::new(),
            read_waker: None,
            write_buf: VecDeque::new(),
            write_waker: None,
            last_recv_time: Instant::now(),
        }));

        let socket = Self { udp, inner: state };

        // Start background receive loop
        let udp_clone = Arc::clone(&socket.udp);
        let inner_clone = Arc::clone(&socket.inner);
        tokio::spawn(async move {
            if let Err(e) = Self::recv_loop(udp_clone, inner_clone).await {
                debug!("uTP background recv loop error: {:?}", e);
            }
        });

        // Send SYN packet
        socket.send_syn().await?;

        // Wait for connection to transition to Connected
        let start = Instant::now();
        loop {
            let curr_state = { socket.inner.lock().await.state };
            if curr_state == SocketState::Connected {
                break;
            }
            if start.elapsed().as_secs() > 10 {
                return Err(crate::Error::Protocol(
                    "uTP connection handshake timeout".to_string(),
                ));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        Ok(socket)
    }

    async fn send_syn(&self) -> Result<()> {
        let inner = self.inner.lock().await;
        let mut buf = [0u8; PacketHeader::LEN];
        let header = PacketHeader {
            packet_type: PacketType::Syn,
            version: 1,
            extension: 0,
            connection_id: inner.conn_id_recv,
            timestamp_us: 0,
            timestamp_difference_us: 0,
            wnd_size: 1048576,
            seq_nr: inner.seq_nr,
            ack_nr: 0,
        };
        header.serialize(&mut buf);
        self.udp.send(&buf).await?;
        Ok(())
    }

    async fn recv_loop(udp: Arc<UdpSocket>, inner: Arc<Mutex<SocketStateInner>>) -> Result<()> {
        let mut buf = [0u8; 2048];
        loop {
            // Check if socket is closed
            {
                let guard = inner.lock().await;
                if guard.state == SocketState::Closed {
                    break;
                }
            }

            let len = match udp.recv(&mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    error!("uTP UDP recv error: {:?}", e);
                    break;
                }
            };

            if len < PacketHeader::LEN {
                continue;
            }

            let header = match PacketHeader::deserialize(&buf[..PacketHeader::LEN]) {
                Ok(h) => h,
                Err(_) => continue,
            };

            let mut guard = inner.lock().await;

            // Handle incoming packets based on type
            match header.packet_type {
                PacketType::Syn => {
                    // SYN is handled by listener, but if received on active connection, ignore or reply reset
                }
                PacketType::State => {
                    // ACK packet
                    if guard.state == SocketState::SynSent && header.ack_nr == guard.seq_nr {
                        guard.state = SocketState::Connected;
                        guard.ack_nr = header.seq_nr;
                        info!("uTP connection established with peer");
                    }

                    // Update LEDBAT base delay using timestamp difference
                    let now = Instant::now();
                    guard
                        .ledbat
                        .on_ack(header.timestamp_difference_us as u64, 0, now);
                }
                PacketType::Data => {
                    // Data packet
                    if guard.state == SocketState::Connected {
                        let payload = &buf[PacketHeader::LEN..len];
                        guard.read_buf.extend(payload);
                        guard.ack_nr = header.seq_nr;

                        // Wake up any pending readers
                        if let Some(waker) = guard.read_waker.take() {
                            waker.wake();
                        }

                        // Send ACK (STATE) packet back
                        let mut ack_buf = [0u8; PacketHeader::LEN];
                        let ack_header = PacketHeader {
                            packet_type: PacketType::State,
                            version: 1,
                            extension: 0,
                            connection_id: guard.conn_id_send,
                            timestamp_us: 0,
                            timestamp_difference_us: 0,
                            wnd_size: 1048576,
                            seq_nr: guard.seq_nr,
                            ack_nr: guard.ack_nr,
                        };
                        ack_header.serialize(&mut ack_buf);
                        let _ = udp.send(&ack_buf).await;
                    }
                }
                PacketType::Fin => {
                    guard.state = SocketState::Closed;
                    if let Some(waker) = guard.read_waker.take() {
                        waker.wake();
                    }
                }
                PacketType::Reset => {
                    guard.state = SocketState::Closed;
                    if let Some(waker) = guard.read_waker.take() {
                        waker.wake();
                    }
                }
            }
        }
        Ok(())
    }
}

// Implement AsyncRead and AsyncWrite traits for UtpSocket so it matches TcpStream
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
        let mut data_buf = vec![0u8; PacketHeader::LEN + buf.len()];
        let header = PacketHeader {
            packet_type: PacketType::Data,
            version: 1,
            extension: 0,
            connection_id: inner.conn_id_send,
            timestamp_us: 0,
            timestamp_difference_us: 0,
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

        tokio::spawn(async move {
            let mut fin_buf = [0u8; PacketHeader::LEN];
            let header = PacketHeader {
                packet_type: PacketType::Fin,
                version: 1,
                extension: 0,
                connection_id: conn_id,
                timestamp_us: 0,
                timestamp_difference_us: 0,
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
