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

pub fn push_i16(data: &mut Vec<u8>, v: i16) {
    data.extend_from_slice(&v.to_le_bytes());
}

pub fn push_slice(data: &mut Vec<u8>, s: &[u8]) {
    push_u32(data, s.len() as u32);
    data.extend_from_slice(s);
}

pub fn pop_u8(data: &mut &[u8]) -> u8 {
    if data.is_empty() {
        return 0;
    }
    let v = data[0];
    *data = &data[1..];
    v
}

pub fn pop_u16(data: &mut &[u8]) -> u16 {
    if data.len() < 2 {
        *data = &[];
        return 0;
    }
    let v = u16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];
    v
}

pub fn pop_u32(data: &mut &[u8]) -> u32 {
    if data.len() < 4 {
        *data = &[];
        return 0;
    }
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

pub fn pop_i16(data: &mut &[u8]) -> i16 {
    if data.len() < 2 {
        *data = &[];
        return 0;
    }
    let v = i16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];
    v
}

const MAX_VEC_LEN: usize = 1024 * 1024; // 1MB cap on deserialized vectors

pub fn pop_vec(data: &mut &[u8]) -> Vec<u8> {
    let len = pop_u32(data) as usize;
    let len = len.min(data.len()).min(MAX_VEC_LEN);
    let v = data[..len].to_vec();
    *data = &data[len..];
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_u8_roundtrip() {
        let mut buf = Vec::new();
        push_u8(&mut buf, 0xAB);
        let mut slice: &[u8] = &buf;
        assert_eq!(pop_u8(&mut slice), 0xAB);
    }

    #[test]
    fn push_pop_u16_little_endian() {
        let mut buf = Vec::new();
        push_u16(&mut buf, 0x1234);
        // LE: low byte first
        assert_eq!(buf[0], 0x34);
        assert_eq!(buf[1], 0x12);
        let mut slice: &[u8] = &buf;
        assert_eq!(pop_u16(&mut slice), 0x1234);
    }

    #[test]
    fn push_pop_u32_little_endian() {
        let mut buf = Vec::new();
        push_u32(&mut buf, 0xDEADBEEF);
        assert_eq!(buf[0], 0xEF);
        assert_eq!(buf[1], 0xBE);
        assert_eq!(buf[2], 0xAD);
        assert_eq!(buf[3], 0xDE);
        let mut slice: &[u8] = &buf;
        assert_eq!(pop_u32(&mut slice), 0xDEADBEEF);
    }

    #[test]
    fn push_pop_bool_true() {
        let mut buf = Vec::new();
        push_bool(&mut buf, true);
        let mut slice: &[u8] = &buf;
        assert!(pop_bool(&mut slice));
    }

    #[test]
    fn push_pop_bool_false() {
        let mut buf = Vec::new();
        push_bool(&mut buf, false);
        let mut slice: &[u8] = &buf;
        assert!(!pop_bool(&mut slice));
    }

    #[test]
    fn push_pop_i8_negative() {
        let mut buf = Vec::new();
        push_i8(&mut buf, -42i8);
        let mut slice: &[u8] = &buf;
        assert_eq!(pop_i8(&mut slice), -42i8);
    }

    #[test]
    fn push_pop_slice_empty() {
        let mut buf = Vec::new();
        push_slice(&mut buf, &[]);
        let mut slice: &[u8] = &buf;
        let result = pop_vec(&mut slice);
        assert!(result.is_empty());
    }

    #[test]
    fn push_pop_slice_nonempty() {
        let data = [0x01u8, 0x02, 0x03, 0xFF];
        let mut buf = Vec::new();
        push_slice(&mut buf, &data);
        let mut slice: &[u8] = &buf;
        let result = pop_vec(&mut slice);
        assert_eq!(result, data);
    }

    #[test]
    fn push_pop_slice_advances_cursor() {
        let payload = [0xAAu8, 0xBB];
        let mut buf = Vec::new();
        push_slice(&mut buf, &payload);
        push_u8(&mut buf, 0x42);
        let mut slice: &[u8] = &buf;
        let result = pop_vec(&mut slice);
        assert_eq!(result, payload);
        // Cursor should now sit right at the trailing u8
        assert_eq!(pop_u8(&mut slice), 0x42);
    }

    #[test]
    fn multiple_values_in_sequence() {
        let mut buf = Vec::new();
        push_u8(&mut buf, 7);
        push_u16(&mut buf, 1000);
        push_bool(&mut buf, true);
        push_i8(&mut buf, -1);
        push_u32(&mut buf, 0xCAFE);

        let mut slice: &[u8] = &buf;
        assert_eq!(pop_u8(&mut slice), 7);
        assert_eq!(pop_u16(&mut slice), 1000);
        assert!(pop_bool(&mut slice));
        assert_eq!(pop_i8(&mut slice), -1i8);
        assert_eq!(pop_u32(&mut slice), 0xCAFE);
        assert!(slice.is_empty());
    }
}
