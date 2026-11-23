//! A zip archive of stored (uncompressed) entries, enough for 3MF: local headers, central directory, end record.

/// CRC-32 (IEEE, reflected, polynomial 0xEDB88320), as zip stores it.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// 1980-01-01 00:00 in MS-DOS format: the earliest date zip can hold (0 is not a valid date).
const DOS_DATE: u16 = 0x0021;

const TOO_BIG: &str = "3MF larger than 4 GiB (zip64 is not written)";

/// The archive of `files` (name, contents) in order, all stored; Err when it would need zip64 (4 GiB or 65 535 entries).
pub(crate) fn zip_stored(files: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
    let to32 = |n: usize| u32::try_from(n).map_err(|_| TOO_BIG.to_string());
    let to16 = |n: usize| u16::try_from(n).map_err(|_| TOO_BIG.to_string());
    let mut out: Vec<u8> = Vec::new();
    // (crc, size, name length, local header offset) per file, for the central directory.
    let mut seen = Vec::new();
    for (name, data) in files {
        let (crc, size, name_len, at) = (crc32(data), to32(data.len())?, to16(name.len())?, to32(out.len())?);
        out.extend(0x0403_4b50u32.to_le_bytes());
        // version needed 20, flags 0, method 0 (stored), time 0, date
        for v in [20u16, 0, 0, 0, DOS_DATE] {
            out.extend(v.to_le_bytes());
        }
        for v in [crc, size, size] {
            out.extend(v.to_le_bytes());
        }
        // name length, extra length 0
        out.extend(name_len.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(name.as_bytes());
        out.extend(*data);
        seen.push((crc, size, name_len, at));
    }
    let dir_at = to32(out.len())?;
    for ((name, _), (crc, size, name_len, at)) in files.iter().zip(&seen) {
        out.extend(0x0201_4b50u32.to_le_bytes());
        // version made by 20, version needed 20, flags 0, method 0, time 0, date
        for v in [20u16, 20, 0, 0, 0, DOS_DATE] {
            out.extend(v.to_le_bytes());
        }
        for v in [*crc, *size, *size] {
            out.extend(v.to_le_bytes());
        }
        // name length, extra 0, comment 0, disk 0, internal attributes 0
        for v in [*name_len, 0, 0, 0, 0] {
            out.extend(v.to_le_bytes());
        }
        // external attributes 0, local header offset
        out.extend(0u32.to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend(name.as_bytes());
    }
    let dir_size = to32(out.len())? - dir_at;
    let entries = to16(files.len())?;
    out.extend(0x0605_4b50u32.to_le_bytes());
    // disk 0, directory disk 0, entries on disk, entries
    for v in [0u16, 0, entries, entries] {
        out.extend(v.to_le_bytes());
    }
    out.extend(dir_size.to_le_bytes());
    out.extend(dir_at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
#[path = "zip_out_tests.rs"]
pub(crate) mod tests;
