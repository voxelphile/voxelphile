use crate::world::{block::Block, World, WorldPosition};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    mem,
    net::*,
    thread,
    time::{Duration, Instant},
};

pub const PORT: usize = 41234;
pub const TIMEOUT: f32 = 5.0;

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

#[derive(Serialize, Deserialize)]
struct Header {}

#[derive(Serialize, Deserialize)]
struct Packet {
    header: Header,
    message: Message,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Message {
    Handshake,
    Ready,
    Blocks { set: Vec<(WorldPosition, Block)> },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(usize);

pub struct Client {
    socket: UdpSocket,
    messages: Vec<Message>,
}

impl Client {
    pub fn connect<A: ToSocketAddrs>(addresses: A) -> Result<Self, Error> {
        let server_address = addresses
            .to_socket_addrs()
            .map_err(|_| Error::InvalidAddress)?
            .next()
            .ok_or(Error::InvalidAddress)?;
        let socket =
            UdpSocket::bind(&format!("127.0.0.1:{}", PORT)).map_err(|_| Error::FailedToBind)?;

        socket
            .connect(server_address)
            .map_err(|_| Error::FailedToConnect)?;

        let mut client = Client {
            socket,
            messages: vec![],
        };

        client
            .send(Message::Handshake)
            .map_err(|_| Error::FailedToSend)?;

        let mut messages = vec![];
        let start = Instant::now();
        loop {
            client.recv().map_err(|_| Error::FailedToRecv)?;
            messages.extend(client.get(|m| matches!(m, Message::Handshake)));
            if messages.len() >= 1 {
                let message = messages.drain(0..1).next().unwrap();
                use Message::*;
                match message {
                    Handshake => {
                        return Ok(client);
                    }
                    _ => {}
                }
            }
            if Instant::now().duration_since(start).as_secs_f32() >= TIMEOUT {
                Err(Error::Timeout)?
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn send(&self, message: Message) -> Result<(), Error> {
        let packet = Packet {
            header: Header {},
            message,
        };

        let json = serde_json::to_string(&packet).map_err(|_| Error::FailedToSerialize)?;

        let bytes = json.into_bytes();

        self.socket.send(&bytes).map_err(|_| Error::FailedToSend)?;

        Ok(())
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        let mut buffer = vec![0u8; 32768];
        while let Ok(len) = self.socket.recv(&mut buffer) {
            let bytes = buffer.iter().take(len).copied().collect::<Vec<u8>>();
            let json = String::from_utf8(bytes).map_err(|_| Error::FailedToParse)?;
            let packet =
                serde_json::from_str::<Packet>(&json).map_err(|_| Error::FailedToDeserialize)?;
            self.messages.push(packet.message);
        }
        Ok(())
    }

    pub fn get<F: Fn(&Message) -> bool>(&mut self, predicate: F) -> Vec<Message> {
        let mut messages = vec![];
        mem::take(&mut messages);
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
    messages: HashMap<ClientId, Vec<Message>>,
    socket: UdpSocket,
}

impl Server {
    pub fn bind() -> Result<Self, Error> {
        let socket =
            UdpSocket::bind(&format!("0.0.0.0:{}", PORT)).map_err(|_| Error::FailedToBind)?;

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

    pub fn prune(&mut self) -> Result<(), Error> {
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
        for (addr, client) in remove {
            self.conns.remove(&addr);
            self.active.remove(&client);
            self.start.remove(&client);
            self.heartbeat.remove(&client);
            self.messages.remove(&client);
            self.mapping.remove(&client);
        }
        Ok(())
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
            let messages = self.get(client, |m| matches!(m, Message::Handshake));

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
        for client in self.active {
            self.send(client, message.clone())?;
        }
        Ok(())
    }

    pub fn send(&self, client: ClientId, message: Message) -> Result<(), Error> {
        let Some(&addr) = self.mapping.get(&client) else {
            Err(Error::ClientDoesNotExist)?
        };
        let packet = Packet {
            header: Header {},
            message,
        };

        let json = serde_json::to_string(&packet).map_err(|_| Error::FailedToSerialize)?;

        let bytes = json.into_bytes();

        self.socket
            .send_to(&bytes, addr)
            .map_err(|_| Error::FailedToSend)?;

        Ok(())
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        let mut buffer = vec![0u8; 32768];
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
                .push(packet.message);
            self.heartbeat
                .entry(client)
                .and_modify(|last| *last = Instant::now())
                .or_insert_with(|| Instant::now());
        }
        Ok(())
    }

    pub fn get<F: Fn(&Message) -> bool>(&mut self, client: ClientId, predicate: F) -> Vec<Message> {
        let Some(client_messages) = self.messages.get_mut(&client) else {
            return vec![];
        };
        let mut messages = vec![];
        mem::take(client_messages);
        let (target, rest) = messages.into_iter().partition(predicate);
        *client_messages = rest;
        target
    }
}
