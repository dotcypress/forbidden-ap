use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    time::Duration,
};

pub struct SimpleDns {
    footer: [u8; 16],
    socket: UdpSocket,
}

impl SimpleDns {
    pub fn try_new(addr: Ipv4Addr) -> io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddrV4::new(addr, 53))?;
        socket.set_read_timeout(Some(Duration::from_millis(10)))?;
        let mut footer = [
            0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x04, 0x00, 0x00,
            0x00, 0x00,
        ];
        footer[12..].copy_from_slice(&addr.octets());
        Ok(Self { footer, socket })
    }

    pub fn poll(&mut self) -> io::Result<()> {
        let mut scratch = [0; 128];
        match self.socket.recv_from(&mut scratch) {
            Ok((len, addr)) => {
                if len > 100 {
                    log::warn!("Received DNS request with invalid packet size: {}", len);
                } else {
                    scratch[2] |= 0x80;
                    scratch[3] |= 0x80;
                    scratch[7] = 0x01;
                    let total = len + self.footer.len();
                    scratch[len..total].copy_from_slice(&self.footer);
                    self.socket.send_to(&scratch[0..total], addr)?;
                }
                Ok(())
            }
            Err(err) => match err.kind() {
                io::ErrorKind::TimedOut => Ok(()),
                _ => Err(err),
            },
        }
    }
}
