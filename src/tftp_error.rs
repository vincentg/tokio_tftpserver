use crate::tftp::tftpprotocol::Command;

// TFTP Error codes enum for better error handling
#[derive(Debug, Clone)]
pub enum TftpError {
    NotDefined(String),          // 0 - Custom error message  
    FileNotFound,                // 1
    AccessViolation,             // 2
    DiskFull,                    // 3
    IllegalOperation,            // 4
    UnknownTransferId,           // 5
    FileAlreadyExists,           // 6
    NoSuchUser,                  // 7
    // Variants that map to standard codes
    SeekFailed,                  // -> 2 Access violation
    UnexpectedEof,               // -> 2 Access violation
    InternalError,               // -> 2 Access violation
    MalformedPacket,             // -> 4 Illegal operation
}

impl TftpError {
    // Get the TFTP error code for this error
    pub fn error_code(&self) -> u16 {
        match self {
            TftpError::NotDefined(_) => 0,
            TftpError::FileNotFound => 1,
            TftpError::AccessViolation => 2,
            TftpError::DiskFull => 3,
            TftpError::IllegalOperation => 4,
            TftpError::UnknownTransferId => 5,
            TftpError::FileAlreadyExists => 6,
            TftpError::NoSuchUser => 7,
            // Variants that map to standard codes
            TftpError::SeekFailed => 2,
            TftpError::UnexpectedEof => 2,
            TftpError::InternalError => 2,
            TftpError::MalformedPacket => 4,
        }
    }

    // Get the default error message for this error type
    pub fn default_message(&self) -> String {
        match self {
            TftpError::NotDefined(msg) => if msg.is_empty() { "Not defined".to_string() } else { msg.clone() },
            TftpError::FileNotFound => "File not found".to_string(),
            TftpError::AccessViolation => "Access violation".to_string(),
            TftpError::DiskFull => "Disk full or allocation exceeded".to_string(),
            TftpError::IllegalOperation => "Illegal TFTP operation".to_string(),
            TftpError::UnknownTransferId => "Unknown transfer ID".to_string(),
            TftpError::FileAlreadyExists => "File already exists".to_string(),
            TftpError::NoSuchUser => "No such user".to_string(),
            TftpError::SeekFailed => "Access violation - seek failed".to_string(),
            TftpError::UnexpectedEof => "Access violation - unexpected EOF".to_string(),
            TftpError::InternalError => "Internal error".to_string(),
            TftpError::MalformedPacket => "Illegal TFTP operation - malformed packet".to_string(),
        }
    }

    // Convert to Command::ERROR for sending
    pub fn to_command(&self) -> Command {
        Command::ERROR { 
            errorcode: self.error_code(), 
            errmsg: self.default_message() 
        }
    }

    // Convert received client error code to TftpError for consistent handling
    pub fn from_error_code(errorcode: u16) -> TftpError {
        match errorcode {
            0 => TftpError::NotDefined("".to_string()), // Will use custom message if provided
            1 => TftpError::FileNotFound,
            2 => TftpError::AccessViolation, 
            3 => TftpError::DiskFull,
            4 => TftpError::IllegalOperation,
            5 => TftpError::UnknownTransferId,
            6 => TftpError::FileAlreadyExists,
            7 => TftpError::NoSuchUser,
            _ => TftpError::NotDefined(format!("Unknown error code {}", errorcode)),
        }
    }

    // Helper function to convert io::Error to appropriate TftpError
    pub fn from_io_error(error: &std::io::Error) -> TftpError {
        match error.kind() {
            std::io::ErrorKind::NotFound => TftpError::FileNotFound,
            std::io::ErrorKind::PermissionDenied => TftpError::AccessViolation,
            std::io::ErrorKind::WriteZero | std::io::ErrorKind::UnexpectedEof => TftpError::DiskFull,
            std::io::ErrorKind::AlreadyExists => TftpError::FileAlreadyExists,
            _ => TftpError::InternalError,
        }
    }

    // Helper for write-specific errors
    pub fn from_write_error(error: &std::io::Error) -> TftpError {
        match error.kind() {
            std::io::ErrorKind::WriteZero | std::io::ErrorKind::UnexpectedEof => TftpError::DiskFull,
            std::io::ErrorKind::PermissionDenied => TftpError::AccessViolation,
            _ => TftpError::InternalError,
        }
    }

    // Get descriptive message for client errors (for logging)
    pub fn get_client_error_message(&self, custom_msg: &str) -> String {
        let base_message = format!("Client reports: {}", self.default_message());
        
        if custom_msg.is_empty() {
            base_message
        } else {
            format!("{} - {}", base_message, custom_msg)
        }
    }

    // Helper to log current operation being aborted
    pub fn log_aborted_operation(current_op: &Command) {
        match current_op {
            Command::RRQ { filename, .. } => {
                eprintln!("Aborting read request for file: {}", filename);
            },
            Command::WRQ { filename, .. } => {
                eprintln!("Aborting write request for file: {}", filename);
            },
            Command::DATA { blocknum, .. } => {
                eprintln!("Aborting data transfer at block: {}", blocknum);
            },
            Command::ACK { blocknum } => {
                eprintln!("Aborting transfer after ACK block: {}", blocknum);
            },
            _ => eprintln!("Aborting unknown operation"),
        }
    }
}