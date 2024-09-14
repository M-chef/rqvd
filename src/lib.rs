use bitvec::prelude::*;
use column::{CellValue, Column, Header};
use quick_xml::de::from_str;
use qvd_structure::{QvdFieldHeader, QvdTableHeader};
use std::{convert::TryInto, fs::File, io::{self, BufReader, Read, BufRead}, path::Path};

pub mod qvd_structure;
pub mod column;



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

fn read_qvd(file_name: impl AsRef<Path>) -> Result<Vec<Column>, QvdError> {
    let file = File::open(&file_name)?;
    let mut reader = BufReader::new(file);
    let xml: String = get_xml_data(&mut reader)?;
    let qvd_structure: QvdTableHeader = from_str(&xml).unwrap();
    let rows_start = qvd_structure.offset;
    
    let mut buf_new = Vec::new();
    reader.read_to_end(&mut buf_new).unwrap();
    let row_section_new = &buf_new[rows_start..buf_new.len()];
    let record_byte_size = qvd_structure.record_byte_size;

    let columns = qvd_structure.fields.headers.iter().map(|field| {
        Column {
            header: Header(field.field_name.clone()),
            symbols: get_column_values(&buf_new, field),
            indexes: get_row_indexes(row_section_new, field, record_byte_size),
        }
    }).collect();

    Ok(columns)

}


fn get_column_values(buf: &[u8], field: &QvdFieldHeader) -> Vec<CellValue> {
    let start = field.offset;
    let end = start + field.length;
    let mut string_start: usize = 0;

    let mut cell_values = Vec::new();

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
                cell_values.push(CellValue::Text(value));
                i += 1;
            }
            1 => {
                // 4 byte integer
                let target_bytes = buf[i + 1..i + 5].to_vec();
                let byte_array: [u8; 4] = target_bytes.try_into().unwrap();
                let numeric_value = i32::from_le_bytes(byte_array);
                cell_values.push(CellValue::Int(numeric_value));
                i += 5;
            }
            2 => {
                // 4 byte double
                let target_bytes = buf[i + 1..i + 9].to_vec();
                let byte_array: [u8; 8] = target_bytes.try_into().unwrap();
                let numeric_value = f64::from_le_bytes(byte_array);
                cell_values.push(CellValue::Float(numeric_value));
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
    cell_values
}

// Retrieve bit stuffed data. Each row has index to value from symbol map.
fn get_row_indexes(buf: &[u8], field: &QvdFieldHeader, record_byte_size: usize) -> Vec<isize> {
    let mut indexes: Vec<isize> = Vec::with_capacity(buf.len() / record_byte_size);
    for chunk in buf.chunks(record_byte_size) {
        let mut chunk = chunk.to_vec();
        chunk.reverse();

        let bits = BitSlice::<Msb0, _>::from_slice(&chunk).unwrap();
        let start = bits.len() - field.bit_offset;
        let end = start - field.bit_width;
        let index = bitslice_to_u32(&bits[end..start]);
        indexes.push(index  + field.bias);
    }
    indexes
}

fn bitslice_to_u32(slice: &BitSlice::<Msb0, u8>) -> isize {
    slice.iter().fold(0, |acc, &bit| (acc << 1) | bit as isize)
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

    use column::{CellValue, Header};

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
        let res = get_column_values(&buf, &field);
        let expected = vec![CellValue::Float(420.0), CellValue::Float(421.0)];
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
        let res = get_column_values(&buf, &field);
        let expected = vec![CellValue::Int(10), CellValue::Int(20)];
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
        let res = get_column_values(&buf, &field);
        let expected = vec![
            CellValue::Float(420.),
            CellValue::Float(421.),
            CellValue::Int(1),
            CellValue::Int(2),
            CellValue::Text("7000".into()),
            CellValue::Text("865.2".into())
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
        let res = get_column_values(&buf, &field);
        let expected = vec![CellValue::Text("example text".into()), CellValue::Text("rust".into())];
        assert_eq!(expected, res);
    }

    #[test]
    #[rustfmt::skip]
    fn test_utf8_string() {
        let buf: Vec<u8> = vec![
            0x04, 0xE4, 0xB9, 0x9F, 0xE6, 0x9C, 0x89, 0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0xE7,
            0xAE, 0x80, 0xE4, 0xBD, 0x93, 0xE5, 0xAD, 0x97, 0x00,
            0x04, 0xF0, 0x9F, 0x90, 0x8D, 0xF0, 0x9F, 0xA6, 0x80, 0x00,
            0x04, 0x54, 0x72, 0xC3, 0xA4, 0x67, 0x65, 0x72, 0x00,
        ];

        let field = QvdFieldHeader {
            length: buf.len(),
            offset: 0,
            field_name: String::new(),
            bias: 0,
            bit_offset: 0,
            bit_width: 0,
        };
        let res = get_column_values(&buf, &field);
        let expected = vec![CellValue::Text("也有中文简体字".into()), CellValue::Text("🐍🦀".into()), CellValue::Text("Träger".into())];
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
        let res = get_column_values(&buf, &field);
        let expected = vec![
            CellValue::Text("example text".into()),
            CellValue::Text("rust".into()),
            CellValue::Text("1234".into()),
            CellValue::Text("double".into()),
        ];
        assert_eq!(expected, res);
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
        let expected: Vec<isize> = vec![5];
        assert_eq!(expected, res);
    }

    #[test]
    fn read_test_file_qvd_null() {
        let result = read_qvd("tests/test_qvd_null.qvd").unwrap();

        let mut expected: Vec<Column> = Vec::new();

        expected.push( Column {
            header: Header("Month".into()),
            symbols: {
                (1..=12).map(|i| {  CellValue::Text(format!("{}", i))}).collect()
            },
            indexes: vec![0,1,2,3,4,5,6,7,8,9,10,11],
        });
        assert_eq!(expected[0], result[0]);

        expected.push( Column {
            header: Header("Quarter".into()),
            symbols: {
                (1..=4).map(|i| {  CellValue::Text(format!("Q{}", i))}).collect()
            },
            indexes: vec![0,0,0,1,1,1,2,2,2,3,3,3],
        });
        assert_eq!(expected[1], result[1]);

        expected.push( Column {
            header: Header("some_null".into()),
            symbols: vec![
                CellValue::Text(1.2.to_string()), 
                CellValue::Text(format!("{:.1}", 10.0)), 
                CellValue::Text(64.to_string()),
                CellValue::Text(1.to_string()),
                CellValue::Text(213.95625.to_string()),
                CellValue::Text(2.to_string()),
                CellValue::Text(3.to_string()),
                CellValue::Text(5.to_string()),
                CellValue::Text(1000.to_string()),
            ],
            indexes: vec![0,1,2,-2,-2,-2,3,4,5,6,7,8],
        });
        assert_eq!(expected[2], result[2]);

        expected.push( Column {
            header: Header("all Null".into()),
            symbols: vec![],
            indexes: vec![-2,-2,-2,-2,-2,-2,-2,-2,-2,-2,-2,-2],
        });
        assert_eq!(expected[3], result[3]);

    
    }

    #[test]
    #[ignore = "manual test"]
    fn read_test_file_columns() {
        use std::io::Write;
        
        let now = Instant::now();
        let result = read_qvd("test/big_file.qvd").unwrap();
        let duration = Instant::now().checked_duration_since(now).unwrap();
        println!("Duration reading: {duration:?}");

        let rows = result[0].indexes.len();
        assert_eq!(rows, 20526);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("output.csv")
            .unwrap();

        let headers =  result.iter().map(|col| col.header.0.as_str()).collect::<Vec<_>>().join(",");
        file.write_all(headers.as_bytes()).unwrap();
        file.write_all(b"\n").unwrap();

        let mut row = Vec::new();
        for i in 0..rows {
            for column in result.iter() {
                let cell_value = column.cell_value(i).unwrap();
                row.push(cell_value)
            }
            let s: String = row.drain(..).map(|cell_value| cell_value.to_string()).collect::<Vec<String>>().join(",");
            file.write_all(s.as_bytes()).unwrap();
            file.write_all(b"\n").unwrap();
        }

        let duration = Instant::now().checked_duration_since(now).unwrap();
        println!("Duration reading: {duration:?}");
        
    }
}
