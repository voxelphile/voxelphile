use crate::{
    input::{Input, Inputs},
    world::{
        block::Block,
        entity::{Change, Target},
        raycast, ChunkPosition, ClientWorld, LocalPosition, WorldPosition,
    },
};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    mem,
    net::*,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const SERVER_PORT: u16 = 41235;
pub const CLIENT_PORT: u16 = 41234;
pub const TIMEOUT: f32 = 5.0;
pub const BUFFER_SIZE: usize = 16777216;

#[derive(Debug)]
pub enum Error {
    InvalidAddress,
    FailedToBind,
    FailedToConnect,
    FailedToSerialize,
    FailedToSend,
    FailedToRecv,
    FailedToParse,
    FailedToDeserialize,
    ClientDoesNotExist,
    Timeout,
}

#[derive(Debug)]
pub struct ClientTag;
#[derive(Debug)]
pub struct ServerTag;

#[derive(Serialize, Deserialize)]
struct Header {
    send_time_epoch_ms: u128,
}

pub struct Info {
    pub send_time_epoch_ms: u128,
    pub recv_time_epoch_ms: u128,
}

impl Info {
    fn new(packet: &Packet) -> Self {
        Self {
            send_time_epoch_ms: packet.header.send_time_epoch_ms,
            recv_time_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH.into())
                .unwrap()
                .as_millis(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Packet {
    header: Header,
    message: Message,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChunkActivated {
    pub position: ChunkPosition,
    pub lod: usize,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChunkUpdated {
    pub position: ChunkPosition,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ChunkMessage {
    Activated(ChunkActivated),
    Updated(ChunkUpdated),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Correct {
    Target(Target),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Message {
    Handshake,
    Chunk(ChunkMessage),
    Inputs(Inputs),
    Change(Change),
    Correct(Correct),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClientId(usize);

pub struct Client {
    socket: UdpSocket,
    messages: Vec<(Message, Info)>,
}

pub struct Connection {
    client: Option<Client>,
    start: Instant,
}

#[profiling::all_functions]
impl Connection {
    pub fn check_connection(mut self) -> Result<Result<Client, Error>, Self> {
        let recv_result = self.client.as_mut().unwrap().recv();
        if let Err(e) = recv_result {
            return Ok(Err(e));
        }
        let recv_result = self
            .client
            .as_mut()
            .unwrap()
            .recv()
            .map_err(|_| Error::FailedToRecv);
        if let Err(e) = recv_result {
            return Ok(Err(e));
        }
        let mut messages = vec![];
        messages.extend(
            self.client
                .as_mut()
                .unwrap()
                .get(|m| matches!(m, (Message::Handshake, _))),
        );
        if messages.len() >= 1 {
            let message = messages.drain(0..1).next().unwrap();
            use Message::*;
            match message.0 {
                Handshake => {
                    return Ok(Ok(self.client.take().unwrap()));
                }
                _ => {}
            }
        }
        if Instant::now().duration_since(self.start).as_secs_f32() >= TIMEOUT {
            return Ok(Result::<Client, Error>::Err(Error::Timeout));
        }
        thread::sleep(Duration::from_millis(100));
        Err(self)
    }
}

#[profiling::all_functions]
impl Client {
    pub fn connect<A: ToSocketAddrs>(addresses: A) -> Result<Connection, Error> {
        let server_address = addresses
            .to_socket_addrs()
            .map_err(|_| Error::InvalidAddress)?
            .next()
            .ok_or(Error::InvalidAddress)?;
        let socket = loop {
            use rand::Rng;
            let port = CLIENT_PORT + thread_rng().gen::<u8>() as u16;
            match UdpSocket::bind(&format!("127.0.0.1:{}", port)) {
                Ok(socket) => break socket,
                _ => {}
            }
        };

        socket
            .connect(server_address)
            .map_err(|_| Error::FailedToConnect)?;

        socket.set_nonblocking(true);

        let mut client = Client {
            socket,
            messages: vec![],
        };

        client
            .send(Message::Handshake)
            .map_err(|_| Error::FailedToSend)?;

        Ok(Connection {
            client: Some(client),
            start: Instant::now(),
        })
    }

    pub fn send(&self, message: Message) -> Result<(), Error> {
        let packet = Packet {
            header: Header {
                send_time_epoch_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH.into())
                    .unwrap()
                    .as_millis(),
            },
            message,
        };

        let json = serde_json::to_string(&packet).map_err(|_| Error::FailedToSerialize)?;

        let bytes = json.into_bytes();

        self.socket.send(&bytes).map_err(|_| Error::FailedToSend)?;

        Ok(())
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        let mut buffer = vec![0u8; BUFFER_SIZE];
        while let Ok(len) = self.socket.recv(&mut buffer) {
            let bytes = buffer.iter().take(len).copied().collect::<Vec<u8>>();
            let json = String::from_utf8(bytes).map_err(|_| Error::FailedToParse)?;
            let packet =
                serde_json::from_str::<Packet>(&json).map_err(|_| Error::FailedToDeserialize)?;
            self.messages
                .push((packet.message.clone(), Info::new(&packet)));
        }
        Ok(())
    }

    pub fn get<F: Fn(&(Message, Info)) -> bool>(&mut self, predicate: F) -> Vec<(Message, Info)> {
        let mut messages = mem::take(&mut self.messages);
        let (target, rest) = messages.into_iter().partition(predicate);
        self.messages = rest;
        target
    }
}

pub struct Server {
    conn_cursor: ClientId,
    conns: HashMap<SocketAddr, ClientId>,
    mapping: HashMap<ClientId, SocketAddr>,
    start: HashMap<ClientId, Instant>,
    active: HashSet<ClientId>,
    heartbeat: HashMap<ClientId, Instant>,
    messages: HashMap<ClientId, Vec<(Message, Info)>>,
    socket: UdpSocket,
}

#[profiling::all_functions]
impl Server {
    pub fn bind() -> Result<Self, Error> {
        let socket = UdpSocket::bind(&format!("0.0.0.0:{}", SERVER_PORT))
            .map_err(|_| Error::FailedToBind)?;

        socket.set_nonblocking(true);

        Ok(Server {
            socket,
            messages: Default::default(),
            mapping: Default::default(),
            conns: Default::default(),
            start: Default::default(),
            active: Default::default(),
            heartbeat: Default::default(),
            conn_cursor: ClientId(0),
        })
    }

    pub fn prune(&mut self) -> Result<HashSet<ClientId>, Error> {
        let mut remove = HashSet::new();
        for (&addr, &client) in &self.conns {
            let last = self.heartbeat[&client];
            let start = self.start[&client];
            if Instant::now().duration_since(last).as_secs_f32() > TIMEOUT {
                remove.insert((addr, client));
            }
            if !self.active.contains(&client)
                && Instant::now().duration_since(start).as_secs_f32() > TIMEOUT
            {
                remove.insert((addr, client));
            }
        }
        for (addr, client) in remove.clone() {
            self.conns.remove(&addr);
            self.active.remove(&client);
            self.start.remove(&client);
            self.heartbeat.remove(&client);
            self.messages.remove(&client);
            self.mapping.remove(&client);
        }
        let remove = remove.into_iter().map(|(_, c)| c).collect::<HashSet<_>>();
        Ok(remove)
    }

    pub fn accept(&mut self) -> Result<HashSet<ClientId>, Error> {
        let potential = self
            .conns
            .iter()
            .filter(|(_, c)| !self.active.contains(c))
            .map(|(_, &c)| c)
            .collect::<HashSet<_>>();
        let mut activated = HashSet::new();
        for client in potential {
            let messages = self.get(client, |m| matches!(m, (Message::Handshake, _)));

            if messages.len() >= 1 {
                self.send(client, Message::Handshake)
                    .map_err(|_| Error::FailedToSend)?;
                activated.insert(client);
            }
        }
        self.active.extend(activated.clone());
        Ok(activated)
    }

    pub fn send_to_all(&self, message: Message) -> Result<(), Error> {
        for &client in &self.active {
            self.send(client, message.clone())?;
        }
        Ok(())
    }

    pub fn send(&self, client: ClientId, message: Message) -> Result<(), Error> {
        let Some(&addr) = self.mapping.get(&client) else {
            Err(Error::ClientDoesNotExist)?
        };
        let packet = Packet {
            header: Header {
                send_time_epoch_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH.into())
                    .unwrap()
                    .as_millis(),
            },
            message,
        };

        let json = serde_json::to_string(&packet).map_err(|_| Error::FailedToSerialize)?;
        let bytes = json.into_bytes();
        self.socket
            .send_to(&bytes, addr)
            .map_err(|e| Error::FailedToSend)?;

        Ok(())
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        let mut buffer = vec![0u8; BUFFER_SIZE];
        while let Ok((len, addr)) = self.socket.recv_from(&mut buffer) {
            let bytes = buffer.iter().take(len).copied().collect::<Vec<u8>>();
            let json = String::from_utf8(bytes).map_err(|_| Error::FailedToParse)?;
            let packet =
                serde_json::from_str::<Packet>(&json).map_err(|_| Error::FailedToDeserialize)?;
            let client = *self.conns.entry(addr).or_insert_with(|| {
                let conn = self.conn_cursor;
                self.conn_cursor.0 += 1;
                conn
            });
            self.mapping.entry(client).or_insert(addr);
            self.messages
                .entry(client)
                .or_default()
                .push((packet.message.clone(), Info::new(&packet)));
            self.heartbeat
                .entry(client)
                .and_modify(|last| *last = Instant::now())
                .or_insert_with(|| Instant::now());
        }
        Ok(())
    }

    pub fn get<F: Fn(&(Message, Info)) -> bool>(
        &mut self,
        client: ClientId,
        predicate: F,
    ) -> Vec<(Message, Info)> {
        let Some(client_messages) = self.messages.get_mut(&client) else {
            return vec![];
        };
        let mut messages = mem::take(client_messages);
        let (target, rest) = messages.into_iter().partition(predicate);
        *client_messages = rest;
        target
    }
}
