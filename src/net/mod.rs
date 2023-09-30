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
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    mem,
    net::*,
    thread,
    time::{self, Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const SERVER_PORT: u16 = 41235;
pub const CLIENT_PORT: u16 = 41234;
pub const ACK: f32 = 1.5;
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
    InvalidChecksum,
    Timeout,
}

#[derive(Debug)]
pub struct ClientTag;
#[derive(Debug)]
pub struct ServerTag;

#[derive(Serialize, Deserialize, Clone)]
struct Header {
    id: PacketId,
}

pub type PacketId = usize;

#[derive(Serialize, Deserialize, Clone)]
enum Packet {
    Ack(Header, HashSet<PacketId>),
    Message(Header, Message),
}

pub struct Socket {
    socket: UdpSocket,
    packet_cursor: PacketId,
    ack_recv_ids: HashSet<PacketId>,
    ack_send_ids: HashSet<PacketId>,
    send: HashMap<PacketId, (Packet, SocketAddr, Instant)>,
    recv: HashMap<SocketAddr, Vec<Packet>>,
}

impl Socket {
    fn bind<A: ToSocketAddrs>(addrs: A) -> Result<Self, Error> {
        match UdpSocket::bind(addrs) {
            Ok(mut socket) => {
                socket.set_nonblocking(true);
                Ok(Self {
                    socket,
                    packet_cursor: 0,
                    ack_recv_ids: Default::default(),
                    ack_send_ids: Default::default(),
                    send: Default::default(),
                    recv: Default::default(),
                })
            }
            Err(_) => Err(Error::FailedToBind),
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addrs: A) -> Result<(), Error> {
        self.socket
            .connect(addrs)
            .map_err(|_| Error::FailedToConnect)
    }

    fn encode(&self, packet: Packet) -> Result<Vec<u8>, Error> {
        let json = serde_json::to_string(&packet).map_err(|_| Error::FailedToSerialize)?;

        let mut buffer = vec![];

        let bytes = json.into_bytes();

        buffer.extend(Self::checksum(&bytes).into_iter());
        buffer.extend(bytes);

        Ok(buffer)
    }

    fn decode(&self, buffer: &[u8], len: usize) -> Result<Packet, Error> {
        const U64_BYTES: usize = mem::size_of::<u64>();

        let checksum = &buffer[..U64_BYTES];
        let payload = &buffer[U64_BYTES..len];

        if checksum != Self::checksum(payload) {
            Err(Error::InvalidChecksum)?
        }

        let bytes = payload.iter().copied().collect::<Vec<u8>>();

        let json = String::from_utf8(bytes).map_err(|_| Error::FailedToParse)?;

        serde_json::from_str::<Packet>(&json).map_err(|_| Error::FailedToDeserialize)
    }

    fn send(&mut self, message: Message) -> Result<usize, Error> {
        self.send_to(message, self.socket.peer_addr().unwrap())
    }

    fn checksum(data: &[u8]) -> [u8; 8] {
        let mut hasher = DefaultHasher::default();
        data.hash(&mut hasher);
        hasher.finish().to_be_bytes()
    }

    fn send_to<A: ToSocketAddrs>(&mut self, message: Message, addrs: A) -> Result<usize, Error> {
        let addr = addrs.to_socket_addrs().unwrap().next().unwrap();

        let id = self.packet_cursor;
        let packet = Packet::Message(Header { id }, message);

        let now = time::Instant::now();

        self.send.insert(id, (packet.clone(), addr, now));

        let mut data = VecDeque::from(self.encode(packet)?);

        let len = self
            .socket
            .send_to(data.make_contiguous(), addr)
            .map_err(|_| Error::FailedToSend)?;

        self.packet_cursor += 1;
        Ok(len)
    }

    //TODO calculate packet loss and display that to the user when it is severe
    fn ack(&mut self) -> Result<(), Error> {
        for (_, packets) in &mut self.recv {
            self.ack_recv_ids.extend(
                packets
                    .drain_filter(|packet| matches!(packet, Packet::Ack(_, _)))
                    .flat_map(|packet| {
                        let Packet::Ack(_, ids) = packet else {
                    panic!("?");
                };
                        ids
                    }),
            );
        }

        for id in self
            .ack_recv_ids
            .drain_filter(|id| self.send.contains_key(&id))
            .collect::<Vec<_>>()
        {
            self.send.remove(&id);
        }

        let mut send_ack = HashMap::<SocketAddr, Vec<PacketId>>::new();

        let now = time::Instant::now();

        for (id, _, addr, send_time) in self.send.iter_mut().map(|(a, (b, c, d))| (a, b, c, d)) {
            if now.duration_since(*send_time).as_secs_f32() >= ACK {
                send_ack.entry(*addr).or_default().push(*id);
                *send_time = now;
            }
        }

        for (addr, ids) in send_ack {
            let packets = ids
                .into_iter()
                .map(|id| self.send.get(&id).unwrap())
                .map(|(packet, _, _)| packet)
                .map(Clone::clone)
                .collect::<Vec<_>>();
            for packet in packets {
                self.socket
                    .send_to(&self.encode(packet)?, addr)
                    .map_err(|_| Error::FailedToSend)?;
            }
        }

        let mut send_ack = HashMap::<SocketAddr, HashSet<PacketId>>::new();

        for (addr, packets) in &self.recv {
            let mut extent = vec![];
            for id in packets
                .iter()
                .map(|packet| match packet {
                    Packet::Ack(header, _) => header.id,
                    Packet::Message(header, _) => header.id,
                })
                .filter(|id| !self.ack_send_ids.contains(&id))
            {
                extent.push((*addr, id));
            }
            for (addr, id) in extent {
                send_ack.entry(addr).or_default().insert(id);
                self.ack_send_ids.insert(id);
            }
        }

        for (addr, ids) in send_ack {
            let id = self.packet_cursor;
            let packet = Packet::Ack(
                Header {
                    id: self.packet_cursor,
                },
                ids,
            );
            self.send.insert(id, (packet.clone(), addr, now));
            self.socket
                .send_to(&self.encode(packet)?, addr)
                .map_err(|_| Error::FailedToSend)?;
            self.packet_cursor += 1;
        }

        Ok(())
    }

    fn recv(&mut self) -> Result<Vec<SocketAddr>, Error> {
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut addrs = vec![];
        while let Ok((len, addr)) = self.socket.recv_from(&mut buffer) {
            let packet = self.decode(&buffer, len)?;
            self.recv.entry(addr).or_default().push(packet);
            addrs.push(addr);
        }

        Ok(addrs)
    }

    pub fn get_from<F: Fn(&Message) -> bool>(
        &mut self,
        predicate: F,
        addr: SocketAddr,
    ) -> Vec<Message> {
        let Some(packets) = self.recv.get_mut(&addr) else{ 
            return vec![];
        };
        packets
            .drain_filter(|packet| match packet {
                Packet::Ack(_, _) => false,
                Packet::Message(_, message) => (predicate)(&message),
            })
            .map(|packet| match packet {
                Packet::Ack(_, _) => panic!("?"),
                Packet::Message(_, message) => message,
            })
            .collect::<Vec<_>>()
    }

    pub fn get<F: Fn(&Message) -> bool>(&mut self, predicate: F) -> Vec<Message> {
        self.get_from(predicate, self.socket.peer_addr().unwrap())
    }
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
    socket: Socket,
}

pub struct Connection {
    client: Option<Client>,
    start: Instant,
}

//#[profiling::all_functions]
impl Connection {
    pub fn check_connection(mut self) -> Result<Result<Client, Error>, Self> {
        let recv_result = self.client.as_mut().unwrap().recv();
        if let Err(e) = recv_result {
            return Ok(Err(e));
        }
        let ack_result = self.client.as_mut().unwrap().ack();
        if let Err(e) = ack_result {
            return Ok(Err(e));
        }
        let mut messages = vec![];
        messages.extend(
            self.client
                .as_mut()
                .unwrap()
                .get(|m| matches!(m, Message::Handshake)),
        );
        if messages.len() >= 1 {
            let message = messages.drain(0..1).next().unwrap();
            use Message::*;
            match message {
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

//#[profiling::all_functions]
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
            match Socket::bind(&format!("127.0.0.1:{}", port)) {
                Ok(socket) => break socket,
                _ => {}
            }
        };

        socket
            .connect(server_address)
            .map_err(|_| Error::FailedToConnect)?;

        let mut client = Client { socket };

        client
            .send(Message::Handshake)
            .map_err(|_| Error::FailedToSend)?;

        Ok(Connection {
            client: Some(client),
            start: Instant::now(),
        })
    }

    pub fn send(&mut self, message: Message) -> Result<usize, Error> {
        self.socket.send(message)
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        self.socket.recv().map(|_| ())
    }

    pub fn ack(&mut self) -> Result<(), Error> {
        self.socket.ack()
    }

    pub fn get<F: Fn(&Message) -> bool>(&mut self, predicate: F) -> Vec<Message> {
        self.socket.get(predicate)
    }
}

pub struct Server {
    conn_cursor: ClientId,
    conns: HashMap<SocketAddr, ClientId>,
    mapping: HashMap<ClientId, SocketAddr>,
    start: HashMap<ClientId, Instant>,
    active: HashSet<ClientId>,
    heartbeat: HashMap<ClientId, Instant>,
    socket: Socket,
}

//#[profiling::all_functions]
impl Server {
    pub fn bind() -> Result<Self, Error> {
        let socket = Socket::bind(&format!("0.0.0.0:{}", SERVER_PORT))?;

        Ok(Server {
            socket,
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

    pub fn send_to_all(&mut self, message: Message) -> Result<(), Error> {
        for client in self.active.iter().copied().collect::<Vec<_>>() {
            self.send(client, message.clone())?;
        }
        Ok(())
    }

    pub fn send(&mut self, client: ClientId, message: Message) -> Result<usize, Error> {
        let Some(&addr) = self.mapping.get(&client) else {
            Err(Error::ClientDoesNotExist)?
        };
        self.socket.send_to(message, addr)
    }

    pub fn recv(&mut self) -> Result<(), Error> {
        let addrs = self.socket.recv()?;
        for addr in addrs {
            let client = *self.conns.entry(addr).or_insert_with(|| {
                let conn = self.conn_cursor;
                self.conn_cursor.0 += 1;
                conn
            });
            self.mapping.entry(client).or_insert(addr);
            self.heartbeat
                .entry(client)
                .and_modify(|last| *last = Instant::now())
                .or_insert_with(|| Instant::now());
        }
        Ok(())
    }

    pub fn ack(&mut self) -> Result<(), Error> {
        self.socket.ack()
    }

    pub fn get<F: Fn(&Message) -> bool>(&mut self, client: ClientId, predicate: F) -> Vec<Message> {
        let Some(&addr) = self.mapping.get(&client) else {
            return vec![];
        };
        self.socket.get_from(predicate, addr)
    }
}
