pub mod tftpprotocol {
    use std::io::Cursor;
    use std::io::BufRead;
    use std::io::Read;
    use std::io::Write;
    use byteorder::{BigEndian};
    use byteorder::{ReadBytesExt,WriteBytesExt};
    use std::convert::TryFrom;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Seek;
    use std::io::SeekFrom;
    use crate::tftp_error::TftpError;
    use log::{info, warn, error, debug};

    enum Opcode {
        RRQ = 1, // Read request
        WRQ = 2, // Write request
        DATA = 3,
        ACK  = 4,
        ERROR = 5,
        UNKNOWN = -1
    }

    impl TryFrom<u16> for Opcode {
        type Error = &'static str;

        fn try_from(opcode: u16) -> Result<Self, Self::Error> {
            match opcode {
                1 => Ok(Opcode::RRQ),
                2 => Ok(Opcode::WRQ),
                3 => Ok(Opcode::DATA),
                4 => Ok(Opcode::ACK),
                5 => Ok(Opcode::ERROR),
                _ => Ok(Opcode::UNKNOWN)
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum Command {
        RRQ  {filename : String, mode:String},
        WRQ  {filename : String, mode:String},
        DATA {blocknum : u16, data:Vec<u8>},
        ACK  {blocknum : u16},
        ERROR {errorcode :u16, errmsg:String}
    }

    #[derive(Debug, Clone)]
    pub struct OpContext {
        pub current_op : Command,  // RRQ or WRQ
        _block_num : u16,      // For RRQ last read block, for WRQ, last written
        ack_num   : u16,       // last ACK received (to detect timeout)
        filename  : String,
        mode      : String
    }

    fn build_new_context(current_op: Command) -> Option<OpContext> {
        // TODO find how to do that without clone 
        let saved_op = current_op.clone();
        match current_op {
            Command::RRQ{filename, mode} | Command::WRQ{filename, mode} =>
                return Some( OpContext {
                    current_op: saved_op,
                    _block_num:0,
                    ack_num:0,
                    filename,
                    mode
                }),
            _ => return None
        }     
    }


    // Updated parse_command with logging
    fn parse_command(opcode: Opcode, reader: &mut Cursor<&[u8]>) -> Command {
        // Helper function for safe string parsing
        fn parse_null_terminated_string(reader: &mut Cursor<&[u8]>) -> Result<String, TftpError> {
            let mut buffer: Vec<u8> = Vec::new();
            reader.read_until(0, &mut buffer).map_err(|_| TftpError::MalformedPacket)?;
            
            if buffer.is_empty() || buffer.last() != Some(&0) {
                return Err(TftpError::MalformedPacket);
            }
            
            buffer.pop(); // Remove null terminator
            String::from_utf8(buffer).map_err(|_| TftpError::MalformedPacket)
        }

        // Inner function for RRQ/WRQ shared parsing logic 
        fn parse_filename_mode(reader: &mut Cursor<&[u8]>) -> Result<(String,String), TftpError> {
            let filename = parse_null_terminated_string(reader)?;
            let mode = parse_null_terminated_string(reader)?;
            Ok((filename, mode))
        }

        match opcode {
            Opcode::RRQ => {
                debug!("Processing RRQ packet");
                match parse_filename_mode(reader) {
                    Ok((filename, mode)) => {
                        info!("RRQ: filename='{}', mode='{}'", filename, mode);
                        Command::RRQ {filename, mode}
                    }
                    Err(tftp_error) => {
                        warn!("Failed to parse RRQ packet: {:?}", tftp_error);
                        tftp_error.to_command()
                    }
                }
            },
            Opcode::WRQ => {
                debug!("Processing WRQ packet");
                match parse_filename_mode(reader) {
                    Ok((filename, mode)) => {
                        info!("WRQ: filename='{}', mode='{}'", filename, mode);
                        Command::WRQ{filename, mode}
                    }
                    Err(tftp_error) => {
                        warn!("Failed to parse WRQ packet: {:?}", tftp_error);
                        tftp_error.to_command()
                    }
                }
            },
            Opcode::ACK => {
                match reader.read_u16::<BigEndian>() {
                    Ok(blocknum) => {
                        debug!("ACK block {}", blocknum);
                        Command::ACK{blocknum}
                    }
                    Err(_) => {
                        warn!("Malformed ACK packet");
                        TftpError::MalformedPacket.to_command()
                    }
                }
            },
            Opcode::ERROR => {
                debug!("Processing ERROR packet");
                let errcode = match reader.read_u16::<BigEndian>() {
                    Ok(code) => code,
                    Err(_) => {
                        error!("Malformed ERROR packet - could not read error code");
                        return TftpError::MalformedPacket.to_command()
                    }
                };
                
                let error_msg = match parse_null_terminated_string(reader) {
                    Ok(msg) => msg,
                    Err(_) => {
                        warn!("Could not parse error message from client error {}", errcode);
                        String::new()
                    }
                };
                
                warn!("Client error {}: {}", errcode, error_msg);
                Command::ERROR{errorcode: errcode, errmsg: error_msg}
            }
            Opcode::DATA => {
                debug!("Processing DATA packet");
                let blocknum = match reader.read_u16::<BigEndian>() {
                    Ok(num) => num,
                    Err(_) => {
                        warn!("Malformed DATA packet - could not read block number");
                        return TftpError::MalformedPacket.to_command()
                    }
                };
                
                let mut buf: [u8; 512] = [0;512];
                let n = match reader.read(&mut buf) {
                    Ok(size) => size,
                    Err(_) => {
                        warn!("Malformed DATA packet - could not read data");
                        return TftpError::MalformedPacket.to_command()
                    }
                };
                
                debug!("DATA block {}, size {}", blocknum, n);
                Command::DATA{blocknum, data: buf[0..n].to_vec()}
            },
            _ => {
                warn!("Unknown opcode received");
                TftpError::IllegalOperation.to_command()
            }
        }
    }

    pub fn get_reply_command(context:OpContext) -> Option<Command> {
        match context.current_op {
            Command::RRQ { .. } => {
                return Some(prepare_data_reply(context.filename, 1, context.mode));
            },
            Command::WRQ { .. } => {
                return Some(Command::ACK{blocknum:0});
            },
            Command::ACK {blocknum} => {
                return Some(prepare_data_reply(context.filename, blocknum+1, context.mode));
            },
            Command::DATA{blocknum, data} => {
                return Some(prepare_ack_reply(context.filename, blocknum, context.mode, data));
            },
            _ => {
                println!("Not Implemented");
                return None;
            }
        }
        
    }

    fn prepare_ack_reply(filename: String, blocknum: u16, mode: String, data: Vec<u8>) -> Command {
        debug!("Preparing ACK reply for file '{}', block {}, data size {}, mode {}",
               filename, blocknum, data.len(), mode);
        
        let mut f: File;
        
        // Handle file creation/opening based on block number
        if blocknum == 1 {
            info!("Creating new file: {}", filename);
            match File::create(&filename) {
                Ok(file) => f = file,
                Err(e) => {
                    error!("Failed to create file '{}': {}", filename, e);
                    return TftpError::from_io_error(&e).to_command();
                }
            }
        } else {
            debug!("Opening existing file '{}' for writing", filename);
            match OpenOptions::new().write(true).open(&filename) {
                Ok(file) => f = file,
                Err(e) => {
                    error!("Failed to open file {}: {}", filename, e);
                    return TftpError::from_io_error(&e).to_command();
                }
            }
            
            // Seek to the correct position for this block
            let blknum64 = blocknum as u64;
            if let Err(e) = f.seek(SeekFrom::Start((blknum64 - 1) * 512)) {
                error!("Failed to seek in file {}: {}", filename, e);
                return TftpError::SeekFailed.to_command();
            }
        }
        
        // Write the data to the file
        if let Err(e) = f.write_all(&data) {
            error!("Failed to write data to file {}: {}", filename, e);
            return TftpError::from_write_error(&e).to_command();
        }
        
        // Ensure data is written to disk
        if let Err(e) = f.flush() {
            error!("Failed to flush file {}: {}", filename, e);
            return TftpError::DiskFull.to_command();
        }
        
        info!("Successfully wrote block {} to file '{}'", blocknum, filename);
        Command::ACK { blocknum }
    }

    fn prepare_data_reply(filename: String, blocknum: u16, mode: String) -> Command {
        debug!("Preparing DATA reply for file '{}', block {}, mode {}", filename, blocknum, mode);
        
        let mut f = match File::open(&filename) {
            Ok(file) => {
                debug!("Successfully opened file '{}'", filename);
                file
            },
            Err(e) => {
                error!("Failed to open file '{}': {}", filename, e);
                return TftpError::from_io_error(&e).to_command();
            }
        };
        
        // Seek to the correct position
        let blknum64 = blocknum as u64;
        if let Err(e) = f.seek(SeekFrom::Start((blknum64 - 1) * 512)) {
            error!("Failed to seek in file {}: {}", filename, e);
            return TftpError::SeekFailed.to_command();
        }
    
        // TFTP Protocol define a max size of 512 bytes.
        // First two bytes is the u16 opcode, next two bytes is the block num
        let writer = vec![0; 516];
        let mut cursor_writer = Cursor::new(writer);
        
        // Write opcode (DATA = 3) with error handling
        if let Err(e) = cursor_writer.write_u16::<BigEndian>(3) {
            error!("Failed to write opcode: {}", e);
            return TftpError::InternalError.to_command();
        }
        
        // Write block number with error handling
        if let Err(e) = cursor_writer.write_u16::<BigEndian>(blocknum) {
            error!("Failed to write block number: {}", e);
            return TftpError::InternalError.to_command();
        }
        
        // Read data from file with error handling
        let sz = match f.read(&mut cursor_writer.get_mut()[4..]) {
            Ok(size) => size,
            Err(e) => {
                error!("Failed to read from file {}: {}", filename, e);
                return match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => TftpError::UnexpectedEof.to_command(),
                    std::io::ErrorKind::PermissionDenied => TftpError::AccessViolation.to_command(),
                    _ => TftpError::InternalError.to_command()
                };
            }
        };

        info!("Successfully read {} bytes from file '{}', block {}", sz, filename, blocknum);
        Command::DATA { 
            blocknum, 
            data: cursor_writer.get_ref()[0..sz + 4].to_vec() 
        }
    }

    pub fn get_buffer_for_command(command: Command) -> Option<Vec<u8>> {
        match command {
            Command::DATA {blocknum: _, data} => {
                return Some(data);
            },
            Command::ACK {blocknum} => {
                // u16 to two Big Endian bytes
                let beblocknum = blocknum.to_be_bytes();
                // Todo use enum ACK value and be_bytes
                let result=vec![0,4,beblocknum[0],beblocknum[1]];
                return Some(result);
            },
            Command::ERROR {errorcode, errmsg} => {
                let mut result = Vec::new();
                // Opcode for ERROR (5) in big endian
                result.extend_from_slice(&5u16.to_be_bytes());
                // Error code in big endian
                result.extend_from_slice(&errorcode.to_be_bytes());
                // Error message as bytes
                result.extend_from_slice(errmsg.as_bytes());
                // Null terminator
                result.push(0);
                return Some(result);
            },
            _ => {return None;}
        }
    }

    pub fn recv(buf: &[u8], size: usize, prev_ctx: Option<OpContext>) -> Option<OpContext> {
        let recv_cmd = process_buffer(buf, size);
        match prev_ctx {
            Some(ctx) => {
                // Allow Continuation of RRQ, other cases return None/NO-OP
                match recv_cmd {
                    Command::ACK { blocknum } | Command::DATA { blocknum, data: _ } => {
                        match ctx.current_op {
                            Command::RRQ { .. } | Command::ACK { .. } | Command::WRQ { .. } | Command::DATA { .. } => {
                                debug!("ACK/DATA {} Post RRQ/WRQ", blocknum);
                                let mut new_ctx = ctx;
                                new_ctx.ack_num = blocknum;
                                // TODO Need to only change current op on new base commands WRQ/RRQ
                                new_ctx.current_op = recv_cmd;
                                return Some(new_ctx);
                            }
                            _ => {
                                debug!("Orphan ACK, ignore");
                                return None;
                            }
                        }
                    },
                    Command::ERROR { errorcode, errmsg } => {
                        // Convert client error to TftpError and use consistent handling
                        let client_error = TftpError::from_error_code(errorcode);
                        warn!("{}", client_error.get_client_error_message(&errmsg));
                        
                        // Log the current operation that was aborted
                        TftpError::log_aborted_operation(&ctx.current_op);
                        
                        // Clean termination - return None to end the transfer
                        return None;
                    },
                    // Other commands create new context (RRQ/WRQ)
                    _ => {
                        return build_new_context(recv_cmd);
                    }
                }
            },
            // No Previous operations, create new for required commands, ignore orphans ones
            None => {
                match recv_cmd {
                    Command::ERROR { errorcode, errmsg } => {
                        // Handle orphan errors using same TftpError logic
                        let client_error = TftpError::from_error_code(errorcode);
                        warn!("Received orphan error from client: {}", 
                                 client_error.get_client_error_message(&errmsg));
                        return None;
                    },
                    _ => return build_new_context(recv_cmd),
                }
            }
        }
    }

    pub fn process_buffer(buf: &[u8], _size: usize) -> Command {
        let mut reader = Cursor::new(buf);
        // Todo, handle Errors without panic!
        let opcode = Opcode::try_from(reader.read_u16::<BigEndian>().unwrap()).unwrap();
        return parse_command(opcode, &mut reader);
    }

}

#[cfg(test)]
mod test {
    use crate::tftpprotocol::*;
    use std::matches;
    
    #[test]
    fn recv_rrq() {
        // 0 1 in big endian + Filename + 0 + mode + 0
        let rrq: [u8; 18] = [0, 1, b'f',b'i',b'l',b'e',b'n',b'm',
                             0, b'n',b'e',b't',b'a',b's',b'c',b'i',b'i',0];
        match process_buffer(&rrq,18) {
           Command::RRQ{ filename, mode } => {
              // Got good command, check parsing is OK
              assert_eq!(filename,"filenm");
              assert_eq!(mode,"netascii");
           }
           _ => { panic!("RECV with 0 1 optype must return RRQ command");}
        }
    }

    #[test]
    fn recv_wrq() {
        // 0 2 in big endian + Filename + 0 + mode + 0
        let wrq: [u8; 18] = [0, 2, b'f',b'i',b'l',b'e',b'n',b'm',
                             0, b'n',b'e',b't',b'a',b's',b'c',b'i',b'i',0];
        match process_buffer(&wrq,18) {
           Command::WRQ{ filename, mode } => {
              // Got good command, check parsing is OK
              assert_eq!(filename,"filenm");
              assert_eq!(mode,"netascii");
           }
           _ => { panic!("RECV with 0 2 optype must return WRQ command");}
        }
    }

    #[test]
    fn recv_ack() {
      // 0 4 in big endian + 2 bytes ACK number in Big Endian
      let ack: [u8; 4] = [0, 4, 0xab, 0xcd];
      match process_buffer(&ack,4) {
         Command::ACK{ blocknum } => {
            // Got good command, check parsing is OK
            assert_eq!(blocknum,0xabcd);
         }
         _ => { panic!("ACK with 0 4 + 0xabcd optype must return ACK 0xabcd blocknum");}
      }
     }

     #[test]
     fn recv_error() {
       // 0 5 in big endian + 2 bytes ERROR code in Big Endian + Error message + 0
       let error: [u8; 10] = [0, 5, 0xab, 0xcd, b'a',b'b',b'c',b'd',b'!',0];
       match process_buffer(&error,10) {
          Command::ERROR{ errorcode, errmsg} => {
             // Got good command, check parsing is OK
             assert_eq!(errorcode,0xabcd);
             assert_eq!(errmsg,"abcd!");
          }
          _ => { panic!("ERROR with code abcd +  message \"abcd!\" was not correctly parsed");}
       }
      }

      #[test]
      fn recv_data() {
         // 0 3 in big endian + 2 bytes Block number in Big Endian + Data
         let data: [u8; 9] = [0, 3, 0xab, 0xcd, b'a',b'b',b'c',b'd',b'!'];
         match process_buffer(&data,10) {
            Command::DATA{ blocknum, data} => {
               // Got good command, check parsing is OK
               assert_eq!(blocknum,0xabcd);
               assert_eq!(data,[b'a',b'b',b'c',b'd',b'!']);
            }
            _ => { panic!("DATA with blknum abcd +  data \"abcd!\" was not correctly parsed");}
         }
        }     

    #[test]
    fn recv_invalid() {
       // Invalid Opcode
       let invalid: [u8; 3] = [9,9,9];
       assert!(matches!(process_buffer(&invalid, 3), Command::ERROR{..}));
    }

}