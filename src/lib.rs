use bitvec::prelude::*;
use quick_xml::de::from_str;
use qvd_structure::{QvdFieldHeader, QvdTableHeader};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Display;
use std::io::{BufReader, SeekFrom};
use std::io::{self, Read};
use std::path::Path;
use std::str;
use std::{collections::HashMap, fs::File};
use std::{convert::TryInto, io::prelude::*};
pub mod qvd_structure;

type Row = BTreeMap<Header, CellValue>;

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
struct Header(String);

#[derive(Debug, PartialEq, Clone)]
enum CellValue {
    Text(String),
    Int(i32),
    Float(f32),
    Null,
}

impl Display for CellValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CellValue::Text(s) => s,
            CellValue::Int(i) => &i.to_string(),
            CellValue::Float(f) => &f.to_string(),
            CellValue::Null => &String::new(),
        };
        write!(f, "{s}")
    }
}

#[derive(Debug)]
pub struct QvdError {
    kind: QvdErrorKind,
    message: String,
}

#[derive(Debug)]
pub enum QvdErrorKind {
    ReadFile
}

impl From<io::Error> for QvdError {
    fn from(value: io::Error) -> Self {
        QvdError { kind: QvdErrorKind::ReadFile, message: value.to_string() }
    }
}


fn read_qvd(file_name: impl AsRef<Path>) -> Result<Vec<Row>, QvdError> {
    let file = File::open(&file_name)?;
    let mut reader = BufReader::new(file);
    let xml: String = get_xml_data(&mut reader)?;
    let qvd_structure: QvdTableHeader = from_str(&xml).unwrap();
    let rows_start = qvd_structure.offset;
    
    let mut buf_new = Vec::new();
    reader.read_to_end(&mut buf_new).unwrap();
    let row_section_new = &buf_new[rows_start..buf_new.len()];
    let record_byte_size = qvd_structure.record_byte_size;

    let mut rows = Vec::new();

    let data: HashMap<Header, Vec<Option<String>>> = qvd_structure.fields.headers.iter().map(|field| {
        let symbols = get_symbols_as_strings(&buf_new, field);
        let symbol_indexes = get_row_indexes(row_section_new, field, record_byte_size);
        let values = match_symbols_with_indexes(&symbols, &symbol_indexes);
        (Header(field.field_name.clone()), values)
    }).collect();

    for idx in 0..qvd_structure.no_of_records as usize {
        let mut row = Row::new();
        for key in data.keys() {
            let cell_data = data.get(key).unwrap().get(idx).unwrap().clone();
            let cell = match cell_data {
                Some(s) => CellValue::Text(s),
                None => CellValue::Null,
            };
            row.insert(key.clone(), cell);
        }
        rows.push(row);
    }

    Ok(rows)


}

fn match_symbols_with_indexes(symbols: &[String], pointers: &[i64]) -> Vec<Option<String>> {
    let mut cols: Vec<Option<String>> = Vec::new();
    for pointer in pointers.iter() {
        if symbols.is_empty() || *pointer < 0 {
            cols.push(None);
        } else {
            cols.push(Some(symbols[*pointer as usize].clone()));
        }
    }
    cols
}

fn get_symbols_as_strings(buf: &[u8], field: &QvdFieldHeader) -> Vec<String> {
    let start = field.offset;
    let end = start + field.length;
    let mut string_start: usize = 0;
    let mut strings: Vec<String> = Vec::new();

    let mut i = start;
    while i < end {
        let byte = &buf[i];
        // Check first byte of symbol. This is not part of the symbol but tells us what type of data to read.
        match byte {
            0 => {
                // Strings are null terminated
                // Read bytes from start fo string (string_start) up to current byte.
                let utf8_bytes = buf[string_start..i].to_vec().to_owned();
                let value = String::from_utf8(utf8_bytes).unwrap_or_else(|_| {
                    panic!(
                    "Error parsing string value in field: {}, field offset: {}, byte offset: {}",
                    field.field_name, start, i
                )
                });
                strings.push(value);
                i += 1;
            }
            1 => {
                // 4 byte integer
                let target_bytes = buf[i + 1..i + 5].to_vec();
                let byte_array: [u8; 4] = target_bytes.try_into().unwrap();
                let numeric_value = i32::from_le_bytes(byte_array);
                strings.push(numeric_value.to_string());
                i += 5;
            }
            2 => {
                // 4 byte double
                let target_bytes = buf[i + 1..i + 9].to_vec();
                let byte_array: [u8; 8] = target_bytes.try_into().unwrap();
                let numeric_value = f64::from_le_bytes(byte_array);
                strings.push(numeric_value.to_string());
                i += 9;
            }
            4 => {
                // Beginning of a null terminated string type
                // Mark where string value starts, excluding preceding byte 0x04
                i += 1;
                string_start = i;
            }
            5 => {
                // 4 bytes of unknown followed by null terminated string
                // Skip the 4 bytes before string
                i += 5;
                string_start = i;
            }
            6 => {
                // 8 bytes of unknown followed by null terminated string
                // Skip the 8 bytes before string
                i += 9;
                string_start = i;
            }
            _ => {
                // Part of a string, do nothing until null terminator
                i += 1;
            }
        }
    }
    strings
}

// Retrieve bit stuffed data. Each row has index to value from symbol map.
fn get_row_indexes(buf: &[u8], field: &QvdFieldHeader, record_byte_size: usize) -> Vec<i64> {
    // let mut cloned_buf = buf.to_owned();
    let chunks = buf.chunks(record_byte_size);
    let mut indexes: Vec<i64> = Vec::new();
    for chunk in chunks {
        // Reverse the bytes in the record
        let mut chunk = chunk.to_vec();
        chunk.reverse();
        let bits = BitSlice::<Msb0, _>::from_slice(&chunk).unwrap();
        let start = bits.len() - field.bit_offset;
        let end = bits.len() - field.bit_offset - field.bit_width;
        let binary = bitslice_to_vec(&bits[end..start]);
        let index = binary_to_u32(binary);
        indexes.push((index as i32 + field.bias) as i64);
    }
    indexes
}

// Slow
fn binary_to_u32(binary: Vec<u8>) -> u32 {
    let mut sum: u32 = 0;
    for bit in binary {
        sum <<= 1;
        sum += bit as u32;
    }
    sum
}

// Slow
fn bitslice_to_vec(bitslice: &BitSlice<Msb0, u8>) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    for bit in bitslice {
        let val = match bit {
            true => 1,
            false => 0,
        };
        v.push(val);
    }
    v
}

fn get_xml_data(reader: &mut BufReader<File>) -> Result<String, io::Error> {
    let mut buffer = Vec::new();
    // There is a line break, carriage return and a null terminator between the XMl and data
    // Find the null terminator
    reader.read_until(0, &mut buffer)
        .expect("Failed to read file");
    let xml_string =
        String::from_utf8(buffer).expect("xml section contains invalid UTF-8 chars");
    Ok(xml_string)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, time::Instant};

    use super::*;

    #[test]
    fn test_double() {
        let buf: Vec<u8> = vec![
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x7a, 0x40, 0x02, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x50, 0x7a, 0x40,
        ];
        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected = vec![420.0.to_string(), 421.0.to_string()];
        assert_eq!(expected, res);
    }

    #[test]
    fn test_int() {
        let buf: Vec<u8> = vec![0x01, 0x0A, 0x00, 0x00, 0x00, 0x01, 0x14, 0x00, 0x00, 0x00];
        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected = vec![10.0.to_string(), 20.0.to_string()];
        assert_eq!(expected, res);
    }

    #[test]
    #[rustfmt::skip]
    fn test_mixed_numbers() {
        let buf: Vec<u8> = vec![
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x7a, 0x40, 
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0x7a, 0x40,
            0x01, 0x01, 0x00, 0x00, 0x00, 
            0x01, 0x02, 0x00, 0x00, 0x00,
            0x05, 0x00, 0x00, 0x00, 0x00, 0x37, 0x30, 0x30, 0x30, 0x00,
            0x06, 0x00,0x00,0x00, 0x00,0x00,0x00,0x00,0x00, 0x38, 0x36, 0x35, 0x2e, 0x32, 0x00
        ];
        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected: Vec<String> = vec![
            420.to_string(),
            421.to_string(),
            1.to_string(),
            2.to_string(),
            7000.to_string(),
            865.2.to_string()
        ];
        assert_eq!(expected, res);
    }

    #[test]
    fn test_string() {
        let buf: Vec<u8> = vec![
            4, 101, 120, 97, 109, 112, 108, 101, 32, 116, 101, 120, 116, 0, 4, 114, 117, 115, 116,
            0,
        ];
        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected: Vec<String> = vec!["example text".into(), "rust".into()];
        assert_eq!(expected, res);
    }

    #[test]
    #[rustfmt::skip]
    fn test_utf8_string() {
        let buf: Vec<u8> = vec![
            0x04, 0xE4, 0xB9, 0x9F, 0xE6, 0x9C, 0x89, 0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0xE7,
            0xAE, 0x80, 0xE4, 0xBD, 0x93, 0xE5, 0xAD, 0x97, 0x00,
            0x04, 0xF0, 0x9F, 0x90, 0x8D, 0xF0, 0x9F, 0xA6, 0x80, 0x00,
        ];

        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected: Vec<String> = vec!["也有中文简体字".into(), "🐍🦀".into()];
        assert_eq!(expected, res);
    }

    #[test]
    fn test_mixed_string() {
        let buf: Vec<u8> = vec![
            4, 101, 120, 97, 109, 112, 108, 101, 32, 116, 101, 120, 116, 0, 4, 114, 117, 115, 116,
            0, 5, 42, 65, 80, 1, 49, 50, 51, 52, 0, 6, 1, 1, 1, 1, 1, 1, 1, 1, 100, 111, 117, 98,
            108, 101, 0,
        ];
        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_symbols_as_strings(&buf, &field);
        let expected: Vec<String> = vec![
            "example text".into(),
            "rust".into(),
            "1234".into(),
            "double".into(),
        ];
        assert_eq!(expected, res);
    }

    #[test]
    fn test_bitslice_to_vec() {
        let mut x: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x11, 0x01, 0x22, 0x02, 0x33, 0x13, 0x34, 0x14, 0x35,
        ];
        let bits = BitSlice::<Msb0, _>::from_slice(&mut x[..]).unwrap();
        let target = &bits[27..32];
        let binary_vec = bitslice_to_vec(&target);

        let mut sum: u32 = 0;
        for bit in binary_vec {
            sum <<= 1;
            sum += bit as u32;
        }
        assert_eq!(17, sum);
    }

    #[test]
    fn test_get_row_indexes() {
        let buf: Vec<u8> = vec![
            0x00, 0x14, 0x00, 0x11, 0x01, 0x22, 0x02, 0x33, 0x13, 0x34, 0x24, 0x35,
        ];
        let field = QvdFieldHeader {
            field_name: String::from("name"),
            offset: 0,
            length: 0,
            bit_offset: 10,
            bit_width: 3,
            bias: 0,
        };
        let record_byte_size = buf.len();
        let res = get_row_indexes(&buf, &field, record_byte_size);
        let expected: Vec<i64> = vec![5];
        assert_eq!(expected, res);
    }

    #[test]
    fn read_test_file_qvd_null() {
        let result = read_qvd("tests/test_qvd_null.qvd").unwrap();

        let some_null = [
            CellValue::Text(1.2.to_string()), 
            CellValue::Text(format!("{:.1}", 10.0)), 
            CellValue::Text(64.to_string()),
            CellValue::Null,
            CellValue::Null,
            CellValue::Null,
            CellValue::Text(1.to_string()),
            CellValue::Text(213.95625.to_string()),
            CellValue::Text(2.to_string()),
            CellValue::Text(3.to_string()),
            CellValue::Text(5.to_string()),
            CellValue::Text(1000.to_string()),
        ];

        let mut expected = Vec::new();
        for i in 1..=12 {
            let mut map = BTreeMap::new();
            map.insert(Header("Quarter".into()), CellValue::Text(format!("Q{}", (i - 1) / 3 + 1)));
            map.insert(Header("Month".into()), CellValue::Text(i.to_string()));
            map.insert(Header("some_null".into()), some_null[i - 1].clone());
            map.insert(Header("all Null".into()), CellValue::Null);
            expected.push(map);
        }
        
        assert_eq!(expected, result);
    }

    #[test]
    #[ignore = "manual test"]
    fn read_test_file() {
        let now = Instant::now();
        let result = read_qvd("test/big_file.qvd").unwrap();
        let duration = Instant::now().checked_duration_since(now).unwrap();
        println!("Duration reading: {duration:?}");

        assert_eq!(result.len(), 20526);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("output.csv")
            .unwrap();
        
        let header = result.first().unwrap().keys().map(|k| k.0.as_str())
            .collect::<Vec<&str>>()
            .join(",");
        file.write_all(header.as_bytes()).unwrap();
            
        for row in result {
            let mut content = row.into_iter().map(|(key, values)| {
                values.to_string()
            })
                .collect::<Vec<String>>()
                .join(",");
            content.push('\n');
            file.write_all(content.as_bytes()).unwrap();
        }
        
    }
}
