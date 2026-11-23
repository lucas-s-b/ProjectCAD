use super::*;

fn u16_at(b: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn u32_at(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize
}

/// The entries of a stored zip, read through its local headers; checks method, sizes, CRCs and the end record.
pub(crate) fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut at = 0;
    while u32_at(bytes, at) == 0x0403_4b50 {
        assert_eq!(u16_at(bytes, at + 8), 0, "method stored");
        let (crc, size, name_len, extra) = (u32_at(bytes, at + 14), u32_at(bytes, at + 18), u16_at(bytes, at + 26), u16_at(bytes, at + 28));
        assert_eq!(size, u32_at(bytes, at + 22));
        let name = String::from_utf8(bytes[at + 30..at + 30 + name_len].to_vec()).unwrap();
        let start = at + 30 + name_len + extra;
        let data = bytes[start..start + size].to_vec();
        assert_eq!(crc, crc32(&data) as usize);
        files.push((name, data));
        at = start + size;
    }
    // `at` is the start of the central directory; the end record is the last 22 bytes.
    let end = bytes.len() - 22;
    assert_eq!(u32_at(bytes, end), 0x0605_4b50);
    assert_eq!((u16_at(bytes, end + 8), u16_at(bytes, end + 10)), (files.len(), files.len()));
    assert_eq!(u32_at(bytes, end + 16), at);
    assert_eq!(u32_at(bytes, at), 0x0201_4b50);
    files
}

#[test]
fn crc32_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn stored_zip_reads_back() {
    let zip = zip_stored(&[("a.txt", b"hello"), ("d/b.bin", &[0, 1, 2])]).unwrap();
    assert_eq!(unzip(&zip), [("a.txt".to_string(), b"hello".to_vec()), ("d/b.bin".to_string(), vec![0, 1, 2])]);
}
