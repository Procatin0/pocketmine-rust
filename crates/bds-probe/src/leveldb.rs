//! Minimal read-only LevelDB reader for worlds written by the Bedrock
//! Dedicated Server.
//!
//! BDS uses Mojang's `leveldb-mcpe` fork. Two things differ from upstream
//! LevelDB and matter for reading:
//!
//! * Block/record payloads may start with a *compressor id* byte
//!   (`2` = zlib, `4` = raw deflate, `0` = uncompressed). Modern worlds use 4.
//! * The journal (`.log`) is a normal LevelDB write-ahead log, so a world that
//!   was stopped right after generation can be read from the log alone.
//!
//! Only point lookups / full scans are supported: we never write.

use flate2::read::{DeflateDecoder, ZlibDecoder};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

/// LevelDB journal block size.
const JOURNAL_BLOCK: usize = 32768;

/// Decompress an mcpe-fork payload. `id` 0 means "stored as-is".
pub fn decompress(id: u8, data: &[u8]) -> Result<Vec<u8>, String> {
    match id {
        0 => Ok(data.to_vec()),
        2 => {
            let mut out = Vec::new();
            ZlibDecoder::new(data)
                .read_to_end(&mut out)
                .map_err(|e| format!("zlib: {e}"))?;
            Ok(out)
        }
        4 => {
            let mut out = Vec::new();
            DeflateDecoder::new(data)
                .read_to_end(&mut out)
                .map_err(|e| format!("deflate: {e}"))?;
            Ok(out)
        }
        other => Err(format!("unsupported compressor id {other}")),
    }
}

fn varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        let byte = *buf.get(*pos)?;
        *pos += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn u32_at(buf: &[u8], pos: usize) -> Option<u32> {
    buf.get(pos..pos + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_at(buf: &[u8], pos: usize) -> Option<u64> {
    buf.get(pos..pos + 8)
        .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

/// One write-ahead-log record: a LevelDB `WriteBatch`.
fn read_journal_records(data: &[u8]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let block = &data[offset..(offset + JOURNAL_BLOCK).min(data.len())];
        offset += JOURNAL_BLOCK;
        let mut pos = 0;
        while pos + 7 <= block.len() {
            let length = u16::from_le_bytes([block[pos + 4], block[pos + 5]]) as usize;
            let kind = block[pos + 6];
            pos += 7;
            if kind == 0 {
                // Zero padding up to the end of the block.
                break;
            }
            if pos + length > block.len() {
                break;
            }
            let fragment = &block[pos..pos + length];
            pos += length;
            match kind {
                1 => records.push(fragment.to_vec()),
                2 => pending = fragment.to_vec(),
                3 => pending.extend_from_slice(fragment),
                4 => {
                    pending.extend_from_slice(fragment);
                    records.push(std::mem::take(&mut pending));
                }
                _ => break,
            }
        }
    }
    records
}

/// Parses a `WriteBatch` into `(sequence, key, Option<value>)`.
///
/// Truncated or corrupt trailing entries are dropped: BDS can leave a
/// half-written batch behind on shutdown and it must not hide the rest.
pub fn batch_entries(record: &[u8]) -> Vec<(u64, Vec<u8>, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let (Some(seq), Some(count)) = (u64_at(record, 0), u32_at(record, 8)) else {
        return out;
    };
    let mut pos = 12usize;
    for _ in 0..count {
        let Some(kind) = record.get(pos).copied() else { break };
        pos += 1;
        let Some(key_len) = varint(record, &mut pos) else { break };
        let key_len = key_len as usize;
        let Some(key) = record.get(pos..pos + key_len) else { break };
        pos += key_len;
        let Some(value_len) = varint(record, &mut pos) else { break };
        let value_len = value_len as usize;
        if kind == 1 {
            let Some(value) = record.get(pos..pos + value_len) else { break };
            pos += value_len;
            out.push((seq, key.to_vec(), Some(value.to_vec())));
        } else {
            out.push((seq, key.to_vec(), None));
        }
    }
    out
}

/// A block read from an SSTable, already decompressed.
fn read_table_block(data: &[u8], offset: usize, size: usize) -> Result<Vec<u8>, String> {
    let raw = data
        .get(offset..offset + size)
        .ok_or_else(|| "table block out of range".to_string())?;
    // mcpe fork: 1 byte compressor id in front of every block.
    let (&id, rest) = raw.split_first().ok_or_else(|| "empty block".to_string())?;
    decompress(id, rest)
}

fn table_entries(data: &[u8]) -> Result<Vec<(u64, Vec<u8>, Option<Vec<u8>>)>, String> {
    let mut out = Vec::new();
    if data.len() < 48 {
        return Ok(out);
    }
    let footer = &data[data.len() - 48..];
    // footer: metaindex handle, index handle, padding, magic (8 bytes)
    let mut pos = 0usize;
    let mut handles = Vec::new();
    for _ in 0..2 {
        let off = u64_at(footer, pos).ok_or("bad footer")? as usize;
        let len = u64_at(footer, pos + 8).ok_or("bad footer")? as usize;
        pos += 16;
        handles.push((off, len));
    }
    let (index_off, index_len) = handles[1];
    let index = read_table_block(data, index_off, index_len)?;
    let mut block_offset = 0usize;
    while block_offset + 4 <= index.len() {
        let Some(off) = varint(&index, &mut block_offset) else { break };
        let Some(size) = varint(&index, &mut block_offset) else { break };
        let Some(_key_len) = varint(&index, &mut block_offset) else { break };
        let Ok(block) = read_table_block(data, off as usize, size as usize) else {
            continue;
        };
        let restarts = match block.len().checked_sub(4) {
            Some(n) => u32_at(&block, n).unwrap_or(0) as usize,
            None => continue,
        };
        let mut end = block.len().saturating_sub(4 + restarts * 4);
        if end > block.len() {
            end = block.len().saturating_sub(4);
        }
        let mut p = 0usize;
        let mut shared_key: Vec<u8> = Vec::new();
        while p < end {
            let Some(shared) = varint(&block, &mut p) else { break };
            let Some(non_shared) = varint(&block, &mut p) else { break };
            let Some(value_len) = varint(&block, &mut p) else { break };
            let (shared, non_shared, value_len) =
                (shared as usize, non_shared as usize, value_len as usize);
            if shared > shared_key.len() || p + non_shared + value_len > block.len() {
                break;
            }
            let mut key = shared_key[..shared].to_vec();
            key.extend_from_slice(&block[p..p + non_shared]);
            p += non_shared;
            let raw_value = &block[p..p + value_len];
            p += value_len;
            // Table values carry an 8-byte sequence/type trailer in upstream
            // leveldb; mcpe compresses per value, so we only unwrap when the
            // value is not valid on its own. Data3D values are self describing,
            // so the caller gets the bytes as stored.
            out.push((0, key.clone(), Some(raw_value.to_vec())));
            shared_key = key;
        }
    }
    Ok(out)
}

/// All key/value pairs of a world database directory.
pub struct Db {
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
    pub journal_records: usize,
    pub table_files: usize,
}

impl Db {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let mut entries: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let mut journal_record_count = 0usize;
        let mut table_files = 0usize;
        let mut paths: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        paths.sort();
        for path in &paths {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let data = match fs::read(path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            match ext {
                "log" => {
                    for record in read_journal_records(&data) {
                        journal_record_count += 1;
                        for (_, key, value) in batch_entries(&record) {
                            match value {
                                Some(v) => {
                                    entries.insert(key, v);
                                }
                                None => {
                                    entries.remove(&key);
                                }
                            }
                        }
                    }
                }
                "ldb" | "sst" => {
                    table_files += 1;
                    for (_, key, value) in table_entries(&data)? {
                        if let Some(v) = value {
                            entries.entry(key).or_insert(v);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            entries,
            journal_records: journal_record_count,
            table_files,
        })
    }

    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.entries.get(key).map(|v| v.as_slice())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.entries.iter().map(|(k, v)| (k.as_slice(), v.as_slice()))
    }
}
