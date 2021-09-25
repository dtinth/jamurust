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
        .arg(
            Arg::with_name("jsonrpcport")
                .long("jsonrpcport")
                .takes_value(true)
                .help("Port for JSON RPC"),
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

    // If JSON-RPC port is specified, spawn a thread for handling JSON RPC
    if let Some(jsonrpc_port) = matches.value_of("jsonrpcport") {
        let jsonrpc_port = jsonrpc_port.parse::<u16>()?;
        tokio::spawn(async move {
            if let Err(error) = jsonrpc::run(jsonrpc_port).await {
                eprintln!("JSON RPC server error: {}", error);
            }
        });
    }

    // Create a Jamulus client
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

mod jsonrpc {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;

    #[derive(Serialize, Deserialize, Debug)]
    struct Request {
        id: serde_json::Value,
        method: String,
        params: serde_json::Value,
    }

    pub async fn run(jsonrpc_port: u16) -> Result<(), Box<dyn std::error::Error>> {
        // Create a TCP socket
        let jsonrpc_socket =
            tokio::net::TcpListener::bind(format!("127.0.0.1:{}", jsonrpc_port)).await?;
        loop {
            let (socket, _) = jsonrpc_socket.accept().await?;
            tokio::spawn(async move {
                if let Err(error) = run_json_rpc_connection(socket).await {
                    eprintln!("JSON RPC connection error: {}", error);
                }
            });
        }
    }

    async fn run_json_rpc_connection(
        mut socket: tokio::net::TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // Read a line from the socket
            let mut line = String::new();
            let mut reader = tokio::io::BufReader::new(&mut socket);
            if 0 == reader.read_line(&mut line).await? {
                return Ok(());
            }
            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(json) => match json {
                    serde_json::Value::Array(array) => {
                        let mut output: Vec<serde_json::Value> = vec![];
                        for value in array {
                            output.push(handle_request_and_serialize(value).await);
                        }
                        send_json(&mut socket, &serde_json::Value::Array(output)).await?;
                    }
                    _ => {
                        let response = handle_request_and_serialize(json).await;
                        send_json(&mut socket, &response).await?;
                    }
                },
                Err(error) => {
                    send_json(
                        &mut socket,
                        &create_error(-32700, format!("Parse error: {}", error), json!(null)),
                    )
                    .await?;
                }
            }
        }
    }
    async fn handle_request_and_serialize(json: serde_json::Value) -> serde_json::Value {
        match handle_request(json).await {
            Ok(result) => result,
            Err(error) => create_error(-32600, format!("Invalid request: {}", error), json!(null)),
        }
    }
    async fn handle_request(
        json: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let request = serde_json::from_value::<Request>(json)?;
        Ok(create_response(request.id, json!("UNIMPLEMENTED")))
    }
    fn create_error(code: i32, message: String, id: serde_json::Value) -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message
            }
        })
    }
    fn create_response(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        })
    }
    async fn send_json(
        socket: &mut tokio::net::TcpStream,
        json: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut string = serde_json::to_string(&json).unwrap();
        string.push('\n');
        socket.write_all(string.as_bytes()).await?;
        Ok(())
    }
}
