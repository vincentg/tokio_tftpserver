pub mod tftpprotocol {
   use std::io::Cursor;
   use std::io::BufRead;
   use std::io::Read;
   use byteorder::{BigEndian};
   use byteorder::{ReadBytesExt,WriteBytesExt};
   use std::convert::TryFrom;
   use std::fs::File;
   use std::io::Seek;
   use std::io::SeekFrom;


   
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
      pub reply_to_send : Option<Command>,
      block_num : u16,       // For RRQ last read block, for WRQ, last written
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
               reply_to_send : None,
               block_num:0,
               ack_num:0,
               filename:filename.clone(),
               mode:mode.clone()
            }),
         _ => return None
      }     
   }


   fn parse_command(opcode: Opcode, reader: &mut Cursor<&[u8]>) -> Command {

      // Inner function for RRQ/WRQ shared parsing logic 
      fn parse_filename_mode(reader: &mut Cursor<&[u8]>) -> (String,String) {
         let mut buffer: Vec<u8> = Vec::new();
         reader.read_until(0, &mut buffer).unwrap();
         // Remove delimiter (\0)
         buffer.pop();
         // Todo Manage Error
         let filename = String::from_utf8(buffer).unwrap();
         // First buffer was moved above, create buffer for Mode
         let mut _mode_buf: Vec<u8> = Vec::new();
         reader.read_until(0, &mut _mode_buf).unwrap();
         _mode_buf.pop();
         let mode = String::from_utf8(_mode_buf).unwrap();
   
         return (filename, mode);
      }

      match opcode {
         Opcode::RRQ => {
             println!("Read");
             let (filename, mode) = parse_filename_mode(reader);
             println!("FileName: {}, Mode: {}",filename, mode);
             return Command::RRQ {filename: filename, mode: mode};
         },
         Opcode::WRQ => {
            println!("Write");
            let (filename, mode) = parse_filename_mode(reader);
            println!("FileName: {}, Mode: {}",filename, mode);
            return Command::WRQ{filename, mode};
         },
         Opcode::ACK => {
            let blocknum = reader.read_u16::<BigEndian>().unwrap();
            println!("ACK {}",blocknum);
            return Command::ACK{blocknum};
         },
         Opcode::ERROR => {
            println!("ERROR");
            let errcode = reader.read_u16::<BigEndian>().unwrap();
            let mut buffer: Vec<u8> = Vec::new();
            let _error_read = reader.read_until(0, &mut buffer).unwrap();
            buffer.pop();
            // Todo Manage Error
            let error = String::from_utf8(buffer).unwrap();
            return Command::ERROR{errorcode:errcode, errmsg: error};
         }

         _ => {
            println!("Other Opcode");
            return Command::ERROR{errorcode :1, errmsg:"NOT IMPLEMENTED".to_string()};
         }
            
      }

   }

   pub fn get_reply_command(context:OpContext) -> Option<Command> {
      match context.current_op {
         Command::RRQ { .. } => {
            return Some(prepare_data(context.filename, 1, context.mode));
         },
         Command::ACK {blocknum} => {
            return Some(prepare_data(context.filename, blocknum+1, context.mode));
         }
         _ => {
            println!("Not Implemented");
            return None;
         }
      }
      
   }

   fn prepare_data(filename :String, blocknum: u16, mode: String) -> Command {
      // Todo manage error
      println!("OPENING FILE: FileName: {} (len:{}), Mode: {}(len:{}), block:{} ",filename,filename.len(), mode, mode.len(), blocknum);
      let mut f = File::open(filename).unwrap();
      let blknum64 = blocknum as u64;
      f.seek(SeekFrom::Start((blknum64-1)*512)).unwrap();
      // TFTP Protocol define a max size of 512 bytes.
      // First two bytes is the u16 chuck num
      let writer = vec![0;516];
      let mut cursor_writer = Cursor::new(writer);
      // TODO SEE HOW TO DERIVE 3 from Opnum::DATA
      cursor_writer.write_u16::<BigEndian>(3).unwrap();
      cursor_writer.write_u16::<BigEndian>(blocknum).unwrap();
      // Todo manage error 
      // Todo SPEC GAP read from BlockNum*512
      //let sz = f.read(&mut writer[4..]).unwrap();
      let sz = f.read(&mut cursor_writer.get_mut()[4..]).unwrap();
      // Check sz
      println!("READ SZ: {}", sz);

      return Command::DATA{blocknum: blocknum, data: cursor_writer.get_ref()[0..sz+4].to_vec()}
   }

   pub fn get_buffer_for_command(command: Command) -> Option<Vec<u8>> {
      match command {
         Command::DATA {blocknum: _blknum, data: _data} => {
            return Some(_data);
         },    
         _ => {return None;}
      }
   }

   pub fn recv(buf: &[u8], size: usize, prev_ctx: Option<OpContext>) -> Option<OpContext> {
      let recv_cmd = process_buffer(buf,size);
      match prev_ctx{
         Some(ctx) => {
            // Allow Continuation of RRQ, other cases return None/NO-OP
            match recv_cmd {
               Command::ACK{ blocknum: blknum } => {
                  match ctx.current_op {
                     Command::RRQ{..} | Command::ACK{..} => {
                        print!("ACK {} Post RRQ", blknum);
                        let mut new_ctx = ctx;
                        new_ctx.ack_num = blknum;
                        new_ctx.current_op = recv_cmd;
                        return Some(new_ctx);
                     }
                     _ => {print!("Orphan ACK, ignore"); return None;}
                  }
               },
               Command::DATA{..} => {print!("TODO/Implement WRQ"); return None;},
               Command::ERROR{errorcode, errmsg} => {
                  eprint!("Aborting command, received from client error {} with message {}",errorcode,errmsg);
                  return None;
               },
               // Other commands create new context (RRQ/WRQ)
               _ => {return build_new_context(recv_cmd);}
            }
         },
         // No Previous operations, create new for required commands, ignore orphans ones
         None => return build_new_context(recv_cmd)
      }
   }
      
   pub fn process_buffer(buf: &[u8], size: usize) -> Command {
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
           Command::RRQ{ filename: filenm, mode: _mode } => {
              // Got good command, check parsing is OK
              assert_eq!(filenm,"filenm");
              assert_eq!(_mode,"netascii");
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
           Command::WRQ{ filename: filenm, mode: _mode } => {
              // Got good command, check parsing is OK
              assert_eq!(filenm,"filenm");
              assert_eq!(_mode,"netascii");
           }
           _ => { panic!("RECV with 0 2 optype must return WRQ command");}
        }
    }

    #[test]
    fn recv_ack() {
      // 0 4 in big endian + 2 bytes ACK number in Big Endian
      let ack: [u8; 4] = [0, 4, 0xab, 0xcd];
      match process_buffer(&ack,4) {
         Command::ACK{ blocknum: blknum } => {
            // Got good command, check parsing is OK
            assert_eq!(blknum,0xabcd);
         }
         _ => { panic!("ACK with 0 4 + 0xabcd optype must return ACK 0xabcd blocknum");}
      }
     }

     #[test]
     fn recv_error() {
       // 0 4 in big endian + 2 bytes ACK number in Big Endian
       let error: [u8; 10] = [0, 5, 0xab, 0xcd, b'a',b'b',b'c',b'd',b'!',0];
       match process_buffer(&error,10) {
          Command::ERROR{ errorcode: code, errmsg: msg} => {
             // Got good command, check parsing is OK
             assert_eq!(code,0xabcd);
             assert_eq!(msg,"abcd!");
          }
          _ => { panic!("ERROR with code abcd +  message \"abcd!\" was not correctly parsed");}
       }
      }     

    #[test]
    fn recv_invalid() {
       // Invalid Opcode
       let invalid: [u8; 3] = [9,9,9];
       assert!(matches!(process_buffer(&invalid, 3), Command::ERROR{..}));
    }

}

