use rugb::savestate::*;

#[test]
fn test_savestate_push_pop_u8() {
    let mut data = Vec::new();
    push_u8(&mut data, 0x42);
    push_u8(&mut data, 0xFF);
    let mut cursor: &[u8] = &data;
    assert_eq!(pop_u8(&mut cursor), 0x42);
    assert_eq!(pop_u8(&mut cursor), 0xFF);
}

#[test]
fn test_savestate_push_pop_u16() {
    let mut data = Vec::new();
    push_u16(&mut data, 0xBEEF);
    let mut cursor: &[u8] = &data;
    assert_eq!(pop_u16(&mut cursor), 0xBEEF);
}

#[test]
fn test_savestate_push_pop_u32() {
    let mut data = Vec::new();
    push_u32(&mut data, 0xDEAD_BEEF);
    let mut cursor: &[u8] = &data;
    assert_eq!(pop_u32(&mut cursor), 0xDEAD_BEEF);
}

#[test]
fn test_savestate_push_pop_bool() {
    let mut data = Vec::new();
    push_bool(&mut data, true);
    push_bool(&mut data, false);
    let mut cursor: &[u8] = &data;
    assert!(pop_bool(&mut cursor));
    assert!(!pop_bool(&mut cursor));
}

#[test]
fn test_savestate_push_pop_i8() {
    let mut data = Vec::new();
    push_i8(&mut data, -42);
    push_i8(&mut data, 100);
    let mut cursor: &[u8] = &data;
    assert_eq!(pop_i8(&mut cursor), -42);
    assert_eq!(pop_i8(&mut cursor), 100);
}

#[test]
fn test_savestate_push_pop_slice() {
    let mut data = Vec::new();
    let slice = vec![1u8, 2, 3, 4, 5];
    push_slice(&mut data, &slice);
    let mut cursor: &[u8] = &data;
    let result = pop_vec(&mut cursor);
    assert_eq!(result, vec![1, 2, 3, 4, 5]);
}
