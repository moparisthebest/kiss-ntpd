/*
kiss-ntpd: an NTP server that Keeps It Simple, Stupid
Copyright (C) 2017  Miroslav Lichvar
Copyright (C) 2021  Travis Burtrum

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use std::convert::TryInto;
use std::env;
use std::io;
use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;
use std::time::SystemTime;

#[derive(Debug, Copy, Clone)]
struct NtpTimestamp {
    ts: u64,
}

impl NtpTimestamp {
    fn now() -> NtpTimestamp {
        let now = SystemTime::now();
        let dur = now.duration_since(std::time::UNIX_EPOCH).unwrap(); // this should be unable to happen
        let secs = dur.as_secs() + 2208988800; // 1900 epoch
        let nanos = dur.subsec_nanos();

        NtpTimestamp {
            ts: (secs << 32) + (nanos as f64 * 4.294967296) as u64,
        }
    }

    fn zero() -> NtpTimestamp {
        NtpTimestamp { ts: 0 }
    }

    fn read(buf: &[u8]) -> NtpTimestamp {
        // this unwrap can never fail because we always send in exactly 8 bytes
        NtpTimestamp {
            ts: u64::from_be_bytes(buf.try_into().unwrap()),
        }
    }

    fn write(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.ts.to_be_bytes());
    }
}

impl PartialEq for NtpTimestamp {
    fn eq(&self, other: &NtpTimestamp) -> bool {
        self.ts == other.ts
    }
}

#[derive(Debug, Copy, Clone)]
struct NtpFracValue {
    val: u32,
}

impl NtpFracValue {
    fn read(buf: &[u8]) -> NtpFracValue {
        // this unwrap can never fail because we always send in exactly 4 bytes
        NtpFracValue {
            val: u32::from_be_bytes(buf.try_into().unwrap()),
        }
    }

    fn write(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.val.to_be_bytes());
    }

    fn zero() -> NtpFracValue {
        NtpFracValue { val: 0 }
    }
}

#[derive(Debug)]
struct NtpPacket {
    remote_addr: SocketAddr,
    local_ts: NtpTimestamp,

    leap: u8,
    version: u8,
    mode: u8,
    stratum: u8,
    poll: i8,
    precision: i8,
    delay: NtpFracValue,
    dispersion: NtpFracValue,
    ref_id: u32,
    ref_ts: NtpTimestamp,
    orig_ts: NtpTimestamp,
    rx_ts: NtpTimestamp,
    tx_ts: NtpTimestamp,
}

impl NtpPacket {
    fn receive(socket: &UdpSocket) -> io::Result<NtpPacket> {
        let mut buf = [0; 1024];

        let (len, addr) = socket.recv_from(&mut buf)?;

        let local_ts = NtpTimestamp::now();

        if len < 48 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "Packet too short"));
        }

        let leap = buf[0] >> 6;
        let version = (buf[0] >> 3) & 0x7;
        let mode = buf[0] & 0x7;

        if version < 1 || version > 4 {
            return Err(Error::new(ErrorKind::Other, "Unsupported version"));
        }

        Ok(NtpPacket {
            remote_addr: addr,
            local_ts: local_ts,
            leap: leap,
            version: version,
            mode: mode,
            stratum: buf[1],
            poll: buf[2] as i8,
            precision: buf[3] as i8,
            delay: NtpFracValue::read(&buf[4..8]),
            dispersion: NtpFracValue::read(&buf[8..12]),
            // this unwrap can never fail because we always send in exactly 4 bytes
            ref_id: u32::from_be_bytes((&buf[12..16]).try_into().unwrap()),
            ref_ts: NtpTimestamp::read(&buf[16..24]),
            orig_ts: NtpTimestamp::read(&buf[24..32]),
            rx_ts: NtpTimestamp::read(&buf[32..40]),
            tx_ts: NtpTimestamp::read(&buf[40..48]),
        })
    }

    fn send(&self, socket: &UdpSocket) -> io::Result<usize> {
        let mut buf = [0; 48];

        buf[0] = self.leap << 6 | self.version << 3 | self.mode;
        buf[1] = self.stratum;
        buf[2] = self.poll as u8;
        buf[3] = self.precision as u8;
        self.delay.write(&mut buf[4..8]);
        self.dispersion.write(&mut buf[8..12]);
        &mut buf[12..16].copy_from_slice(&self.ref_id.to_be_bytes());
        self.ref_ts.write(&mut buf[16..24]);
        self.orig_ts.write(&mut buf[24..32]);
        self.rx_ts.write(&mut buf[32..40]);
        self.tx_ts.write(&mut buf[40..48]);

        socket.send_to(&buf, self.remote_addr)
    }

    fn is_request(&self) -> bool {
        self.mode == 1 || self.mode == 3 || (self.mode == 0 && self.version == 1 && self.remote_addr.port() != 123)
    }

    fn make_response(&self) -> Option<NtpPacket> {
        if !self.is_request() {
            return None;
        }

        Some(NtpPacket {
            remote_addr: self.remote_addr,
            local_ts: NtpTimestamp::zero(),
            leap: 0,
            version: self.version,
            mode: if self.mode == 1 { 2 } else { 4 },
            stratum: 8,
            poll: self.poll,
            precision: 0,
            delay: NtpFracValue::zero(),
            dispersion: NtpFracValue::zero(),
            ref_id: 0,
            ref_ts: NtpTimestamp::now(),
            orig_ts: self.tx_ts,
            rx_ts: self.local_ts,
            tx_ts: NtpTimestamp::now(),
        })
    }
}

struct NtpServer {
    socket: UdpSocket,
    debug: bool,
}

impl NtpServer {
    fn new(local_addr: String, debug: bool) -> NtpServer {
        NtpServer {
            socket: UdpSocket::bind(local_addr).expect("could not bind to socket"),
            debug: debug,
        }
    }

    fn process_requests(debug: bool, socket: UdpSocket) {
        println!("Server thread started");

        loop {
            match NtpPacket::receive(&socket) {
                Ok(request) => {
                    if debug {
                        println!("received {:?}", request);
                    }

                    match request.make_response() {
                        Some(response) => match response.send(&socket) {
                            Ok(_) => {
                                if debug {
                                    println!("sent {:?}", response);
                                }
                            }
                            Err(e) => println!("failed to send packet to {}: {}", response.remote_addr, e),
                        },
                        None => {}
                    }
                }
                Err(e) => {
                    println!("failed to receive packet: {}", e);
                }
            }
        }
    }

    fn run(self) {
        NtpServer::process_requests(self.debug, self.socket);
    }
}

fn arg_to_env(arg: &str) -> Option<String> {
    if !arg.starts_with("--") {
        return None;
    }
    let env = "KISS_NTPD_".to_owned();
    let mut env = env + &arg.trim_matches('-').replace("-", "_");
    env.make_ascii_uppercase();
    Some(env)
}

fn env_for_arg(arg: &str) -> Option<String> {
    arg_to_env(arg).and_then(|key| std::env::var(key).ok())
}

pub struct Args<'a> {
    args: &'a Vec<String>,
}

impl<'a> Args<'a> {
    pub fn new(args: &'a Vec<String>) -> Args {
        Args { args }
    }
    pub fn flag(&self, flag: &'a str) -> bool {
        if self.args.contains(&flag.to_owned()) {
            return true;
        }
        // because env we want slightly special handling of empty/0/false
        match env_for_arg(flag) {
            Some(env) => &env != "" && &env != "0" && &env != "false",
            None => false,
        }
    }
    pub fn get_option(&self, flags: &[&'a str]) -> Option<String> {
        for flag in flags.iter() {
            let mut found = false;
            for arg in self.args.iter() {
                if found {
                    return Some(arg.to_owned());
                }
                if arg == flag {
                    found = true;
                }
            }
        }
        // no matching arguments are found, so check env variables as a fallback
        for flag in flags.iter() {
            let env = env_for_arg(flag);
            if env.is_some() {
                return env;
            }
        }
        return None;
    }
    pub fn get_str(&self, flags: &[&'a str], def: &'a str) -> String {
        match self.get_option(flags) {
            Some(ret) => ret,
            None => def.to_owned(),
        }
    }
    pub fn get<T: FromStr>(&self, flags: &[&'a str], def: T) -> T {
        match self.get_option(flags) {
            Some(ret) => match ret.parse::<T>() {
                Ok(ret) => ret,
                Err(_) => def, // or panic
            },
            None => def,
        }
    }
}

fn main() {
    let raw_args = env::args().collect();
    let args = Args::new(&raw_args);

    if args.flag("-V") || args.flag("-v") || args.flag("--version") {
        println!("kiss-ntpd {} ", env!("CARGO_PKG_VERSION"));
        return;
    }

    let default_udp_host = "0.0.0.0:123";

    let bind = args.get_str(&["-b", "--bind"], default_udp_host).to_owned();

    if args.flag("-h") || args.flag("--help") {
        println!(
            r#"usage: kiss-ntpd [options...]
 -b, --bind                      address to bind to, default '{}'
 -h, --help                      print this usage text
 -V, -v, --version               Show version number then quit
 -d, --debug                     Print packets sent and recieved, very verbose

 Environment variable support:
 You if environmental variable KISS_NTPD_BIND is set, it is used in place of --bind
 Also KISS_NTPD_DEBUG=true can be used in place of --debug
        "#,
            default_udp_host
        );
        return;
    }

    let server = NtpServer::new(bind, args.flag("-d") || args.flag("--debug"));

    server.run();
}
