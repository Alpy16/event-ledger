use crate::domain::Event;
use std::io::{Read, Seek, SeekFrom, Write};
use std::{fs::File, fs::OpenOptions, io, path::Path};

#[derive(Debug)]
pub enum LedgerError {
    Io(io::Error),
    Serialization(serde_json::Error),
}

const MAX_PAYLOAD_LEN: u32 = 1024 * 1024; // 1 MiB sanity limit
const HEADER_SIZE: usize = 24;
const MAGIC: [u8; 4] = *b"EVLG";
const VERSION: u32 = 1;

pub struct Ledger {
    file: File,
    next_sequence: u64,
}

struct LedgerHeader {
    magic: [u8; 4],
    version: u32,
    sequence: u64,
    payload_len: u32,
    checksum: u32,
}

fn encode_header(header: &LedgerHeader) -> [u8; HEADER_SIZE] {
    let mut buffer = [0u8; HEADER_SIZE];
    buffer[0..4].copy_from_slice(&header.magic);
    buffer[4..8].copy_from_slice(&header.version.to_le_bytes());
    buffer[8..16].copy_from_slice(&header.sequence.to_le_bytes());
    buffer[16..20].copy_from_slice(&header.payload_len.to_le_bytes());
    buffer[20..24].copy_from_slice(&header.checksum.to_le_bytes());

    buffer
}

fn decode_header(buffer: &[u8; HEADER_SIZE]) -> LedgerHeader {
    let mut magic = [0u8; 4];
    magic.copy_from_slice(&buffer[0..4]);

    let mut version = [0u8; 4];
    version.copy_from_slice(&buffer[4..8]);
    let version = u32::from_le_bytes(version);

    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&buffer[8..16]);
    let sequence = u64::from_le_bytes(sequence);

    let mut payload_len = [0u8; 4];
    payload_len.copy_from_slice(&buffer[16..20]);
    let payload_len = u32::from_le_bytes(payload_len);

    let mut checksum = [0u8; 4];
    checksum.copy_from_slice(&buffer[20..24]);
    let checksum = u32::from_le_bytes(checksum);

    LedgerHeader {
        magic,
        version,
        sequence,
        payload_len,
        checksum,
    }
}
fn encode_event(event: &Event) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(event)
}
impl Ledger {
    pub fn open(path: &Path) -> Result<Ledger, io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;

        let mut next_sequence = 0;

        loop {
            let mut header_bytes = [0u8; HEADER_SIZE];

            // Probe one byte first so we can distinguish:
            // clean EOF vs. a truncated header.
            match file.read(&mut header_bytes[0..1]) {
                Ok(0) => {
                    // No byte available at all:
                    // we reached the end of the ledger cleanly.
                    break;
                }

                Ok(1) => {}

                Ok(_) => unreachable!(),

                Err(error) => return Err(error),
            }

            // We saw the beginning of a record.
            // Therefore the remaining 23 header bytes MUST exist.
            if let Err(error) = file.read_exact(&mut header_bytes[1..HEADER_SIZE]) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("truncated ledger header: {error}"),
                ));
            }

            let header = decode_header(&header_bytes);

            // Are we actually looking at one of our records?
            if header.magic != MAGIC {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid ledger magic",
                ));
            }

            // Do we understand this record format?
            if header.version != VERSION {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported ledger version",
                ));
            }

            // Records should appear in the sequence we expect.
            if header.sequence != next_sequence {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "sequence mismatch: expected {}, found {}",
                        next_sequence, header.sequence
                    ),
                ));
            }

            // Never blindly trust a length read from disk.
            // Corrupt data could otherwise make us try to allocate gigabytes.
            if header.payload_len > MAX_PAYLOAD_LEN {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "payload length exceeds maximum",
                ));
            }

            // The header told us exactly how many payload bytes should follow.
            let mut payload = vec![0u8; header.payload_len as usize];

            // If we cannot get all of them, the final record is truncated.
            if let Err(error) = file.read_exact(&mut payload) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("truncated ledger payload: {error}"),
                ));
            }

            // The checksum was calculated over the PAYLOAD when we wrote it.
            // So recompute CRC over the recovered payload and compare.
            let actual_checksum = crc32fast::hash(&payload);

            if actual_checksum != header.checksum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "payload checksum mismatch",
                ));
            }

            // Make sure these payload bytes actually represent a valid Event.
            // We don't need to keep the Event yet; we're just validating
            // that this record is fully recoverable.
            if let Err(error) = serde_json::from_slice::<Event>(&payload) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid serialized event: {error}"),
                ));
            }

            // This entire record is valid.
            // The next record should therefore have the next sequence number.
            next_sequence += 1;
        }

        Ok(Ledger {
            file,
            next_sequence,
        })
    }

    pub fn append(&mut self, event: &Event) -> Result<(), LedgerError> {
        let payload = match encode_event(event) {
            Ok(payload) => payload,
            Err(error) => return Err(LedgerError::Serialization(error)),
        };

        let payload_len = payload.len() as u32;
        let checksum = crc32fast::hash(&payload);
        let header = LedgerHeader {
            magic: MAGIC,
            version: VERSION,
            sequence: self.next_sequence,
            payload_len,
            checksum,
        };
        let header_bytes = encode_header(&header);
        match self.file.write_all(&header_bytes) {
            Ok(_) => {}
            Err(error) => return Err(LedgerError::Io(error)),
        }
        match self.file.write_all(&payload) {
            Ok(_) => {}
            Err(error) => return Err(LedgerError::Io(error)),
        }

        self.next_sequence += 1;

        Ok(())
    }

    pub fn replay(&mut self) -> Result<Vec<Event>, io::Error> {
        // open() left the read cursor at EOF, so rewind to byte 0.
        self.file.seek(SeekFrom::Start(0))?;

        let mut events = Vec::new();
        let mut expected_sequence = 0;

        loop {
            let mut header_bytes = [0u8; HEADER_SIZE];

            // Probe one byte so clean EOF is distinguishable
            // from a partially written/truncated header.
            match self.file.read(&mut header_bytes[0..1]) {
                Ok(0) => break,
                Ok(1) => {}
                Ok(_) => unreachable!(),
                Err(error) => return Err(error),
            }

            // A record started, so the remaining 23 header bytes must exist.
            if let Err(error) = self.file.read_exact(&mut header_bytes[1..HEADER_SIZE]) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("truncated ledger header: {error}"),
                ));
            }

            let header = decode_header(&header_bytes);

            if header.magic != MAGIC {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid ledger magic",
                ));
            }

            if header.version != VERSION {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported ledger version",
                ));
            }

            if header.sequence != expected_sequence {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "sequence mismatch: expected {}, found {}",
                        expected_sequence, header.sequence
                    ),
                ));
            }

            if header.payload_len > MAX_PAYLOAD_LEN {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "payload length exceeds maximum",
                ));
            }

            let mut payload = vec![0u8; header.payload_len as usize];

            if let Err(error) = self.file.read_exact(&mut payload) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("truncated ledger payload: {error}"),
                ));
            }

            let actual_checksum = crc32fast::hash(&payload);

            if actual_checksum != header.checksum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "payload checksum mismatch",
                ));
            }

            // Unlike open(), replay actually keeps the decoded Event.
            let event = match serde_json::from_slice::<Event>(&payload) {
                Ok(event) => event,
                Err(error) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid serialized event: {error}"),
                    ));
                }
            };

            events.push(event);
            expected_sequence += 1;
        }

        Ok(events)
    }
}
