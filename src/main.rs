use nom::IResult;
use std::error::Error;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

mod crc;

fn main() {
    // Bind a UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.connect("127.0.0.1:22124").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    // Print the bound port
    println!("Bound to {}", socket.local_addr().unwrap());

    let mut client = JamulusClient::new(socket);
    client.run();
}

struct JamulusClient {
    socket: UdpSocket,
    next_counter_id: u8,
}

impl JamulusClient {
    fn new(socket: UdpSocket) -> Self {
        JamulusClient {
            socket,
            next_counter_id: 1,
        }
    }
    fn run(&mut self) {
        let mut silence = SilentOpusStream::new();

        // Receive a datagram with 100ms timeout
        loop {
            let mut buf = [0; 2048];
            if let Err(e) = self.socket.send(&silence.next()[..]) {
                println!("Unable to send: {}", e);
            }
            match self.socket.recv(&mut buf) {
                Ok(n) => {
                    let payload = &buf[..n];
                    match Message::parse(payload) {
                        Ok((_, msg)) => {
                            if let Err(e) = self.handle_message(msg) {
                                println!("Unable to handle message: {}", e);
                            }
                        }
                        Err(e) => {
                            self.handle_audio_packet(payload);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    println!("Timed out");
                }
                Err(e) => {
                    println!("Unable to receive: {}", e);
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
    fn handle_message(&mut self, msg: Message) -> Result<(), Box<dyn Error>> {
        println!("Received {:?}", msg);

        match msg.id {
            32 => {
                // Client ID
                let channel_id = msg.data[0];
                println!("Channel ID is {}", channel_id);
            }
            34 => {
                // Request split message support
            }
            24 => {
                // Client list
                let clients = ClientInfo::parse_all(msg.data);
                println!("Clients: {:?}", clients);
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

                self.send_message(20, &bytes);
            }
            11 => {
                // Request jitter buffer size
                self.send_message(10, &(4 as u16).to_le_bytes());
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
                let name = "listener";
                bytes.write(&(name.len() as u16).to_le_bytes()).unwrap();
                bytes.write(name.as_bytes()).unwrap();

                // City
                let city = "";
                bytes.write(&(city.len() as u16).to_le_bytes()).unwrap();
                bytes.write(city.as_bytes()).unwrap();

                self.send_message(25, &bytes);
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
            self.socket.send(&ack.to_bytes()).unwrap();
        }

        Ok(())
    }
    fn send_message(&mut self, message_id: u16, data: &[u8]) {
        let datagram = Message {
            id: message_id,
            counter: self.next_counter_id,
            data: data,
        };
        self.next_counter_id = self.next_counter_id.wrapping_add(1);
        self.socket.send(&datagram.to_bytes()).unwrap();
    }
    fn handle_audio_packet(&mut self, packet: &[u8]) {
        if packet.len() == 332 {
        } else {
            println!("Received unknown packet of length {}", packet.len());
        }
    }
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
