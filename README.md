kiss-ntpd
---------

[![Build Status](https://ci.moparisthe.best/job/moparisthebest/job/kiss-ntpd/job/master/badge/icon%3Fstyle=plastic)](https://ci.moparisthe.best/job/moparisthebest/job/kiss-ntpd/job/master/)
[![crates.io](https://img.shields.io/crates/v/kiss-ntpd.svg)](https://crates.io/crates/kiss-ntpd)

An NTP server that Keeps It Simple, Stupid.

It simply responds to NTP queries with the current system time, it doesn't fuss with 
leap seconds or stratum or any of those other things we don't care about.  It will
simply synchronize your clock to the server's clock rather closely and that's it.

Host this on your router for all your LAN clients, and let systemd-timesyncd or another
ntp client keep that clock in sync.

##### Usage

```
$ kiss-ntpd -h
usage: kiss-ntpd [options...]
 -b, --bind                      address to bind to, default '0.0.0.0:123'
 -h, --help                      print this usage text
 -V, -v, --version               Show version number then quit
 -d, --debug                     Print packets sent and recieved, very verbose

 Environment variable support:
 You if environmental variable KISS_NTPD_BIND is set, it is used in place of --bind
 Also KISS_NTPD_DEBUG=true can be used in place of --debug
```

There is an example systemd unit in `systemd/kiss-ntpd.service` which runs it with minimal permissions
and as locked down as possible.

Many thanks to [rsntp](https://github.com/mlichvar/rsntp) from which I forked this code.
