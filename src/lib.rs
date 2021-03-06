use async_trait::async_trait;
use nom::IResult;
use std::error::Error;
use std::future::Future;
use std::io::Write;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;

pub mod audio;
mod crc;
pub mod jitter;

pub struct JamulusClient<H: Handler> {
    name: String,
    socket: UdpSocket,
    next_counter_id: u8,
    handler: H,
    shutting_down: bool,
}
impl<H: Handler> JamulusClient<H> {
    pub fn new(socket: UdpSocket, name: String, handler: H) -> Self {
        JamulusClient {
            name,
            socket,
            next_counter_id: 1,
            handler,
            shutting_down: false,
        }
    }
    pub async fn run(&mut self, shutdown: impl Future) {
        tokio::select! {
            _ = self.communicate() => {}
            _ = shutdown => {}
        }

        eprintln!("Disconnecting...");
        self.shutting_down = true;
        self.send_message(1010, &[]).await;
    }
    async fn communicate(&mut self) {
        let mut silence = SilentOpusStream::new();
        let mut send_interval = tokio::time::interval(Duration::from_millis(100));

        while !self.shutting_down {
            let mut buf = [0; 2048];
            tokio::select! {
                recv_result = self.socket.recv(&mut buf) => {
                    match recv_result {
                        Ok(n) => {
                            self.handle_packet(&buf[..n]).await;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            eprintln!("Timed out");
                        }
                        Err(e) => {
                            eprintln!("Unable to receive: {}", e);
                            sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
                _ = send_interval.tick() => {
                    if let Err(e) = self.socket.send(&silence.next()[..]).await {
                        eprintln!("Unable to send audio data: {}", e);
                    }
                }
            }
        }
    }
    async fn handle_packet(&mut self, payload: &[u8]) {
        match Message::parse(payload) {
            Ok((_, msg)) => {
                if let Err(e) = self.handle_chat_text(msg).await {
                    eprintln!("Unable to handle message: {}", e);
                }
            }
            Err(_e) => {
                self.handle_audio_packet(payload).await;
            }
        }
    }
    async fn handle_chat_text<'a>(&mut self, msg: Message<'a>) -> Result<(), Box<dyn Error>> {
        eprintln!("Received {:?}", msg);

        match msg.id {
            32 => {
                // Client ID
                let channel_id = msg.data[0];
                eprintln!("Channel ID is {}", channel_id);
            }
            34 => {
                // Request split message support
            }
            24 => {
                // Client list
                match ClientInfo::parse_all(msg.data) {
                    Ok(clients) => {
                        eprintln!("Clients: {:?}", clients);

                        // Unmute each client
                        for client in clients {
                            let mut bytes = Vec::with_capacity(3);

                            // Client ID
                            bytes.write(&(client.channel_id).to_le_bytes())?;

                            // Gain
                            bytes.write(&(0x8000 as u16).to_le_bytes())?;

                            debug_assert_eq!(bytes.len(), 3);

                            self.send_message(13, &bytes).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("Unable to parse client list: {}", e);
                    }
                }
            }
            21 => {
                // Request network properties
                let mut bytes = Vec::with_capacity(19);

                // Packet size
                bytes.write(&(166 as u32).to_le_bytes()).unwrap();

                // Block size
                bytes.write(&(2 as u16).to_le_bytes()).unwrap();

                // Stereo
                bytes.write(&(2 as u8).to_le_bytes()).unwrap();

                // Sample rate
                bytes.write(&(48000 as u32).to_le_bytes()).unwrap();

                // Codec: Opus
                bytes.write(&(2 as u16).to_le_bytes()).unwrap();

                // Flags: Add sequence number
                bytes.write(&(1 as u16).to_le_bytes()).unwrap();

                // Codec options (none)
                bytes.write(&(0 as u32).to_le_bytes()).unwrap();

                debug_assert_eq!(bytes.len(), 19);
                self.send_message(20, &bytes).await;
            }
            11 => {
                // Request jitter buffer size
                self.send_message(10, &(4 as u16).to_le_bytes()).await;
            }
            23 => {
                // Request channel info
                let mut bytes = Vec::new();

                // Country
                bytes.write(&(0 as u16).to_le_bytes()).unwrap();

                // Instrument: Listener
                bytes.write(&(25 as u32).to_le_bytes()).unwrap();

                // Skill Level
                bytes.write(&(3 as u8).to_le_bytes()).unwrap();

                // Name
                bytes
                    .write(&(self.name.len() as u16).to_le_bytes())
                    .unwrap();
                bytes.write(self.name.as_bytes()).unwrap();

                // City
                let city = "";
                bytes.write(&(city.len() as u16).to_le_bytes()).unwrap();
                bytes.write(city.as_bytes()).unwrap();

                self.send_message(25, &bytes).await;
            }
            18 => {
                self.handler
                    .handle_chat_text(std::str::from_utf8(&msg.data[2..])?)
                    .await;
            }
            _ => {}
        }

        if msg.id != 1 && msg.id < 1000 {
            // Send acknowledgement
            let ack = Message {
                id: 1,
                counter: msg.counter,
                data: &msg.id.to_le_bytes(),
            };
            if let Err(error) = self.socket.send(&ack.to_bytes()).await {
                eprintln!("Unable to send acknowledgement packet: {}", error);
            }
        }

        Ok(())
    }
    async fn send_message(&mut self, message_id: u16, data: &[u8]) {
        let datagram = Message {
            id: message_id,
            counter: self.next_counter_id,
            data: data,
        };
        self.next_counter_id = self.next_counter_id.wrapping_add(1);
        if let Err(error) = self.socket.send(&datagram.to_bytes()).await {
            eprintln!(
                "Unable to send message {} with counter {}: {}",
                datagram.id, datagram.counter, error
            );
        }
    }
    async fn handle_audio_packet(&mut self, packet: &[u8]) {
        if packet.len() == 332 {
            self.handler
                .handle_opus_packet(&packet[0..165], packet[165])
                .await;
            self.handler
                .handle_opus_packet(&packet[166..331], packet[331])
                .await;
        } else {
            eprintln!("Received unknown packet of length {}", packet.len());
        }
    }
}

#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle_opus_packet(&mut self, _packet: &[u8], _sequence_number: u8) {}
    async fn handle_chat_text(&mut self, _text: &str) {}
}

#[derive(Debug)]
struct Message<'a> {
    id: u16,
    counter: u8,
    data: &'a [u8],
}
impl Message<'_> {
    fn parse<'a>(input_bytes: &'a [u8]) -> IResult<&'a [u8], Message<'a>> {
        // Use `nom` to parse the message.
        // All numbers are in little endian.
        let bytes = input_bytes;

        // First two bytes are 0x00 0x00.
        let (bytes, _) = nom::bytes::complete::tag([0x00, 0x00])(bytes)?;

        // Next two bytes are the message ID.
        let (bytes, id) = nom::number::complete::le_u16(bytes)?;

        // The next byte is the counter.
        let (bytes, counter) = nom::number::complete::le_u8(bytes)?;

        // The next two bytes are the length of the data.
        let (bytes, len) = nom::number::complete::le_u16(bytes)?;

        // The next `len` bytes are the data.
        let (bytes, data) = nom::bytes::complete::take(len)(bytes)?;

        // Verify the checksum.
        let expected = crc::crc(&input_bytes[0..((len as usize) + 7)]).to_le_bytes();

        // Finally, two more bytes for the checksum.
        let (bytes, _) = nom::bytes::complete::tag(expected)(bytes)?;

        // Return the parsed message.
        Ok((bytes, Message { id, counter, data }))
    }
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(9 + self.data.len());
        bytes.write(&[0x00, 0x00]).unwrap();
        bytes.write(&self.id.to_le_bytes()).unwrap();
        bytes.write(&self.counter.to_le_bytes()).unwrap();
        bytes
            .write(&(self.data.len() as u16).to_le_bytes())
            .unwrap();
        bytes.write(&self.data).unwrap();
        let crc = crc::crc(&bytes);
        bytes.write(&crc.to_le_bytes()).unwrap();
        bytes
    }
}

struct SilentOpusStream {
    counter: u8,
}
impl SilentOpusStream {
    pub fn new() -> Self {
        SilentOpusStream { counter: 0 }
    }
    pub fn next(&mut self) -> [u8; 332] {
        let mut packet: [u8; 332] = [0; 332];
        self.write(&mut packet[..]);
        self.write(&mut packet[166..]);
        packet
    }
    fn write(&mut self, slice: &mut [u8]) {
        slice[0] = 0x04;
        slice[1] = 0xff;
        slice[2] = 0xfe;
        self.counter = self.counter.wrapping_add(1);
        slice[165] = self.counter;
    }
}

#[derive(Debug)]
struct ClientInfo {
    channel_id: u8,
    country_id: u16,
    instrument_id: u32,
    skill_level: u8,
    name: String,
    city: String,
}
impl ClientInfo {
    fn parse_client<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], ClientInfo> {
        let (bytes, channel_id) = nom::number::complete::le_u8(bytes)?;
        let (bytes, country_id) = nom::number::complete::le_u16(bytes)?;
        let (bytes, instrument_id) = nom::number::complete::le_u32(bytes)?;
        let (bytes, skill_level) = nom::number::complete::le_u8(bytes)?;
        let (bytes, _ip) = nom::number::complete::le_u32(bytes)?;
        let (bytes, name_len) = nom::number::complete::le_u16(bytes)?;
        let (bytes, name) = nom::bytes::complete::take(name_len)(bytes)?;
        let name_str = match std::str::from_utf8(name) {
            Ok(s) => s,
            Err(_) => {
                return Err(nom::Err::Failure(nom::error::make_error(
                    bytes,
                    nom::error::ErrorKind::Satisfy,
                )))
            }
        };
        let (bytes, city_len) = nom::number::complete::le_u16(bytes)?;
        let (bytes, city) = nom::bytes::complete::take(city_len)(bytes)?;
        let city_str = match std::str::from_utf8(city) {
            Ok(s) => s,
            Err(_) => {
                return Err(nom::Err::Failure(nom::error::make_error(
                    bytes,
                    nom::error::ErrorKind::Satisfy,
                )))
            }
        };
        Ok((
            bytes,
            ClientInfo {
                channel_id,
                country_id,
                instrument_id,
                skill_level,
                name: String::from(name_str),
                city: String::from(city_str),
            },
        ))
    }
    fn parse_all<'a>(mut bytes: &'a [u8]) -> Result<Vec<ClientInfo>, Box<dyn Error + 'a>> {
        let mut clients = Vec::new();
        loop {
            if bytes.len() <= 0 {
                break;
            }
            let (next_bytes, client) = Self::parse_client(bytes)?;
            clients.push(client);
            bytes = next_bytes;
        }
        Ok(clients)
    }
}
