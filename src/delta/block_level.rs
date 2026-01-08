use crate::delta::rolling::RollingChecksum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write, Seek, SeekFrom};

pub const DEFAULT_BLOCK_SIZE: usize = 4096;

/// Signature for a single block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSignature {
    pub index: u32,
    pub weak: u32,       // Adler-32
    pub strong: [u8; 16], // BLAKE3 (first 16 bytes)
}

/// Signature for the whole file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignature {
    pub blocks: Vec<BlockSignature>,
    pub block_size: usize,
    pub file_size: u64,
}

/// Operation to reconstruct the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOp {
    /// Copy existing block from old file
    Copy { index: u32 },
    /// Insert new data
    Data { data: Vec<u8> },
}

/// Delta containing ops to patch the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDelta {
    pub ops: Vec<DeltaOp>,
    pub final_size: u64,
}

/// Compute signature for a file
pub fn compute_signature<R: Read>(reader: &mut R, block_size: usize) -> std::io::Result<FileSignature> {
    let mut blocks = Vec::new();
    let mut buffer = vec![0u8; block_size];
    let mut index = 0;
    let mut file_size = 0;

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file_size += n as u64;

        let chunk = &buffer[0..n];
        
        // Weak checksum
        let mut weak_calc = RollingChecksum::new();
        weak_calc.update(chunk);
        let weak = weak_calc.digest();
        
        // Strong checksum
        let strong_hash = blake3::hash(chunk);
        let mut strong = [0u8; 16];
        strong.copy_from_slice(&strong_hash.as_bytes()[0..16]);

        blocks.push(BlockSignature {
            index,
            weak,
            strong,
        });

        index += 1;
    }

    Ok(FileSignature {
        blocks,
        block_size,
        file_size,
    })
}

/// Compute delta given a local file and a remote signature
pub fn compute_delta(local_data: &[u8], remote_sig: &FileSignature) -> FileDelta {
    let block_size = remote_sig.block_size;
    let mut ops = Vec::new();
    
    // Build lookup table for weak checksums: weak -> Vec<BlockSignature>
    let mut lookup: HashMap<u32, Vec<&BlockSignature>> = HashMap::new();
    for block in &remote_sig.blocks {
        lookup.entry(block.weak).or_default().push(block);
    }

    let mut pos = 0;
    let mut literal_start = 0;
    let mut rolling = RollingChecksum::new();
    
    // Initialize window
    if local_data.len() >= block_size {
        rolling.update(&local_data[0..block_size]);
    }

    while pos + block_size <= local_data.len() {
        let weak = rolling.digest();
        let mut matched_block: Option<&BlockSignature> = None;

        if let Some(candidates) = lookup.get(&weak) {
            let chunk = &local_data[pos..pos + block_size];
            let strong_hash = blake3::hash(chunk);
            let strong_bytes = &strong_hash.as_bytes()[0..16];

            for candidate in candidates {
                if candidate.strong == strong_bytes {
                    matched_block = Some(candidate);
                    break;
                }
            }
        }

        if let Some(block) = matched_block {
            // Found a match!
            
            // 1. Flush any pending literal data
            if pos > literal_start {
                ops.push(DeltaOp::Data { 
                    data: local_data[literal_start..pos].to_vec() 
                });
            }

            // 2. Add Copy instruction
            ops.push(DeltaOp::Copy { index: block.index });

            // 3. Move window forward by block_size
            pos += block_size;
            literal_start = pos;
            
            // Reset rolling checksum for next window
            if pos + block_size <= local_data.len() {
                rolling.update(&local_data[pos..pos + block_size]);
            }
        } else {
            // No match
            // Can we roll to the next byte?
            if pos + block_size < local_data.len() {
                let old_byte = local_data[pos];
                let new_byte = local_data[pos + block_size]; 
                rolling.roll(old_byte, new_byte);
                pos += 1;
            } else {
                // Reached end of file with a full block that didn't match.
                // We treat the current byte as literal and advance.
                // Since we can't form a new window, we just break the loop 
                // (or let the loop condition handle it after increment).
                pos += 1;
            }
        }
    }

    // Handle remaining data (tail)
    if literal_start < local_data.len() {
         ops.push(DeltaOp::Data { 
             data: local_data[literal_start..].to_vec() 
         });
    }

    FileDelta {
        ops,
        final_size: local_data.len() as u64,
    }
}

/// Apply delta to reconstruct the file
/// `old_file` must be seekable to read blocks
pub fn apply_delta<R: Read + Seek + ?Sized, W: Write>(old_file: &mut R, delta: &FileDelta, out_file: &mut W, block_size: usize) -> std::io::Result<()> {
    for op in &delta.ops {
        match op {
            DeltaOp::Data { data } => {
                out_file.write_all(data)?;
            },
            DeltaOp::Copy { index } => {
                let offset = *index as u64 * block_size as u64;
                old_file.seek(SeekFrom::Start(offset))?;
                
                // Note: The last block might be smaller than block_size.
                // But the Signature was generated from the old file, so the index guarantees validity
                // EXCEPT if the old file was truncated/changed concurrently.
                // We should read `block_size` or up to EOF.
                // However, strict rsync usually implies we know the exact size of that block. 
                // For simplicity here, we assume full blocks except maybe last.
                // Actually, standard rsync saves block lengths if variable. 
                // Here we simplify: We just read block_size. 
                // If it's the last block, read might return less, which is fine if it matches what was signed.
                // But wait, if we copy the last block of old file, and it was 100 bytes, we copy 100 bytes.
                // We need to limit the read.
                
                let mut buffer = vec![0u8; block_size];
                let n = old_file.read(&mut buffer)?;
                out_file.write_all(&buffer[0..n])?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_signature_and_delta() {
        let old_data = b"Hello world, this is a test file for sync."; // 42 bytes
        let block_size = 10;
        
        // 1. Compute Signature of Old Data
        let mut cursor = Cursor::new(old_data);
        let sig = compute_signature(&mut cursor, block_size).unwrap();
        
        // Blocks: "Hello worl", "d, this is", " a test fi", "le for syn", "c."
        assert_eq!(sig.blocks.len(), 5);

        // 2. New Data: Modify middle, append end
        // "Hello world, this is A CHANGED file for sync. EXTRA"
        // Matches: "Hello worl" (0), "d, this is" (1), "le for syn" (3), "c." (4 is prefix of "c. EXTRA"?)
        // Wait, "c." is 2 bytes. 
        // Our rolling checksum logic in compute_delta handles literal inserts.
        
        let new_data = b"Hello world, this is A CHANGED file for sync. EXTRA";
        
        // 3. Compute Delta
        let delta = compute_delta(new_data, &sig);
        
        // Expected:
        // Copy 0 ("Hello worl")
        // Copy 1 ("d, this is")
        // Insert " A CHANGED fi" (replacing " a test fi")
        // Copy 3 ("le for syn")
        // Insert "c. EXTRA" (replacing "c.") - wait, "c." might match block 4?
        // Block 4 is "c.". 
        // Rolling check for "c. E" vs "c.". 
        // If window is 10, and data is only 2, compute_signature produces a block of 2.
        // compute_delta window is 10. It won't match a block of size 2 unless we handle small blocks.
        // CURRENT SIMPLIFICATION: We only match full blocks. Tail blocks are usually only matched if they are at the end?
        // Rsync handles this by having a specific size for the last block.
        // Our `compute_delta` loop condition `pos + block_size <= len` ignores the tail for matching.
        // So the tail will always be literal data.
        
        // Let's verify.
        
        // 4. Apply Delta
        let mut old_reader = Cursor::new(old_data);
        let mut out_buffer = Vec::new();
        apply_delta(&mut old_reader, &delta, &mut out_buffer, block_size).unwrap();
        
        assert_eq!(out_buffer, new_data);
    }
}
