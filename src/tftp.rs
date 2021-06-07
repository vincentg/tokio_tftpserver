pub mod TFTPProtocol {
   use std::io::Cursor;
   use byteorder::{BigEndian};
   use byteorder::ReadBytesExt;
   use std::convert::TryFrom;


   enum Opcode {
       RRQ, // Read request
       WRQ, // Write request
       DATA,
       ACK,
       ERROR
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

   enum Command {
      RRQ  {filename : String, mode:String},
      WRQ  {filename : String, mode:String},
      DATA {blocknum : u16, data:Vec<u8>},
      ACK  {blocknum : u16},
      ERROR {errorcode :u16, errmsg:String}
   }

   fn parse_command(opcode: Opcode, reader: Cursor<&[u8]>) -> Command {
      match opcode {
         Opcode::RRQ => {
             println!("Read");
             return Command::RRQ {filename:"XXX".to_string(),mode:"YYY".to_string()};
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

   pub fn recv(buf: &[u8], size: usize) {
      let mut reader = Cursor::new(buf);
      // Todo, handle Errors without panic!
      let opcode = Opcode::try_from(reader.read_u16::<BigEndian>().unwrap()).unwrap();
      let command = parse_command(opcode, reader);
   }




   
}
