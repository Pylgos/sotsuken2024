use packed_struct::prelude::*;
use packed_struct::types::bits::ByteArray;
use std::io::{self, Read, Write};
use thiserror::Error;

mod msg;
pub use msg::Message;

const BUF_SIZE: usize = 256;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("buffer overflow")]
    BufferOverflow,
    #[error("invalid message")]
    InvalidMessage,
    #[error("packing error")]
    PackingError(#[from] packed_struct::PackingError),
}

pub struct Bridge<Reader: Read, Writer: Write> {
    reader: io::BufReader<Reader>,
    writer: Writer,
    synced: bool,
}

const CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

impl<Reader: Read, Writer: Write> Bridge<Reader, Writer> {
    pub fn new(reader: Reader, writer: Writer) -> Self {
        Self {
            reader: io::BufReader::with_capacity(BUF_SIZE, reader),
            writer,
            synced: true,
        }
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut encoded_data = [0u8; BUF_SIZE];
        let mut to_encode = [0u8; BUF_SIZE];
        if BUF_SIZE < data.len() + 2 {
            return Err(Error::BufferOverflow);
        }
        let crc = CRC.checksum(&data);
        to_encode[..data.len()].copy_from_slice(data);
        to_encode[data.len()] = (crc & 0xff) as u8;
        to_encode[data.len() + 1] = (crc >> 8) as u8;
        let encoded_len = cobs::try_encode(&to_encode[..data.len() + 2], &mut encoded_data)
            .map_err(|_| Error::BufferOverflow)?;
        self.writer.write_all(&encoded_data[..encoded_len])?;
        self.writer.write(&[0])?;
        Ok(())
    }

    fn read_bytes(&mut self, dest: &mut [u8]) -> Result<usize, Error> {
        if !self.synced {
            loop {
                let mut b = [0u8; 1];
                self.reader.read_exact(&mut b)?;
                if b[0] == 0 {
                    self.synced = true;
                    break;
                }
            }
        }
        let mut read_buf = [0u8; BUF_SIZE];
        let mut read_len = 0;
        loop {
            if read_len >= read_buf.len() {
                return Err(Error::BufferOverflow);
            }
            let mut b = [0u8; 1];
            self.reader.read_exact(&mut b)?;
            read_buf[read_len] = b[0];
            read_len += 1;
            if b[0] == 0 {
                break;
            }
        }
        let mut decode_buf = [0u8; BUF_SIZE];
        let decoded_len = cobs::decode(&read_buf[..read_len], &mut decode_buf)
            .map_err(|_| Error::InvalidMessage)?;
        if decoded_len < 2 {
            return Err(Error::InvalidMessage);
        }
        let crc_received =
            decode_buf[decoded_len - 2] as u16 | (decode_buf[decoded_len - 1] as u16) << 8;
        let crc_computed = CRC.checksum(&decode_buf[..decoded_len - 2]);
        if crc_computed != crc_received {
            return Err(Error::InvalidMessage);
        }
        dest[0..decoded_len - 2].copy_from_slice(&decode_buf[0..decoded_len - 2]);
        Ok(decoded_len - 2)
    }

    pub fn write(&mut self, msg: &Message) -> Result<(), Error> {
        let packed = msg.pack()?;
        self.write_bytes(packed.as_bytes_slice())?;
        Ok(())
    }

    pub fn read(&mut self) -> Result<Message, Error> {
        let mut buf = <Message as PackedStruct>::ByteArray::new(0);
        self.read_bytes(buf.as_mut_bytes_slice())?;
        Ok(Message::unpack(&buf)?)
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    use crate::{Bridge, Message};

    #[derive(Debug, Clone)]
    struct TestBuffer {
        buf: Rc<RefCell<VecDeque<u8>>>,
    }

    impl TestBuffer {
        fn new<T: Into<VecDeque<u8>>>(val: T) -> Self {
            Self {
                buf: Rc::new(RefCell::new(val.into())),
            }
        }
    }

    impl std::io::Read for TestBuffer {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut b = self.buf.borrow_mut();
            let mut n_read = 0;
            for byte in buf.iter_mut() {
                match b.pop_back() {
                    Some(b) => *byte = b,
                    None => break,
                }
                n_read += 1;
            }
            Ok(n_read)
        }
    }

    impl std::io::Write for TestBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut b = self.buf.borrow_mut();
            for byte in buf.iter() {
                b.push_front(*byte);
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.buf.borrow_mut().flush()
        }
    }

    #[test]
    fn test() {
        let buf = TestBuffer::new([]);
        let mut bridge = Bridge::new(buf.clone(), buf.clone());
        {
            let send = Message {
                forward_vel: 100,
                turn_vel: 100,
            };
            bridge.write(&send).unwrap();
            let recv = bridge.read().unwrap();
            assert_eq!(send, recv);
        }
    }
}
