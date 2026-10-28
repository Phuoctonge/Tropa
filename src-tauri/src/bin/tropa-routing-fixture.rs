use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Transport {
    Tcp,
    Udp,
}

struct Options {
    transport: Transport,
    target: SocketAddr,
    nonce: String,
    samples: u32,
    interval: Duration,
    timeout: Duration,
    fresh_sockets: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, &'static str> {
        let mut args = args.into_iter();
        let mut values = std::collections::HashMap::new();
        while let Some(name) = args.next() {
            if !matches!(
                name.as_str(),
                "--transport"
                    | "--target"
                    | "--nonce"
                    | "--samples"
                    | "--interval-ms"
                    | "--timeout-ms"
                    | "--socket-mode"
            ) {
                return Err("Unknown argument");
            }
            let value = args.next().ok_or("Missing argument value")?;
            if values.insert(name, value).is_some() {
                return Err("Duplicate argument");
            }
        }
        let transport = match values.get("--transport").map(String::as_str) {
            Some("tcp") => Transport::Tcp,
            Some("udp") => Transport::Udp,
            _ => return Err("Transport must be tcp or udp"),
        };
        let target: SocketAddr = values
            .get("--target")
            .ok_or("Missing target")?
            .parse()
            .map_err(|_| "Target must be an IP socket address")?;
        if target.port() == 0 || target.port() == 53 || target.ip().is_unspecified() || target.ip().is_multicast() {
            return Err("Target must be unicast on a non-DNS port");
        }
        let nonce = values.get("--nonce").ok_or("Missing nonce")?.clone();
        if !(16..=128).contains(&nonce.len()) || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Nonce must contain 16 to 128 hexadecimal characters");
        }
        let number = |name: &str, default: u64, min, max| -> Result<u64, &'static str> {
            let value = match values.get(name) {
                Some(value) => value.parse().map_err(|_| "Invalid numeric argument")?,
                None => default,
            };
            if !(min..=max).contains(&value) {
                return Err("Numeric argument is outside its limit");
            }
            Ok(value)
        };
        let fresh_sockets = match values.get("--socket-mode").map(String::as_str) {
            None | Some("persistent") => false,
            Some("fresh") => true,
            _ => return Err("Socket mode must be persistent or fresh"),
        };
        if transport == Transport::Tcp && !fresh_sockets {
            return Err("TCP probes require --socket-mode fresh");
        }
        Ok(Self {
            transport,
            target,
            nonce,
            samples: number("--samples", 1, 1, 10000)? as u32,
            interval: Duration::from_millis(number("--interval-ms", 0, 0, 60000)?),
            timeout: Duration::from_millis(number("--timeout-ms", 1000, 10, 10000)?),
            fresh_sockets,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    transport: Transport,
    reply_matched: bool,
    elapsed_ms: u64,
    samples: u32,
    matched: u32,
    lost: u32,
    invalid_replies: u32,
    median_ms: Option<u64>,
    observed_source_sha256: BTreeSet<String>,
}

fn response_source(bytes: &[u8], nonce: &str) -> Option<Option<IpAddr>> {
    if bytes.len() > 512 || !bytes.ends_with(b"\n") {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let fields: Vec<_> = text.trim_end_matches(['\r', '\n']).split(' ').collect();
    if fields.len() < 2 || fields[0] != "TROPA1" || fields[1] != nonce {
        return None;
    }
    match fields.len() {
        2 => Some(None),
        3 => Some(Some(fields[2].parse().ok()?)),
        _ => None,
    }
}

fn udp_socket(target: SocketAddr) -> std::io::Result<UdpSocket> {
    let socket = UdpSocket::bind(if target.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" })?;
    socket.connect(target)?;
    Ok(socket)
}

fn sample(
    options: &Options,
    nonce: &str,
    persistent: Option<&UdpSocket>,
    invalid: &mut u32,
) -> std::io::Result<Option<IpAddr>> {
    let request = format!("TROPA1 {nonce}\n");
    if options.transport == Transport::Tcp {
        let deadline = Instant::now() + options.timeout;
        let mut stream = TcpStream::connect_timeout(&options.target, options.timeout)?;
        stream.set_write_timeout(Some(options.timeout))?;
        stream.write_all(request.as_bytes())?;
        let mut reply = Vec::new();
        while reply.len() <= 512 && !reply.ends_with(b"\n") {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
            stream.set_read_timeout(Some(remaining))?;
            let mut buffer = [0u8; 513];
            let limit = 513 - reply.len();
            let size = stream.read(&mut buffer[..limit])?;
            if size == 0 {
                break;
            }
            reply.extend_from_slice(&buffer[..size]);
        }
        return response_source(&reply, nonce).ok_or_else(|| {
            *invalid += 1;
            std::io::Error::from(std::io::ErrorKind::InvalidData)
        });
    }

    let owned;
    let socket = match persistent {
        Some(socket) => socket,
        None => {
            owned = udp_socket(options.target)?;
            &owned
        }
    };
    socket.set_write_timeout(Some(options.timeout))?;
    socket.send(request.as_bytes())?;
    let deadline = Instant::now() + options.timeout;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
        socket.set_read_timeout(Some(remaining))?;
        let mut reply = [0u8; 513];
        let size = socket.recv(&mut reply)?;
        if let Some(source) = response_source(&reply[..size], nonce) {
            return Ok(source);
        }
        *invalid += 1;
    }
}

fn run(options: &Options) -> std::io::Result<Report> {
    let started = Instant::now();
    let socket = if options.transport == Transport::Udp && !options.fresh_sockets {
        Some(udp_socket(options.target)?)
    } else {
        None
    };
    let mut timings = Vec::new();
    let mut sources = BTreeSet::new();
    let mut invalid_replies = 0;
    for index in 0..options.samples {
        let sample_started = Instant::now();
        if let Ok(source) = sample(
            options,
            &format!("{}:{index}", options.nonce),
            socket.as_ref(),
            &mut invalid_replies,
        ) {
            timings.push(sample_started.elapsed().as_millis() as u64);
            if let Some(ip) = source {
                sources.insert(
                    Sha256::digest(ip.to_string().as_bytes())
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                );
            }
        }
        if index + 1 < options.samples {
            std::thread::sleep(options.interval.saturating_sub(sample_started.elapsed()));
        }
    }
    timings.sort_unstable();
    let matched = timings.len() as u32;
    Ok(Report {
        transport: options.transport,
        reply_matched: matched == options.samples,
        elapsed_ms: started.elapsed().as_millis() as u64,
        samples: options.samples,
        matched,
        lost: options.samples - matched,
        invalid_replies,
        median_ms: timings.get(timings.len() / 2).copied(),
        observed_source_sha256: sources,
    })
}

fn main() {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("Routing fixture: {message}");
            std::process::exit(2);
        }
    };
    match run(&options) {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report).expect("public fixture report"));
            if !report.reply_matched {
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("Routing fixture: socket initialization failed");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_and_stale_packets_cannot_pass_the_probe() {
        assert_eq!(
            response_source(b"TROPA1 a:0 192.0.2.1\n", "a:0"),
            Some(Some("192.0.2.1".parse().unwrap()))
        );
        assert_eq!(response_source(b"TROPA1 a:0\n", "a:0"), Some(None));
        for reply in [
            b"TROPA1 a:0\n".as_slice(),
            b"TROPA1 a:1 invalid\n",
            b"TROPA1 a:1\nextra",
            b"TROPA1 a:1 192.0.2.1 extra\n",
        ] {
            assert_eq!(response_source(reply, "a:1"), None);
        }
    }

    #[test]
    fn dns_ports_and_ambiguous_cli_arguments_are_rejected() {
        let args = |target: &str| {
            vec![
                "--transport".into(),
                "udp".into(),
                "--target".into(),
                target.into(),
                "--nonce".into(),
                "0123456789abcdef".into(),
            ]
        };
        assert!(Options::parse(args("127.0.0.1:53")).is_err());
        let mut duplicate = args("127.0.0.1:5555");
        duplicate.extend(["--target".into(), "127.0.0.1:5556".into()]);
        assert!(Options::parse(duplicate).is_err());
    }

    #[test]
    fn tcp_fixture_checks_each_short_connection_independently() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let target = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = [0u8; 512];
                let size = stream.read(&mut request).unwrap();
                let expected = format!("TROPA1 0123456789abcdef:{index}\n");
                assert_eq!(&request[..size], expected.as_bytes());
                stream.write_all(&request[..size]).unwrap();
            }
        });
        let options = Options {
            transport: Transport::Tcp,
            target,
            nonce: "0123456789abcdef".into(),
            samples: 2,
            interval: Duration::ZERO,
            timeout: Duration::from_secs(1),
            fresh_sockets: true,
        };
        let report = run(&options).unwrap();
        server.join().unwrap();
        assert_eq!(report.matched, 2);
        assert!(report.observed_source_sha256.is_empty());
    }

    #[test]
    fn local_udp_fixture_checks_nonce_and_hashes_observed_source() {
        let responder = UdpSocket::bind("127.0.0.1:0").unwrap();
        responder.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let target = responder.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut buffer = [0u8; 512];
            for _ in 0..3 {
                let (size, peer) = responder.recv_from(&mut buffer).unwrap();
                responder.send_to(b"TROPA1 stale:0\n", peer).unwrap();
                let request = std::str::from_utf8(&buffer[..size]).unwrap().trim_end();
                responder
                    .send_to(format!("{request} {}\n", peer.ip()).as_bytes(), peer)
                    .unwrap();
            }
        });
        let options = Options {
            transport: Transport::Udp,
            target,
            nonce: "0123456789abcdef".into(),
            samples: 3,
            interval: Duration::ZERO,
            timeout: Duration::from_secs(1),
            fresh_sockets: false,
        };
        let report = run(&options).unwrap();
        server.join().unwrap();
        assert!(report.reply_matched);
        assert_eq!(report.invalid_replies, 3);
        assert_eq!(report.observed_source_sha256.len(), 1);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("127.0.0.1"));
        assert!(!json.contains("0123456789abcdef"));
    }
}
