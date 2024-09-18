use std::{fmt::Display, path::Path};


use crate::{error::QvdError, reader::read_qvd};

#[cfg(test)]
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

#[derive(Debug)]
pub struct QvdDocument {
    columns: Vec<Column>,
}

impl QvdDocument {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, QvdError> {
        let columns = read_qvd(path.as_ref())?;
        Ok(Self { columns })
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn rows(&self) -> RowIter {
        let values: Vec<_> = self.columns()
            .iter()
            .map(|col| {
                col.as_values()
            })
            .collect();
        let rows_total = values[0].len();
        RowIter {
            values,
            rows_total,
            index: 0,
        }
    }

    #[cfg(test)]
    pub fn rows_par(&self) -> RowIter {
        let values: Vec<_> = self.columns()
            .par_iter()
            .map(|col| {
                col.as_values()
            })
            .collect();
        let rows_total = values[0].len();
        RowIter {
            values,
            rows_total,
            index: 0,
        }
    }

    #[cfg(test)]
    pub fn rows_alt(&self) -> RowIterAlt {
        RowIterAlt {
            columns: self.columns(),
            index: 0,
        }
    }

    pub fn find_row_indexes(&self, column_name: impl AsRef<str>, value: impl Into<CellValue>) -> Vec<usize> {
        self.columns.iter()
            .find(|col| col.header.0 == column_name.as_ref())
            .map(|col| col.find_row_indexes(value))
            .unwrap_or_default()
    }

    pub fn rows_by_indexes<'a>(&'a self, row_indexes: &'a [usize]) -> RowIter {
        let values: Vec<_> = self.columns()
            .iter()
            .map(|col| {
                col.indexes_to_values(row_indexes)
            })
            .collect();

        let rows_total = values[0].len();
        RowIter {
            values,
            rows_total,
            index: 0
        }
    }
}

pub struct RowIter<'a> {
    values: Vec<Vec<&'a CellValue>>,
    rows_total: usize,
    index: usize
}

impl<'a, 'b: 'a> Iterator for RowIter<'a> {
    type Item = Vec<&'a CellValue>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.rows_total {
            let row: Vec<_> = self.values.iter()
                .map(|col| *col.get(self.index).unwrap())
                .collect();
            self.index += 1;
            Some(row)

        } else {
            None
        }
    }
}

#[cfg(test)]
pub struct RowIterAlt<'a> {
    columns: &'a [Column],
    index: usize
}

#[cfg(test)]
impl<'a, 'b: 'a> Iterator for RowIterAlt<'a> {
    type Item = Vec<&'a CellValue>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.columns[0].indexes.len() {
            let row: Vec<_> = self.columns.iter()
                .flat_map(|col| col.indexes_to_values(&[self.index]))
                .collect();
            self.index += 1;
            Some(row)
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Column {
    pub(crate) header: Header, 
    pub(crate) symbols: Vec<CellValue>,
    pub(crate) indexes: Vec<isize>,
}

impl Column {

    pub fn header(&self) -> Header {
        self.header.clone()
    }

    pub fn as_values(&self) -> Vec<&CellValue> {
        self.indexes.iter().map(|&idx| {
            match idx {
                i if i < 0 => { &CellValue::Null },
                i => self.symbols.get(i as usize).unwrap(),
            }
        }).collect()
    }

    pub fn into_values(self) -> Vec<CellValue> {
        self.indexes.into_iter().map(|idx| {
            match idx {
                i if i < 0 => { CellValue::Null },
                i => self.symbols.get(i as usize).unwrap().clone(),
            }
        }).collect()
    }

    // pub fn value_from_row_index(&self, row_index: usize) -> Option<CellValue> {
    //     let row_index = self.indexes.get(row_index)?;
    //     let value = match *row_index {
    //         i if i < 0 => { CellValue::Null },
    //         i => self.symbols.get(i as usize).unwrap().clone(),
    //     };
    //     Some(value)
    // }

    pub fn indexes_to_values(&self, row_indexes: &[usize]) -> Vec<&CellValue> {
        row_indexes.iter().map(|&idx| {
            match self.indexes.get(idx) {
                Some(&i) if i < 0 => { &CellValue::Null },
                Some(&i) => self.symbols.get(i as usize).unwrap(),
                None => { &CellValue::Null }
            }
            
        }).collect()
    }

    pub fn find_row_indexes(&self, value: impl Into<CellValue>) -> Vec<usize> {
        let cell_value = value.into();
        let rows: Vec<_> = self.symbols.iter()
            .enumerate()
            .filter(|(_, elem)| **elem == cell_value)
            .map(|(symbol_idx, _)| symbol_idx as isize)
            .collect();

        self.indexes.iter()
            .enumerate()
            .filter(|(_, symbol_idx)| rows.contains(symbol_idx))
            .map(|(idx, _)| idx)
            .collect()
    }

}

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Header(pub(crate) String);

impl From<&str> for Header {
    fn from(value: &str) -> Self {
        Header(value.into())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CellValue {
    Text(String),
    Int(i32),
    Float(f64),
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

impl From<&str> for CellValue {
    fn from(value: &str) -> Self {
        CellValue::Text(value.into())
    }
}

impl From<i32> for CellValue {
    fn from(value: i32) -> Self {
        CellValue::Int(value)
    }
}

impl From<f64> for CellValue {
    fn from(value: f64) -> Self {
        CellValue::Float(value)
    }
}


#[cfg(test)]
mod tests {

    use super::*;
    
    #[test]
    fn test_row_indexes_for_string() {
        let column =  Column {
            header: Header("Quarter".into()),
            symbols: {
                (1..=4).map(|i| {  CellValue::Text(format!("Q{}", i))}).collect()
            },
            indexes: vec![0,0,0,1,-2,1,2,2,2,3,3,3],
        };
        let row_indexes = column.find_row_indexes("Q2");
        assert_eq!(row_indexes, vec![3, 5]);
    }

    #[test]
    fn test_row_indexes_for_int() {
        let column = Column {
            header: Header("Integer".into()),
            symbols: {
                (1..=12).map(|i| {  CellValue::Int(i) }).collect()
            },
            indexes: vec![0,1,2,3,4,5,6,6,8,9,10,11],
        };
        let row_indexes = column.find_row_indexes(7);
        assert_eq!(row_indexes, vec![6,7]);
    }

    #[test]
    fn test_row_indexes_for_float() {
        let column = Column {
            header: Header("Float".into()),
            symbols: {
                (1..=12).map(|i| {  CellValue::Float(i as f64) }).collect()
            },
            indexes: vec![0,1,2,3,4,5,6,6,8,9,10,11],
        };
        let row_indexes = column.find_row_indexes(7.);
        assert_eq!(row_indexes, vec![6,7]);
    }

    #[test]
    fn test_value_from_row_index() {
        let column = Column {
            header: Header("Float".into()),
            symbols: {
                (1..=12).map(|i| {  CellValue::Float(i as f64) }).collect()
            },
            indexes: vec![0,1,2,3,4,5,6,7,8,9,10,11],
        };
        let value = column.indexes_to_values(&[3]);
        assert_eq!(*value[0], CellValue::Float(4.));
    }

    #[test]
    fn test_qvd_document_rows() {
        let doc = QvdDocument::read("tests/test_file.qvd").unwrap();
        let mut rows = doc.rows();
        let expected = [1.into(), "Q1".into(), 1.1.into(), 1.2.into(), CellValue::Null];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));

        let expected = [2.into(), "Q1".into(), 2.2.into(), 10.0.into(), CellValue::Null];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));
    }

    #[test]
    fn qvd_document_test() {
        let doc = QvdDocument::read("tests/test_file.qvd").unwrap();
        let row_indexes = doc.find_row_indexes("all_string", "Q1");
        let mut rows = doc.rows_by_indexes(&row_indexes);
        let expected = [1.into(), "Q1".into(), 1.1.into(), 1.2.into(), CellValue::Null];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));
        assert!(rows.next().is_some());
        assert!(rows.next().is_some());
        assert!(rows.next().is_none());
    }
    
}