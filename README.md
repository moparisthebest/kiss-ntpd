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
usage: kiss-ntpd [options...] [bind_addresses...]
 -h, --help                      print this usage text
 -V, -v, --version               Show version number then quit

 If no bind_addresses supplied, defaults to 0.0.0.0:123
```

There is an example systemd unit in `systemd/kiss-ntpd.service` which runs it with minimal permissions
and as locked down as possible.

Many thanks to [rsntp](https://github.com/mlichvar/rsntp) from which I forked this code.
