pub type SolId = usize;

pub type SolAddress = std::net::SocketAddr;

pub enum SolError {
    CouldNotSpawn,
}
