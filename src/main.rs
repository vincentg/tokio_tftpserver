//! An UDP tftp_server based on Async tokio with privilege drop
//!
//! 

#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::{io,str::FromStr};
use clap::Parser;

use tokio::net::UdpSocket;

#[cfg(unix)]
use privdrop;

mod tftp;
use tftp::tftpprotocol;

struct Server {
    socket: UdpSocket,
    buf: Vec<u8>,
    to_send: Option<(usize, SocketAddr)>,
}

#[derive(Parser,Debug)]
struct Args {
    #[arg(short,long,default_value_t = std::net::IpAddr::from_str("127.0.0.1").unwrap())]
    bind: std::net::IpAddr,

    #[arg(short,long,default_value_t = 69)]
    port: u16,

    #[cfg(unix)]
    #[arg(short,long,value_name ="USER_TO_DROP_PRIVILEGES_TO")]
    user: String,

    #[cfg(unix)]
    #[arg(short,long,value_name ="BASE_DIRECTORY", value_hint = clap::ValueHint::DirPath)]
    directory: std::path::PathBuf,

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
                let new_context = tftpprotocol::recv(&buf[..size], size, context);
                context = new_context.clone();
                
                match new_context {
                    Some(ctx) => {
                        // Valid context - process and send reply
                        match tftpprotocol::get_reply_command(ctx) {
                            Some(reply_to_send) => {
                                match tftpprotocol::get_buffer_for_command(reply_to_send) {
                                    Some(send) => {
                                        if let Err(e) = socket.send_to(&send, &peer).await {
                                            eprintln!("Error {} sending to client {}", e, peer);
                                        }
                                    }
                                    None => {
                                        eprintln!("Failed to serialize command for client {}", peer);
                                    }
                                }
                            }
                            None => {
                                eprintln!("No reply command generated for client {}", peer);
                            }
                        }
                    }
                    None => {
                        // Context is None - either client error or end of transfer
                        // Reset context and continue serving other clients
                        eprintln!("Transfer ended or error occurred for client {}, ready for new connections", peer);
                        context = None;
                    }
                }
            }
            
            // Continue listening for new packets
            to_send = Some({
                let mut retries = 0;
                const MAX_RETRIES: u32 = 3;
                loop {
                    match socket.recv_from(&mut buf).await {
                        Ok(v) => break v,
                        Err(e) if retries < MAX_RETRIES && should_retry(&e) => {
                            retries += 1;
                            eprintln!("recv_from failed (attempt {}): {}", retries, e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        }
                        Err(e) => {
                            eprintln!("Fatal recv_from error: {}", e);
                            return Err(e);
                        }
                    }
                }
            });
        }
    }
}

fn should_retry(error: &io::Error) -> bool {
    match error.kind() {
        io::ErrorKind::WouldBlock | 
        io::ErrorKind::TimedOut | 
        io::ErrorKind::ConnectionReset |
        io::ErrorKind::Interrupted => true,
        _ => false,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let addr = format!("{}:{}",args.bind,args.port); 

    let socket = UdpSocket::bind(&addr).await?;
    println!("Listening on: {}", socket.local_addr()?);
    
    #[cfg(unix)]
    println!("Dropping privileges");

    #[cfg(unix)]
    privdrop::PrivDrop::default()
        .chroot(args.directory) 
        .user(args.user)
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
