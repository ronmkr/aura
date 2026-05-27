#[derive(Debug, Clone)]
pub enum WorkerCommand {
    CancelPiece(usize),
    RequestPiece(usize),
    Choke(String, u16),
    Unchoke(String, u16),
}
