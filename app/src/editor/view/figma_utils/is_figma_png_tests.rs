
fn build_png_with_text_chunk(keyword: &[u8], text: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let data: Vec<u8> = keyword.iter().chain(b"\x00").chain(text).copied().collect();
    let length = data.len() as u32;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(b"tEXt");
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(&[0u8; 4]); // fake CRC
    bytes
}









