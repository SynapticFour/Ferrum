// SPDX-License-Identifier: Apache-2.0
//! Record-exact body URLs for BGZF formats (BAM, VCF.gz, BCF).
//!
//! htsget-rs builds body ranges at BGZF block boundaries. That is only a valid file when every
//! block starts and ends on a record boundary. htslib writes BAM that way, and bgzip/bcftools
//! write VCF.gz that way while lines are shorter than one block, but **BCF records routinely
//! span blocks** (htslib fills BCF blocks to 65,280 bytes), and so do BAM records or VCF lines
//! longer than one block (ultra-long reads, VCFs with many samples). A block-aligned ticket then
//! starts or ends inside a record and the client cannot parse it.
//!
//! The index chunks carry virtual positions that are always record boundaries. This module
//! turns the chunks of the requested regions into ticket URLs:
//!
//! * a chunk start inside a block: that block is decompressed, cut at the start offset and
//!   re-compressed into a new BGZF block served as a `data:` URL;
//! * whole blocks in between: an HTTP `Range` on the DRS stream (unchanged bytes);
//! * a chunk end inside a block: the block is cut at the end offset, re-compressed, `data:` URL.
//!
//! Only plain objects: a Crypt4GH object's blocks cannot be cut without decrypting them for the
//! client (handled by htsget-rs, see `region.rs`).

use base64::Engine;
use ferrum_storage::ObjectStorage;
use htsget_config::types::{Class, HtsGetError, Url};
use noodles::bgzf::VirtualPosition;
use noodles::core::region::Interval;
use noodles::core::Position;
use noodles::csi::binning_index::index::reference_sequence::bin::Chunk;
use noodles::csi::BinningIndex;
use std::io::{Read, Write};
use tokio::io::AsyncReadExt;

/// Largest BGZF block (BSIZE is a u16 of size − 1).
const MAX_BLOCK: u64 = 65_536;

/// Format of the data file, as far as this module cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgzfFormat {
    Bam,
    Vcf,
    Bcf,
}

/// Parsed index plus the reference-name → index-id mapping it needs.
pub struct Located {
    index: Box<dyn BinningIndex + Send + Sync>,
    names: Vec<String>,
    /// Virtual position of the first record = end of the header.
    header_end: VirtualPosition,
}

fn internal(msg: impl std::fmt::Display) -> HtsGetError {
    HtsGetError::InternalError(msg.to_string())
}

async fn read_all(storage: &dyn ObjectStorage, key: &str) -> Result<Vec<u8>, HtsGetError> {
    let mut r = storage.get(key).await.map_err(internal)?;
    let mut out = Vec::new();
    r.read_to_end(&mut out).await.map_err(internal)?;
    Ok(out)
}

async fn read_range(
    storage: &dyn ObjectStorage,
    key: &str,
    start: u64,
    end_inclusive: u64,
) -> Result<Vec<u8>, HtsGetError> {
    let mut r = storage
        .get_range(key, start, end_inclusive)
        .await
        .map_err(internal)?;
    let mut out = Vec::new();
    r.read_to_end(&mut out).await.map_err(internal)?;
    Ok(out)
}

/// Reference names in index order, from the file header (`header_bytes` = the header range of
/// the data file) or from the index's own tabix header.
fn reference_names(
    format: BgzfFormat,
    header_bytes: &[u8],
    index: &dyn BinningIndex,
) -> Result<Vec<String>, HtsGetError> {
    if let Some(h) = index.header() {
        return Ok(h
            .reference_sequence_names()
            .iter()
            .map(|n| String::from_utf8_lossy(n.as_ref()).into_owned())
            .collect());
    }
    let cursor = std::io::Cursor::new(header_bytes);
    match format {
        BgzfFormat::Bam => {
            let header = noodles::bam::io::Reader::new(cursor)
                .read_header()
                .map_err(|e| internal(format!("BAM header: {e}")))?;
            Ok(header
                .reference_sequences()
                .keys()
                .map(|k| String::from_utf8_lossy(k.as_ref()).into_owned())
                .collect())
        }
        BgzfFormat::Bcf => {
            let header = noodles::bcf::io::Reader::new(cursor)
                .read_header()
                .map_err(|e| internal(format!("BCF header: {e}")))?;
            // BCF stores contig ids from the header dictionary (IDX), which need not follow the
            // order of the ##contig lines; the CSI uses the same ids.
            let contigs = header.string_maps().contigs();
            let mut names = Vec::new();
            for name in header.contigs().keys() {
                if let Some(i) = contigs.get_index_of(name) {
                    if names.len() <= i {
                        names.resize(i + 1, String::new());
                    }
                    names[i] = name.to_string();
                }
            }
            Ok(names)
        }
        BgzfFormat::Vcf => {
            let header = noodles::vcf::io::Reader::new(noodles::bgzf::io::Reader::new(cursor))
                .read_header()
                .map_err(|e| internal(format!("VCF header: {e}")))?;
            Ok(header.contigs().keys().map(|k| k.to_string()).collect())
        }
    }
}

/// Parse a BAI, CSI or TBI index (detected from its magic bytes).
fn parse_index(bytes: &[u8]) -> Result<Box<dyn BinningIndex + Send + Sync>, HtsGetError> {
    let plain_magic = |m: &[u8]| bytes.starts_with(m);
    if plain_magic(b"BAI\x01") {
        let idx = noodles::bam::bai::io::Reader::new(std::io::Cursor::new(bytes))
            .read_index()
            .map_err(|e| internal(format!("BAI: {e}")))?;
        return Ok(Box::new(idx));
    }
    // CSI and TBI are BGZF-compressed; look at the decompressed magic.
    let mut head = [0u8; 4];
    noodles::bgzf::io::Reader::new(std::io::Cursor::new(bytes))
        .read_exact(&mut head)
        .map_err(|e| internal(format!("index: {e}")))?;
    match &head {
        b"CSI\x01" => Ok(Box::new(
            noodles::csi::io::Reader::new(std::io::Cursor::new(bytes))
                .read_index()
                .map_err(|e| internal(format!("CSI: {e}")))?,
        )),
        b"TBI\x01" => Ok(Box::new(
            noodles::tabix::io::Reader::new(std::io::Cursor::new(bytes))
                .read_index()
                .map_err(|e| internal(format!("TBI: {e}")))?,
        )),
        _ => Err(internal("index is not BAI, CSI or TBI")),
    }
}

impl Located {
    /// Parse the index, find where the header ends (the first record's virtual position, from
    /// the index) and read the reference names.
    pub async fn load(
        storage: &dyn ObjectStorage,
        format: BgzfFormat,
        index_key: &str,
        data_key: &str,
    ) -> Result<Self, HtsGetError> {
        let index = parse_index(&read_all(storage, index_key).await?)?;
        let n = index.reference_sequences().count();
        let mut first: Option<VirtualPosition> = index
            .reference_sequences()
            .filter_map(|r| r.metadata().map(|m| m.start_position()))
            .min();
        let whole = Interval::from(Position::MIN..);
        for id in 0..n {
            if let Ok(chunks) = index.query(id, whole) {
                if let Some(c) = chunks.iter().map(|c| c.start()).min() {
                    first = Some(first.map_or(c, |f| f.min(c)));
                }
            }
        }
        let header_end = first.ok_or_else(|| {
            HtsGetError::NotFound("the index lists no records for this object".into())
        })?;
        let header = header_plain(storage, data_key, header_end).await?;
        let names = reference_names(format, &compress(&header)?, index.as_ref())?;
        Ok(Self {
            index,
            names,
            header_end,
        })
    }

    pub fn header_end(&self) -> VirtualPosition {
        self.header_end
    }

    /// Index chunks for one htsget region (0-based start inclusive, end exclusive).
    pub fn chunks(
        &self,
        reference: &str,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Vec<Chunk>, HtsGetError> {
        let id = self
            .names
            .iter()
            .position(|n| n == reference)
            .ok_or_else(|| HtsGetError::NotFound(format!("reference {reference} not found")))?;
        let pos = |v: u64| {
            usize::try_from(v)
                .ok()
                .and_then(Position::new)
                .ok_or_else(|| HtsGetError::InvalidRange(format!("{v} is out of range")))
        };
        // htsget [start, end) 0-based → 1-based closed [start + 1, end].
        let lo = pos(start.unwrap_or(0) + 1)?;
        let interval: Interval = match end {
            Some(0) => return Ok(Vec::new()),
            Some(e) => (lo..=pos(e)?).into(),
            None => (lo..).into(),
        };
        match self.index.query(id, interval) {
            Ok(c) => Ok(c),
            // No bins for this reference (no records on it): nothing to return.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => Ok(Vec::new()),
            Err(e) => Err(internal(format!("index query: {e}"))),
        }
    }
}

/// Sort and merge overlapping or touching chunks.
pub fn merge_chunks(mut chunks: Vec<Chunk>) -> Vec<(VirtualPosition, VirtualPosition)> {
    chunks.sort_by_key(|c| c.start());
    let mut out: Vec<(VirtualPosition, VirtualPosition)> = Vec::new();
    for c in chunks {
        match out.last_mut() {
            Some(last) if c.start() <= last.1 => last.1 = last.1.max(c.end()),
            _ => out.push((c.start(), c.end())),
        }
    }
    out
}

/// One BGZF block at `offset`: (compressed length, decompressed data).
async fn read_block(
    storage: &dyn ObjectStorage,
    key: &str,
    offset: u64,
    size: u64,
) -> Result<(u64, Vec<u8>), HtsGetError> {
    if offset >= size {
        return Err(internal("block offset past end of object"));
    }
    let raw = read_range(storage, key, offset, (offset + MAX_BLOCK - 1).min(size - 1)).await?;
    let bsize = block_size(&raw).ok_or_else(|| internal("not a BGZF block at index offset"))?;
    let mut data = Vec::new();
    noodles::bgzf::io::Reader::new(&raw[..bsize as usize])
        .read_to_end(&mut data)
        .map_err(|e| internal(format!("BGZF block: {e}")))?;
    Ok((bsize, data))
}

/// Decompressed bytes before virtual position `end` (the whole header).
async fn header_plain(
    storage: &dyn ObjectStorage,
    key: &str,
    end: VirtualPosition,
) -> Result<Vec<u8>, HtsGetError> {
    let size = storage.size(key).await.map_err(internal)?;
    let (c, u) = (end.compressed(), end.uncompressed() as usize);
    let mut out = Vec::new();
    if c > 0 {
        let raw = read_range(storage, key, 0, c - 1).await?;
        noodles::bgzf::io::Reader::new(&raw[..])
            .read_to_end(&mut out)
            .map_err(|e| internal(format!("BGZF header blocks: {e}")))?;
    }
    if u > 0 {
        let (_, data) = read_block(storage, key, c, size).await?;
        out.extend_from_slice(
            data.get(..u)
                .ok_or_else(|| internal("index offset beyond block"))?,
        );
    }
    Ok(out)
}

/// Total block length from the BGZF header's `BC` extra subfield.
fn block_size(b: &[u8]) -> Option<u64> {
    if b.len() < 18 || b[0..4] != [0x1f, 0x8b, 0x08, 0x04] {
        return None;
    }
    let xlen = u16::from_le_bytes([b[10], b[11]]) as usize;
    let mut i = 12;
    while i + 4 <= 12 + xlen && i + 4 <= b.len() {
        let slen = u16::from_le_bytes([b[i + 2], b[i + 3]]) as usize;
        if b[i] == b'B' && b[i + 1] == b'C' && slen == 2 && i + 6 <= b.len() {
            let bsize = u16::from_le_bytes([b[i + 4], b[i + 5]]) as u64 + 1;
            return (bsize as usize <= b.len()).then_some(bsize);
        }
        i += 4 + slen;
    }
    None
}

/// Compress `data` as BGZF block(s), without the EOF marker.
fn compress(data: &[u8]) -> Result<Vec<u8>, HtsGetError> {
    let mut w = noodles::bgzf::io::Writer::new(Vec::new());
    w.write_all(data).map_err(internal)?;
    let mut out = w.finish().map_err(internal)?;
    out.truncate(out.len().saturating_sub(28)); // drop the EOF block finish() appends
    Ok(out)
}

fn data_url(bytes: &[u8], class: Class) -> Url {
    Url::new(format!(
        "data:;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
    .set_class(Some(class))
}

fn range_url(stream_url: &str, start: u64, end_inclusive: u64, class: Class) -> Url {
    Url::new(stream_url.to_string())
        .add_headers(
            htsget_config::types::Headers::default()
                .with_header("Range", format!("bytes={start}-{end_inclusive}")),
        )
        .set_class(Some(class))
}

/// URLs for merged virtual ranges: cut edge blocks at record boundaries, `Range` for the rest.
/// `class` is `Header` for the header range `[0, first record)`, `Body` otherwise.
pub async fn range_urls(
    storage: &dyn ObjectStorage,
    data_key: &str,
    stream_url: &str,
    chunks: &[(VirtualPosition, VirtualPosition)],
    class: Class,
) -> Result<Vec<Url>, HtsGetError> {
    let size = storage.size(data_key).await.map_err(internal)?;
    let mut urls = Vec::new();
    for (start, end) in chunks {
        let (sc, su) = (start.compressed(), start.uncompressed() as usize);
        let (ec, eu) = (end.compressed(), end.uncompressed() as usize);
        if (sc, su) >= (ec, eu) {
            continue;
        }
        if sc == ec {
            let (_, data) = read_block(storage, data_key, sc, size).await?;
            let cut = data
                .get(su..eu)
                .ok_or_else(|| internal("index offset beyond block"))?;
            urls.push(data_url(&compress(cut)?, class));
            continue;
        }
        // First block: whole (record boundary at its start) or cut.
        let mut next = sc;
        if su > 0 {
            let (len, data) = read_block(storage, data_key, sc, size).await?;
            let cut = data
                .get(su..)
                .ok_or_else(|| internal("index offset beyond block"))?;
            if !cut.is_empty() {
                urls.push(data_url(&compress(cut)?, class));
            }
            next = sc + len;
        }
        // Whole blocks up to the block holding the chunk end.
        if next < ec {
            urls.push(range_url(stream_url, next, ec - 1, class));
        }
        // Last block: records before the end offset.
        if eu > 0 {
            let (_, data) = read_block(storage, data_key, ec, size).await?;
            let cut = data
                .get(..eu)
                .ok_or_else(|| internal("index offset beyond block"))?;
            urls.push(data_url(&compress(cut)?, class));
        }
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_size_reads_bc_subfield() {
        let block = compress(b"hello world").unwrap();
        assert_eq!(block_size(&block), Some(block.len() as u64));
        assert_eq!(block_size(b"not bgzf at all, really"), None);
    }

    #[test]
    fn compress_round_trips_without_eof() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let c = compress(&data).unwrap();
        let mut back = Vec::new();
        noodles::bgzf::io::Reader::new(&c[..])
            .read_to_end(&mut back)
            .unwrap();
        assert_eq!(back, data);
        assert!(
            !c.ends_with(&[0x1b, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        );
    }

    #[test]
    fn merge_chunks_joins_overlaps() {
        let vp = |c: u64, u: u16| VirtualPosition::try_from((c, u)).unwrap();
        let chunks = vec![
            Chunk::new(vp(100, 5), vp(300, 0)),
            Chunk::new(vp(0, 10), vp(100, 7)),
            Chunk::new(vp(500, 0), vp(600, 1)),
        ];
        let m = merge_chunks(chunks);
        assert_eq!(m, vec![(vp(0, 10), vp(300, 0)), (vp(500, 0), vp(600, 1))]);
    }
}
