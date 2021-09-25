use clap::{App, Arg};
use jamurust::{self, JamulusClient};
use std::io::Write;
use std::net::UdpSocket;
use std::time::Duration;

fn main() {
    let matches = App::new("jam-listener")
        .version("0.1.0")
        .author("dtinth <dtinth@spacet.me>")
        .about("Stream sound from a Jamulus server as s16le")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .takes_value(true)
                .default_value("127.0.0.1:22124")
                .help("Jamulus Server to connect to"),
        )
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .takes_value(true)
                .default_value("0.0.0.0:0")
                .help("UDP bind address"),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .takes_value(true)
                .default_value("listener")
                .help("Client name"),
        )
        .get_matches();

    // Bind a UDP socket
    let socket = UdpSocket::bind(matches.value_of("bind").unwrap()).unwrap();
    socket.connect(matches.value_of("server").unwrap()).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    // Print the bound port
    eprintln!("Bound to {}", socket.local_addr().unwrap());

    let mut client = JamulusClient::new(
        socket,
        String::from(matches.value_of("name").unwrap()),
        AudioHandler::new(),
    );
    client.run();
}

struct AudioHandler {
    audio_decoder: jamurust::audio::Decoder,
    jitter_buffer: jamurust::jitter::JitterBuffer<Vec<u8>>,
}
impl AudioHandler {
    fn new() -> Self {
        Self {
            audio_decoder: jamurust::audio::Decoder::new(),
            jitter_buffer: jamurust::jitter::JitterBuffer::new(96),
        }
    }
}
impl jamurust::Handler for AudioHandler {
    fn handle_opus_packet(&mut self, packet: &[u8], sequence_number: u8) {
        if let Some(opus_packet) = self.jitter_buffer.put_in(packet.to_vec(), sequence_number) {
            let mut output = [0 as i16; 1000];
            let decoded = self.audio_decoder.decode(&opus_packet, &mut output);
            for value in output[..decoded * 2].iter() {
                let b = value.to_le_bytes();
                std::io::stdout().write_all(&b).unwrap();
            }
        }
    }
}
