pub mod tftpprotocol {
   use std::io::Cursor;
   use std::io::BufRead;
   use std::io::Read;
   use byteorder::{BigEndian};
   use byteorder::{ReadBytesExt,WriteBytesExt};
   use std::convert::TryFrom;
   use std::fs::File;

   
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

   pub enum Command {
      RRQ  {filename : String, mode:String},
      WRQ  {filename : String, mode:String},
      DATA {blocknum : u16, data:Vec<u8>},
      ACK  {blocknum : u16},
      ERROR {errorcode :u16, errmsg:String}
   }

   struct OpContext {
      current_op : Command,  // RRQ or WRQ
      block_num : u16,       // For RRQ last read block, for WRQ, last written
      ack_num   : u16,       // last ACK received (to detect timeout)
      openfile : Option<std::fs::File> // keep file descriptor/open_file
   }


   fn parse_filename_mode(reader: &mut Cursor<&[u8]>) -> (String,String) {
      let mut _buffer = vec![0; 1024];
      let _file_read = reader.read_until(0, &mut _buffer).unwrap();
      // !! TODO See why resulting vector have zeros bytes !!
      _buffer.retain(|&x| x != 0);
      // Todo Manage Error
      let _filename = String::from_utf8(_buffer).unwrap();
      // First buffer was moved above, create buffer for Mode
      let mut _mode_buf = vec![0; 254];
      let _mode_read = reader.read_until(0, &mut _mode_buf).unwrap();
      // !! TODO See why resulting vector have zeros bytes !!
      _mode_buf.retain(|&x| x != 0);
      let _mode = String::from_utf8(_mode_buf).unwrap();

      return (_filename,_mode);
   }

   fn parse_command(opcode: Opcode, reader: &mut Cursor<&[u8]>) -> Command {
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
            return Command::WRQ{filename: filename, mode: mode};
         }
         _ => {
            println!("Other Opcode");
            return Command::ERROR{errorcode :1, errmsg:"NOT IMPLEMENTED".to_string()};
         }
            
      }

   }

   pub fn get_reply_command(command: Command, filename:Option<String>) -> Option<Command> {
      match command {
         Command::RRQ { filename: _filename, mode: _mode } => {
            return Some(prepare_data(_filename, 1, _mode));            
         },
         _ => {
            println!("Not Implemented");
            return None;
         }
      }
      
   }

   fn prepare_data(filename :String, blocknum: u16, mode: String) -> Command {
      // Todo manage error
      println!("OPENING FILE: FileName: {} (len:{}), Mode: {}(len:{}), ",filename,filename.len(), mode, mode.len());
      let mut f = File::open(filename).unwrap();
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
      
   pub fn recv(buf: &[u8], size: usize) -> Command {
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
        match recv(&rrq,18) {
           Command::RRQ{ filename: _filename, mode: _mode } => {
              // Got good command, check parsing is OK
              assert_eq!(_filename,"filenm");
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
        match recv(&wrq,18) {
           Command::WRQ{ filename: _filename, mode: _mode } => {
              // Got good command, check parsing is OK
              assert_eq!(_filename,"filenm");
              assert_eq!(_mode,"netascii");
           }
           _ => { panic!("RECV with 0 2 optype must return WRQ command");}
        }
    }

    #[test]
    fn recv_invalid() {
       // Invalid Opcode
       let invalid: [u8; 3] = [9,9,9];
       assert!(matches!(recv(&invalid, 3), Command::ERROR{..}));
    }

}

