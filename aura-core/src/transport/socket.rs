//! Async uTP Socket and Listener implementations over Tokio UDP.

use super::ledbat::LedbatController;
use super::packet::{PacketHeader, PacketType};
use crate::Result;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::Waker;
use std::time::Instant;
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
    pub(crate) udp: Arc<UdpSocket>,
    pub(crate) inner: Arc<Mutex<SocketStateInner>>,
}

#[allow(dead_code)]
pub(crate) struct SocketStateInner {
    pub(crate) peer_addr: SocketAddr,
    pub(crate) state: SocketState,
    pub(crate) conn_id_send: u16,
    pub(crate) conn_id_recv: u16,
    pub(crate) seq_nr: u16,
    pub(crate) ack_nr: u16,
    pub(crate) last_acked_seq: u16,
    pub(crate) ledbat: LedbatController,
    pub(crate) read_buf: VecDeque<u8>,
    pub(crate) read_waker: Option<Waker>,
    pub(crate) write_buf: VecDeque<u8>,
    pub(crate) write_waker: Option<Waker>,
    pub(crate) last_recv_time: Instant,
    pub(crate) last_received_timestamp_us: u32,
    pub(crate) last_measured_delay_us: u32,
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
            last_acked_seq: 0,
            ledbat: LedbatController::new(),
            read_buf: VecDeque::new(),
            read_waker: None,
            write_buf: VecDeque::new(),
            write_waker: None,
            last_recv_time: Instant::now(),
            last_received_timestamp_us: 0,
            last_measured_delay_us: 0,
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
            timestamp_us: get_microsecond_timestamp(),
            timestamp_difference_us: inner.last_measured_delay_us,
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

            let now_us = get_microsecond_timestamp();
            let mut guard = inner.lock().await;

            guard.last_received_timestamp_us = header.timestamp_us;
            guard.last_measured_delay_us = now_us.wrapping_sub(header.timestamp_us);
            guard.last_recv_time = Instant::now();

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
                        guard.last_acked_seq = header.seq_nr;
                        info!("uTP connection established with peer");
                    }

                    // Update LEDBAT base delay using timestamp difference
                    let now = Instant::now();
                    let acked_diff = header.ack_nr.wrapping_sub(guard.last_acked_seq);
                    if acked_diff > 0 && acked_diff < 32768 {
                        let bytes_newly_acked = (acked_diff as u32) * 1400; // Estimate 1400 bytes per packet
                        guard.last_acked_seq = header.ack_nr;
                        guard.ledbat.on_ack(
                            header.timestamp_difference_us as u64,
                            bytes_newly_acked,
                            now,
                        );
                    } else {
                        guard
                            .ledbat
                            .on_ack(header.timestamp_difference_us as u64, 0, now);
                    }
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
                            timestamp_us: get_microsecond_timestamp(),
                            timestamp_difference_us: guard.last_measured_delay_us,
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

/// Helper function to get the current system time in microseconds modulo 2^32.
pub(crate) fn get_microsecond_timestamp() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_micros() as u32
}
