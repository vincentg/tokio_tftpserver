//! An UDP tftp_server based on Async tokio with privilege drop
//!
//! 

#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::{env, io};
use tokio::net::UdpSocket;
use privdrop;

mod tftp;
use tftp::tftpprotocol;

struct Server {
    socket: UdpSocket,
    buf: Vec<u8>,
    to_send: Option<(usize, SocketAddr)>,
}

impl Server {
    async fn run(self) -> Result<(), io::Error> {
        let Server {
            socket,
            mut buf,
            mut to_send,
        } = self;

        let mut context = None;
        loop {
            if let Some((size, peer)) = to_send {
                let new_context = tftpprotocol::recv(&buf[..size],size, context);
                context = new_context.clone();
                match new_context {
                    Some(ctx) => {
                        let reply_to_send = tftpprotocol::get_reply_command(ctx).unwrap();
                        let send = tftpprotocol::get_buffer_for_command(reply_to_send).unwrap();
                        socket.send_to(&send, &peer).await?;
                    }
                    None => {return Ok(())}
                }
            }
            to_send = Some(socket.recv_from(&mut buf).await?);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:69".to_string());

    let socket = UdpSocket::bind(&addr).await?;
    println!("Listening on: {}", socket.local_addr()?);
    println!("Dropping privileges");

    privdrop::PrivDrop::default()
        .chroot("/home/vgerard") // todo parse arguments
        .user("vgerard")
        .apply()
        .unwrap_or_else(|e| { panic!("Failed to drop privileges: {}", e) });

    let server = Server {
        socket,
        buf: vec![0; 1024],
        to_send: None,
    };

    // This starts the server task.
    server.run().await?;

    Ok(())
}
