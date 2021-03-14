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

use std::io::{Error, ErrorKind, Result};
use std::net::UdpSocket;
use std::time::SystemTime;

fn ts_now() -> [u8; 8] {
    let now = SystemTime::now();
    let dur = now.duration_since(std::time::UNIX_EPOCH).unwrap(); // this should be unable to happen
    let secs = dur.as_secs() + 2208988800; // 1900 epoch
    let nanos = dur.subsec_nanos();

    ((secs << 32) + (nanos as f64 * 4.294967296) as u64).to_be_bytes()
}

struct NtpServer {
    socket: UdpSocket,
    buf: [u8; 48],
}

impl NtpServer {
    fn new(local_addr: String) -> NtpServer {
        NtpServer {
            socket: UdpSocket::bind(local_addr).expect("could not bind to socket"),
            buf: [0u8; 48],
        }
    }

    fn respond(&mut self) -> Result<usize> {
        let (len, remote_addr) = self.socket.recv_from(&mut self.buf)?;

        if len < 48 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "Packet too short"));
        }

        let version = (self.buf[0] >> 3) & 0x7;
        let mode = self.buf[0] & 0x7;

        if version < 1 || version > 4 {
            return Err(Error::new(ErrorKind::Other, "Unsupported version"));
        }

        if !(mode == 1 || mode == 3 || (mode == 0 && version == 1 && remote_addr.port() != 123)) {
            return Err(Error::new(ErrorKind::Other, "Not a valid NTP request"));
        }

        let ts = ts_now();

        // first 0u8 here is leap
        self.buf[0] = 0u8 << 6 | version << 3 | if mode == 1 { 2 } else { 4 };
        self.buf[1] = 8; // stratum
        // poll is at index 2 in both request and response, so do nothing
        self.buf[3] = 0; // precision
        // delay (4 bytes), dispersion (4 bytes), ref_id (4 bytes), but we don't really care about these
        // we could set them to 0 but should we bother?
        //&mut self.buf[4..16].copy_from_slice(&[0u8; 12]);
        &mut self.buf[16..24].copy_from_slice(&ts); // ref_ts
        // orig_ts needs moved from 40..48 in request to 24..32 in response
        let (dst, src) = self.buf.split_at_mut(40);
        dst[24..32].copy_from_slice(&src[..8]); // orig_ts
        &mut self.buf[32..40].copy_from_slice(&ts); // rx_ts
        &mut self.buf[40..48].copy_from_slice(&ts); // tx_ts

        self.socket.send_to(&self.buf, remote_addr)
    }

    fn run(mut self) {
        loop {
            if let Err(e) = self.respond() {
                eprintln!("error: {}", e);
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);

    let default_udp_host = "0.0.0.0:123";

    let first_arg = args.next().map_or(default_udp_host.to_owned(), |a| a.to_owned());

    if first_arg == "-h" || first_arg == "--help" {
        println!(
            r#"usage: kiss-ntpd [options...] [bind_addresses...]
 -h, --help                      print this usage text
 -V, -v, --version               Show version number then quit

 If no bind_addresses supplied, defaults to {}
        "#,
            default_udp_host
        );
        return;
    } else if first_arg == "-V" || first_arg == "-v" || first_arg == "--version" {
        println!("kiss-ntpd {} ", env!("CARGO_PKG_VERSION"));
        return;
    }

    for bind_address in args {
        std::thread::spawn(|| {
            NtpServer::new(bind_address).run();
        });
    }

    NtpServer::new(first_arg).run();
}
