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
       ERROR = 5
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
            _ => Err("Unknown Opcode received from client")
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

   fn parse_command(opcode: Opcode, reader: &mut Cursor<&[u8]>) -> Command {
      match opcode {
         Opcode::RRQ => {
             println!("Read");
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
             println!("FileName: {}, Mode: {}",_filename, _mode);
             return Command::RRQ {filename: _filename, mode: _mode};
         },
         Opcode::WRQ => {
            println!("Write");
            return Command::WRQ{filename: "XXX".to_string(), mode:"YYY".to_string()};
         }
         _ => {
            println!("Other Opcode");
            return Command::ERROR{errorcode :1, errmsg:"NOT IMPLEMENTED".to_string()};
         }
            
      }

   }

   pub fn getReplyCommand(command: Command, filename:Option<String>) -> Option<Command> {
      match command {
         Command::RRQ { filename: _filename, mode: _mode } => {
            return Some(PrepareData(_filename, 1, _mode));            
         },
         _ => {
            println!("Not Implemented");
            return None;
         }
      }
      
   }

   fn PrepareData(filename :String, blocknum: u16, mode: String) -> Command {
      // Todo manage error
      println!("OPENING FILE: FileName: {} (len:{}), Mode: {}(len:{}), ",filename,filename.len(), mode, mode.len());
      let mut f = File::open(filename).unwrap();
      // TFTP Protocol define a max size of 512 bytes.
      // First two bytes is the u16 chuck num
      let mut writer = vec![0;516];
      let mut cursorWriter = Cursor::new(writer);
      // TODO SEE HOW TO DERIVE 3 from Opnum::DATA
      cursorWriter.write_u16::<BigEndian>(3).unwrap();
      cursorWriter.write_u16::<BigEndian>(blocknum).unwrap();
      // Todo manage error 
      // Todo SPEC GAP read from BlockNum*512
      //let sz = f.read(&mut writer[4..]).unwrap();
      let sz = f.read(&mut cursorWriter.get_mut()[4..]).unwrap();
      // Check sz

      return Command::DATA{blocknum: blocknum, data: cursorWriter.get_ref()[0..sz+4].to_vec()}
   }

   pub fn getBufferForCommand(command: Command) -> Option<Vec<u8>> {
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
    
    #[test]
    fn recv_rrq() {
			// 0 1 in big endian + Filename + 0 + mode + 0
        let rrq: [u8; 18] = [0, 1, b'f',b'i',b'l',b'e',b'n',b'm',
                             0, b'n',b'e',b't',b'a',b's',b'c',b'i',b'i',0];
        match recv(&rrq,18) {
           Command::RRQ{ filename: _filename, mode: _mode } => { 
              assert_eq!(_filename,"filenm");
              assert_eq!(_mode,"netascii");
           }
           _ => { panic!("RECV with 0 1 optype must return RRQ command");}
        }
    }

}

