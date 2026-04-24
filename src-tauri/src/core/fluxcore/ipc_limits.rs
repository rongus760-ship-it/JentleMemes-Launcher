use crate::error::{Error, Result};

const MAX_STRING_LEN: usize = 512 * 1024;
const MAX_URL_LEN: usize = 4096;
const MAX_BASE64_LEN: usize = 2 * 1024 * 1024;

pub fn validate_string(value: &str, field_name: &str) -> Result<()> {
    if value.len() > MAX_STRING_LEN {
        return Err(Error::Custom(format!(
            "IPC: поле '{field_name}' слишком большое ({} > {MAX_STRING_LEN} байт)",
            value.len()
        )));
    }
    Ok(())
}

pub fn validate_url(value: &str, field_name: &str) -> Result<()> {
    if value.len() > MAX_URL_LEN {
        return Err(Error::Custom(format!(
            "IPC: URL '{field_name}' слишком длинный ({} > {MAX_URL_LEN} байт)",
            value.len()
        )));
    }
    Ok(())
}

pub fn validate_base64(value: &str, field_name: &str) -> Result<()> {
    if value.len() > MAX_BASE64_LEN {
        return Err(Error::Custom(format!(
            "IPC: base64 '{field_name}' слишком большой ({} > {MAX_BASE64_LEN} байт)",
            value.len()
        )));
    }
    Ok(())
}

pub fn validate_vec_len<T>(vec: &[T], max: usize, field_name: &str) -> Result<()> {
    if vec.len() > max {
        return Err(Error::Custom(format!(
            "IPC: массив '{field_name}' слишком большой ({} > {max})",
            vec.len()
        )));
    }
    Ok(())
}
