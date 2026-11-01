//! Random access to EPWING data files, plain or EBZip-compressed (`.ebz`).
//!
//! EBZip splits a file into fixed-size slices (2048 << level bytes), each
//! zlib-compressed on its own, with a table of slice offsets after a 22-byte
//! header. We keep a small cache of decompressed slices.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

pub const PAGE: u64 = 2048;

enum Kind {
    Plain { size: u64 },
    Ebzip { size: u64, slice_size: u64, width: usize, slices: u64 },
}

pub struct Zio {
    file: Mutex<File>,
    kind: Kind,
    cache: Mutex<HashMap<u64, std::sync::Arc<Vec<u8>>>>,
}

impl Zio {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut head = [0u8; 22];
        let n = file.read(&mut head)?;
        let kind = if n == 22 && &head[..5] == b"EBZip" {
            let mode = head[5] >> 4;
            let level = (head[5] & 0x0f) as u32;
            if mode != 1 && mode != 2 {
                return Err(Error::Format(format!("unsupported EBZip mode {mode}")));
            }
            // Bytes 9..14 hold the original size (big-endian, 5 bytes for files over 4 GB).
            let size = u64::from_be_bytes([0, 0, 0, head[9], head[10], head[11], head[12], head[13]]);
            let slice_size = PAGE << level;
            let slices = size.div_ceil(slice_size);
            let width = if size < 1 << 16 {
                2
            } else if size < 1 << 24 {
                3
            } else if size < 1 << 32 {
                4
            } else {
                5
            };
            Kind::Ebzip { size, slice_size, width, slices }
        } else {
            Kind::Plain { size: file.metadata()?.len() }
        };
        Ok(Self { file: Mutex::new(file), kind, cache: Mutex::new(HashMap::new()) })
    }

    pub fn size(&self) -> u64 {
        match self.kind {
            Kind::Plain { size } | Kind::Ebzip { size, .. } => size,
        }
    }

    /// Read `len` bytes at `offset` (short reads at end of file are zero-padded).
    pub fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut out = vec![0u8; len];
        match self.kind {
            Kind::Plain { size } => {
                if offset < size {
                    let mut f = self.file.lock().unwrap();
                    f.seek(SeekFrom::Start(offset))?;
                    let want = len.min((size - offset) as usize);
                    f.read_exact(&mut out[..want])?;
                }
            }
            Kind::Ebzip { size, slice_size, .. } => {
                let mut done = 0usize;
                while done < len {
                    let pos = offset + done as u64;
                    if pos >= size {
                        break;
                    }
                    let idx = pos / slice_size;
                    let within = (pos % slice_size) as usize;
                    let slice = self.slice(idx)?;
                    let take = (slice.len().saturating_sub(within)).min(len - done);
                    if take == 0 {
                        break;
                    }
                    out[done..done + take].copy_from_slice(&slice[within..within + take]);
                    done += take;
                }
            }
        }
        Ok(out)
    }

    pub fn read_page(&self, page: u32) -> Result<Vec<u8>> {
        self.read_at((page.max(1) as u64 - 1) * PAGE, PAGE as usize)
    }

    fn slice(&self, idx: u64) -> Result<std::sync::Arc<Vec<u8>>> {
        if let Some(s) = self.cache.lock().unwrap().get(&idx) {
            return Ok(s.clone());
        }
        let Kind::Ebzip { slice_size, width, slices, .. } = self.kind else { unreachable!() };
        if idx >= slices {
            return Ok(std::sync::Arc::new(vec![0; slice_size as usize]));
        }
        let data = {
            let mut f = self.file.lock().unwrap();
            let mut ix = vec![0u8; width * 2];
            f.seek(SeekFrom::Start(22 + idx * width as u64))?;
            f.read_exact(&mut ix)?;
            let be = |b: &[u8]| b.iter().fold(0u64, |a, &x| (a << 8) | x as u64);
            let start = be(&ix[..width]);
            let end = be(&ix[width..]);
            if end < start {
                return Err(Error::Format("corrupt EBZip index".into()));
            }
            let mut raw = vec![0u8; (end - start) as usize];
            f.seek(SeekFrom::Start(start))?;
            f.read_exact(&mut raw)?;
            if raw.len() as u64 == slice_size {
                raw
            } else {
                let mut dec = Vec::with_capacity(slice_size as usize);
                flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut dec)?;
                dec.resize(slice_size as usize, 0);
                dec
            }
        };
        let arc = std::sync::Arc::new(data);
        let mut cache = self.cache.lock().unwrap();
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(idx, arc.clone());
        Ok(arc)
    }
}

/// Find a file or directory inside `dir` ignoring case, optionally with an `.ebz`
/// suffix or a `;1` ISO-9660 version suffix.
pub fn find_ci(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    let want = name.to_ascii_lowercase();
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best = None;
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        let n = n.trim_end_matches(";1").trim_end_matches('.').to_string();
        if n == want {
            return Some(e.path());
        }
        if n == format!("{want}.ebz") || n == format!("{want}.org") {
            best = Some(e.path());
        }
    }
    best
}
