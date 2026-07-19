//! Save-state serialisation helpers.
//!
//! All values are stored in little-endian byte order.

pub fn push_u8(data: &mut Vec<u8>, v: u8) {
    data.push(v);
}

pub fn push_u16(data: &mut Vec<u8>, v: u16) {
    data.extend_from_slice(&v.to_le_bytes());
}

pub fn push_u32(data: &mut Vec<u8>, v: u32) {
    data.extend_from_slice(&v.to_le_bytes());
}

pub fn push_bool(data: &mut Vec<u8>, v: bool) {
    data.push(v as u8);
}

pub fn push_i8(data: &mut Vec<u8>, v: i8) {
    data.push(v as u8);
}

pub fn push_slice(data: &mut Vec<u8>, s: &[u8]) {
    push_u32(data, s.len() as u32);
    data.extend_from_slice(s);
}

pub fn pop_u8(data: &mut &[u8]) -> u8 {
    let v = data[0];
    *data = &data[1..];
    v
}

pub fn pop_u16(data: &mut &[u8]) -> u16 {
    let v = u16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];
    v
}

pub fn pop_u32(data: &mut &[u8]) -> u32 {
    let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    *data = &data[4..];
    v
}

pub fn pop_bool(data: &mut &[u8]) -> bool {
    pop_u8(data) != 0
}

pub fn pop_i8(data: &mut &[u8]) -> i8 {
    pop_u8(data) as i8
}

pub fn pop_vec(data: &mut &[u8]) -> Vec<u8> {
    let len = pop_u32(data) as usize;
    let v = data[..len].to_vec();
    *data = &data[len..];
    v
}
