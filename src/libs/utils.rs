use std::fmt::Write;

use convert_case::{Case, Casing};
use eyre::{ContextCompat, Result};
use serde::Serialize;

use crate::model::EndpointSchema;

pub fn get_log_id() -> u64 {
    chrono::Utc::now().timestamp_micros() as _
}

pub fn get_conn_id() -> u32 {
    chrono::Utc::now().timestamp_micros() as _
}

pub fn encode_header<T: Serialize>(v: T, schema: EndpointSchema) -> Result<String> {
    let mut s = String::new();
    write!(s, "0{}", schema.name.to_ascii_lowercase())?;
    let v = serde_json::to_value(&v)?;

    for (i, f) in schema.parameters.iter().enumerate() {
        let key = f.name.to_case(Case::Camel);
        let value = v.get(&key).with_context(|| format!("key: {}", key))?;
        if value.is_null() {
            continue;
        }
        write!(
            s,
            ", {}{}",
            i + 1,
            urlencoding::encode(&value.to_string().replace('\"', ""))
        )?;
    }
    Ok(s)
}

pub fn get_time_milliseconds() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
pub fn get_time_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
pub fn hex_decode(s: &[u8]) -> Result<Vec<u8>> {
    if s.starts_with(b"0x") {
        Ok(hex::decode(&s[2..])?)
    } else {
        Ok(hex::decode(s)?)
    }
}

/// format decimal by specific significant figures
pub fn decimal_sf(num: rust_decimal::Decimal, sig_figs: usize) -> rust_decimal::Decimal {
    num.round_sf(sig_figs as _).unwrap()
}

/// Aligns the precision of one `f64` value to match another `f64` value.
///
/// # Examples
///
/// ```
/// let a = 123.456789;
/// let b = 78.9;
/// let aligned_a = lib::utils::align_precision(a, b);
/// assert_eq!(aligned_a, 123.5);
/// ```
/// ```
/// let a = 2.37;
/// let b = 631.3;
/// let aligned_a = lib::utils::align_precision(a, b);
/// assert_eq!(aligned_a, 2.4);
/// ```
pub fn align_precision(a: f64, b: f64) -> f64 {
    let precision_b = count_dp(b);
    let precision_a = format!("{:.0$}", { precision_b });
    let aligned_a = format!("{:.*}", precision_a.parse::<usize>().unwrap(), a);
    aligned_a.parse().unwrap()
}

/// ```
/// assert_eq!(lib::utils::count_dp(1.5), 1);
/// ```
/// ```
/// assert_eq!(lib::utils::count_dp(1.1), 1);
/// ```
pub fn count_dp(num: f64) -> usize {
    // Convert the f64 to a string representation
    let num_str = format!("{}", num);

    // Find the position of the decimal point
    if let Some(decimal_index) = num_str.find('.') {
        // Count the number of characters after the decimal point
        num_str.len() - decimal_index - 1
    } else {
        // If there is no decimal point, return 0
        0
    }
}
