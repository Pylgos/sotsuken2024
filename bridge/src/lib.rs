// これ使えばよくね？？？？？ https://github.com/LukaOber/serialmessage-rs

use std::io::{BufRead, Write};
use thiserror::Error;

const DEFAULT_BUF_SIZE: usize = 256;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("buffer overflow")]
    BufferOverflow,
    #[error("invalid message")]
    InvalidMessage,
}

pub struct Bridge<Reader, Writer> {
    reader: Reader,
    writer: Writer,
    synced: bool,
}

const CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

impl<R: BufRead, W: Write> Bridge<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            synced: true,
        }
    }

    pub fn send_with_custom_buffer(
        &mut self,
        data: &[u8],
        buf_a: &mut [u8],
        buf_b: &mut [u8],
    ) -> Result<(), Error> {
        let encoded_data = buf_a;
        let to_encode = buf_b;
        if encoded_data.len().min(to_encode.len()) < data.len() + 2 {
            return Err(Error::BufferOverflow);
        }
        let crc = CRC.checksum(data);
        to_encode[..data.len()].copy_from_slice(data);
        to_encode[data.len()] = (crc & 0xff) as u8;
        to_encode[data.len() + 1] = (crc >> 8) as u8;
        let encoded_len = cobs::try_encode(&to_encode[..data.len() + 2], encoded_data)
            .map_err(|_| Error::BufferOverflow)?;
        self.writer.write_all(&encoded_data[..encoded_len])?;
        self.writer.write_all(&[0])?;
        Ok(())
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut buf_a = [0u8; DEFAULT_BUF_SIZE];
        let mut buf_b = [0u8; DEFAULT_BUF_SIZE];
        self.send_with_custom_buffer(data, &mut buf_a, &mut buf_b)
    }

    pub fn recv_with_custom_buffer(
        &mut self,
        dest: &mut [u8],
        buf_a: &mut [u8],
        buf_b: &mut [u8],
    ) -> Result<usize, Error> {
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
        let read_buf = buf_a;
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
        let decode_buf = buf_b;
        let decoded_len =
            cobs::decode(&read_buf[..read_len], decode_buf).map_err(|_| Error::InvalidMessage)?;
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

    pub fn recv(&mut self, dest: &mut [u8]) -> Result<usize, Error> {
        let mut buf_a = [0u8; DEFAULT_BUF_SIZE];
        let mut buf_b = [0u8; DEFAULT_BUF_SIZE];
        self.recv_with_custom_buffer(dest, &mut buf_a, &mut buf_b)
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, collections::VecDeque, io, rc::Rc};

    use super::*;
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Message {
        pub forward_vel: f32,
        pub turn_vel: f32,
    }

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
        let mut bridge = Bridge::new(io::BufReader::new(buf.clone()), buf.clone());
        {
            let msg = Message {
                forward_vel: 100.0,
                turn_vel: 100.0,
            };
            let serialized = bincode::serialize(&msg).unwrap();
            bridge.send(&serialized).unwrap();
            let mut buf = [0u8; 256];
            let recv_len = bridge.recv(&mut buf).unwrap();
            let deserialized: Message = bincode::deserialize(&buf[..recv_len]).unwrap();
            assert_eq!(msg, deserialized);
        }
    }
}
