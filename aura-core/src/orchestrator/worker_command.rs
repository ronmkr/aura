#[derive(Debug, Clone)]
pub enum WorkerCommand {
    CancelPiece(usize),
    RequestPiece(usize),
    EndgameFetch(usize),
    CheckWork,
    Choke(String, u16),
    Unchoke(String, u16),
    PexUpdate(std::collections::HashSet<std::net::SocketAddr>),
}
