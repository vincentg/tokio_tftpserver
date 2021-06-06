pub mod protocol {
   enum Opcodes {
       RRQ = 1, // Read request
       WRQ = 2, // Write request
       DATA = 3,
       ACK = 4,
       ERROR = 5
   }


   enum Arguments {
      RRQ  {filename : String, mode:String},
      WRQ  {filename : String, mode:String},
      DATA {blocknum : u16, data:Vec<u8>},
      ACK  {blocknum : u16},
      ERROR {errorcode :u16, errmsg:String}
   }

   pub fn recv(mut buf: Vec<u8>) {}
   
}
