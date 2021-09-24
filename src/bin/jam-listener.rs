use clap::{App, Arg};
use jamurust::JamulusClient;
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

    let mut client = JamulusClient::new(socket, String::from(matches.value_of("name").unwrap()));
    client.run();
}
