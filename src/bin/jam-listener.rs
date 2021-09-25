use clap::{App, Arg};
use jamurust::{self, JamulusClient};
use std::io::Write;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let socket = UdpSocket::bind(matches.value_of("bind").unwrap()).await?;
    socket.connect(matches.value_of("server").unwrap()).await?;

    // Print the bound port
    eprintln!("Bound to {}", socket.local_addr().unwrap());

    // Create a channel for receiving shutdown conditions
    let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel::<()>();
    let shutdown_condition = async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = shutdown_rx.recv() => {},
        }
    };

    let mut client = JamulusClient::new(
        socket,
        String::from(matches.value_of("name").unwrap()),
        AudioHandler::new(shutdown_tx),
    );
    client.run(shutdown_condition).await;
    Ok(())
}

struct AudioHandler {
    audio_decoder: jamurust::audio::Decoder,
    jitter_buffer: jamurust::jitter::JitterBuffer<Vec<u8>>,
    shutdown_tx: mpsc::UnboundedSender<()>,
    dead: bool,
}
impl AudioHandler {
    fn new(shutdown_tx: mpsc::UnboundedSender<()>) -> Self {
        Self {
            audio_decoder: jamurust::audio::Decoder::new(),
            jitter_buffer: jamurust::jitter::JitterBuffer::new(96),
            shutdown_tx,
            dead: false,
        }
    }
}
impl jamurust::Handler for AudioHandler {
    fn handle_opus_packet(&mut self, packet: &[u8], sequence_number: u8) {
        if self.dead {
            return;
        }
        if let Some(opus_packet) = self.jitter_buffer.put_in(packet.to_vec(), sequence_number) {
            let mut output = [0 as i16; 1000];
            let decoded = self.audio_decoder.decode(&opus_packet, &mut output);
            for value in output[..decoded * 2].iter() {
                let b = value.to_le_bytes();
                if let Err(err) = std::io::stdout().write_all(&b) {
                    eprintln!("Error writing to stdout: {}", err);
                    self.shutdown_tx.send(()).unwrap();
                    self.dead = true;
                    return;
                }
            }
        }
    }
}
