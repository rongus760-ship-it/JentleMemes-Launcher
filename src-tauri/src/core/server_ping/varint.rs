use crate::error::{Error, Result};

pub fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut b = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if value == 0 {
            break;
        }
    }
}

pub fn read_varint_slice(data: &mut &[u8]) -> Result<i32> {
    let mut num = 0i32;
    let mut shift = 0;
    for _ in 0..5 {
        let Some(&b) = data.first() else {
            return Err(Error::Custom("VarInt: неожиданный конец буфера".into()));
        };
        *data = &data[1..];
        num |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 {
            return Ok(num);
        }
        shift += 7;
    }
    Err(Error::Custom("VarInt: слишком длинный".into()))
}

pub fn write_string(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    let b = s.as_bytes();
    if b.len() > i32::MAX as usize {
        return Err(Error::Custom("Строка пакета слишком длинная".into()));
    }
    write_varint(buf, b.len() as i32);
    buf.extend_from_slice(b);
    Ok(())
}

pub fn read_string_slice(data: &mut &[u8]) -> Result<String> {
    let len = read_varint_slice(data)?;
    if len < 0 {
        return Err(Error::Custom("Отрицательная длина строки".into()));
    }
    let len = len as usize;
    if len > data.len() {
        return Err(Error::Custom("Строка выходит за пределы пакета".into()));
    }
    let (a, rest) = data.split_at(len);
    *data = rest;
    String::from_utf8(a.to_vec()).map_err(|e| Error::Custom(format!("UTF-8: {e}")))
}
