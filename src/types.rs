use std::{fmt::Display, path::Path, sync::Arc};

use crate::{error::QvdError, reader::read_qvd};

use rayon::iter::{
    plumbing::UnindexedProducer, IndexedParallelIterator, IntoParallelIterator,
    IntoParallelRefIterator, ParallelIterator,
};
// #[cfg(test)]
// use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
// use serde::de::value;

#[derive(Debug)]
pub struct QvdDocument {
    columns: Vec<Column>,
}

impl QvdDocument {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, QvdError> {
        let columns = read_qvd(path.as_ref())?;
        Ok(Self { columns })
    }

    pub fn columns(&self) -> ColumnIter {
        ColumnIter {
            columns: &self.columns,
            current_index: 0,
        }
    }

    pub fn rows(&self) -> RowIter {
        let values: Vec<_> = self
            .columns()
            .into_par_iter()
            .map(|col| col.as_values())
            .collect();
        let rows_total = values[0].len();
        RowIter {
            values,
            rows_total,
            index: 0,
        }
    }

    // #[cfg(test)]
    // pub fn rows_par(&self) -> RowIter {
    //     let values: Vec<_> = self
    //         .columns()
    //         .par_iter()
    //         .map(|col| col.as_values())
    //         .collect();
    //     let rows_total = values[0].len();
    //     RowIter {
    //         values,
    //         rows_total,
    //         index: 0,
    //     }
    // }

    // #[cfg(test)]
    // pub fn rows_alt(&self) -> RowIterAlt {
    //     RowIterAlt {
    //         columns: self.columns(),
    //         index: 0,
    //     }
    // }

    /// Search row indexes in table for a given column name and cell value
    pub fn find_row_indexes(
        &self,
        column_name: impl Into<Arc<str>>,
        value: impl Into<CellValue>,
    ) -> Vec<usize> {
        let column_name = column_name.into();
        self.columns
            .par_iter()
            .find_first(|col| col.header.0 == *column_name)
            .map(|col| col.find_row_indexes(value))
            .unwrap_or_default()
    }

    /// Return an iterator over rows for given indexes
    pub fn rows_by_indexes<'a>(&'a self, row_indexes: &'a [usize]) -> RowIter<'a> {
        let values: Vec<_> = self
            .columns()
            .into_par_iter()
            .map(|col| col.indexes_to_values(row_indexes))
            .collect();

        let rows_total = values[0].len();
        RowIter {
            values,
            rows_total,
            index: 0,
        }
    }
}

pub struct ColumnIter<'a> {
    columns: &'a [Column],
    current_index: usize,
}

impl<'a> ColumnIter<'a> {
    /// get the underlying slice
    pub fn as_slice(&self) -> &'a [Column] {
        self.columns
    }
}

impl<'a> Iterator for ColumnIter<'a> {
    type Item = &'a Column;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.columns.len() {
            let item = &self.columns[self.current_index];
            self.current_index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<'a> IntoParallelIterator for ColumnIter<'a> {
    type Item = &'a Column;

    type Iter = ParallelColumnIter<'a>;

    fn into_par_iter(self) -> Self::Iter {
        ParallelColumnIter {
            columns: self.columns,
        }
    }
}

pub struct ParallelColumnIter<'a> {
    columns: &'a [Column],
}

impl<'a> ParallelIterator for ParallelColumnIter<'a> {
    type Item = &'a Column;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        let producer = ColumnProducer {
            columns: self.columns,
            start: 0,
            end: self.columns.len(),
        };
        rayon::iter::plumbing::bridge_unindexed(producer, consumer)
    }
}

pub struct ColumnProducer<'a> {
    columns: &'a [Column],
    start: usize,
    end: usize,
}

impl<'a> UnindexedProducer for ColumnProducer<'a> {
    type Item = &'a Column;

    fn split(self) -> (Self, Option<Self>) {
        let len = self.end - self.start;
        if len <= 1 {
            // If there's 0 or 1 element, no more splitting
            return (self, None);
        }

        let mid = self.start + len / 2;
        let left = ColumnProducer {
            columns: self.columns,
            start: self.start,
            end: mid,
        };
        let right = ColumnProducer {
            columns: self.columns,
            start: mid,
            end: self.end,
        };

        (left, Some(right))
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        let mut folder = folder;
        for i in self.start..self.end {
            folder = folder.consume(&self.columns[i]);
            if folder.full() {
                break;
            }
        }
        folder
    }
}

pub struct RowIter<'a> {
    values: Vec<Vec<&'a CellValue>>,
    rows_total: usize,
    index: usize,
}

impl<'a, 'b: 'a> Iterator for RowIter<'a> {
    type Item = Vec<&'a CellValue>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.rows_total {
            let row: Vec<_> = self
                .values
                .par_iter()
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
    index: usize,
}

#[cfg(test)]
impl<'a, 'b: 'a> Iterator for RowIterAlt<'a> {
    type Item = Vec<&'a CellValue>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.columns[0].indexes.len() {
            let row: Vec<_> = self
                .columns
                .iter()
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
        self.indexes
            .par_iter()
            .map(|&idx| match idx {
                i if i < 0 => &CellValue::Null,
                i => self.symbols.get(i as usize).unwrap(),
            })
            .collect()
    }

    pub fn into_values(self) -> Vec<CellValue> {
        self.indexes
            .into_par_iter()
            .map(|idx| match idx {
                i if i < 0 => CellValue::Null,
                i => self.symbols.get(i as usize).unwrap().clone(),
            })
            .collect()
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
        row_indexes
            .par_iter()
            .map(|&idx| match self.indexes.get(idx) {
                Some(&i) if i < 0 => &CellValue::Null,
                Some(&i) => self.symbols.get(i as usize).unwrap(),
                None => &CellValue::Null,
            })
            .collect()
    }

    pub fn find_row_indexes(&self, value: impl Into<CellValue>) -> Vec<usize> {
        let cell_value = value.into();
        let rows: Vec<_> = self
            .symbols
            .par_iter()
            .enumerate()
            .filter(|(_, elem)| **elem == cell_value)
            .map(|(symbol_idx, _)| symbol_idx as isize)
            .collect();

        self.indexes
            .par_iter()
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

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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

impl CellValue {
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            CellValue::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            CellValue::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            CellValue::Text(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parallel_colum_iter() {
        let columns = vec![
            Column {
                header: "C1".into(),
                symbols: { (1..=12).map(|i| CellValue::Int(i)).collect() },
                indexes: vec![0, 1, 2, 3, 4, 5, 6, 6, 8, 9, 10, 11],
            },
            Column {
                header: "C2".into(),
                symbols: { (1..=12).map(|i| CellValue::Float(i as f64)).collect() },
                indexes: vec![0, 1, 2, 3, 4, 5, 6, 6, 8, 9, 10, 11],
            },
            Column {
                header: "C3".into(),
                symbols: { (1..=12).map(|i| CellValue::Text(format!("{i}"))).collect() },
                indexes: vec![0, 1, 2, 3, 4, 5, 6, 6, 8, 9, 10, 11],
            },
        ];
        let iter = ColumnIter {
            columns: &columns,
            current_index: 0,
        };
        iter.into_par_iter().for_each(|col| {
            println!("{}", col.header);
        })
    }

    #[test]
    fn test_row_indexes_for_string() {
        let column = Column {
            header: Header("Quarter".into()),
            symbols: {
                (1..=4)
                    .map(|i| CellValue::Text(format!("Q{}", i)))
                    .collect()
            },
            indexes: vec![0, 0, 0, 1, -2, 1, 2, 2, 2, 3, 3, 3],
        };
        let row_indexes = column.find_row_indexes("Q2");
        assert_eq!(row_indexes, vec![3, 5]);
    }

    #[test]
    fn test_row_indexes_for_int() {
        let column = Column {
            header: Header("Integer".into()),
            symbols: { (1..=12).map(|i| CellValue::Int(i)).collect() },
            indexes: vec![0, 1, 2, 3, 4, 5, 6, 6, 8, 9, 10, 11],
        };
        let row_indexes = column.find_row_indexes(7);
        assert_eq!(row_indexes, vec![6, 7]);
    }

    #[test]
    fn test_row_indexes_for_float() {
        let column = Column {
            header: Header("Float".into()),
            symbols: { (1..=12).map(|i| CellValue::Float(i as f64)).collect() },
            indexes: vec![0, 1, 2, 3, 4, 5, 6, 6, 8, 9, 10, 11],
        };
        let row_indexes = column.find_row_indexes(7.);
        assert_eq!(row_indexes, vec![6, 7]);
    }

    #[test]
    fn test_value_from_row_index() {
        let column = Column {
            header: Header("Float".into()),
            symbols: { (1..=12).map(|i| CellValue::Float(i as f64)).collect() },
            indexes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        };
        let value = column.indexes_to_values(&[3]);
        assert_eq!(*value[0], CellValue::Float(4.));
    }

    #[test]
    fn test_qvd_document_rows() {
        let doc = QvdDocument::read("tests/test_file.qvd").unwrap();
        let mut rows = doc.rows();
        let expected = [
            1.into(),
            "Q1".into(),
            1.1.into(),
            1.2.into(),
            CellValue::Null,
        ];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));

        let expected = [
            2.into(),
            "Q1".into(),
            2.2.into(),
            10.0.into(),
            CellValue::Null,
        ];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));
    }

    #[test]
    fn qvd_document_test() {
        let doc = QvdDocument::read("tests/test_file.qvd").unwrap();
        let row_indexes = doc.find_row_indexes("all_string", "Q1");
        let mut rows = doc.rows_by_indexes(&row_indexes);
        let expected = [
            1.into(),
            "Q1".into(),
            1.1.into(),
            1.2.into(),
            CellValue::Null,
        ];
        let expected: Vec<_> = expected.iter().collect();
        assert_eq!(rows.next(), Some(expected));
        assert!(rows.next().is_some());
        assert!(rows.next().is_some());
        assert!(rows.next().is_none());
    }

    #[test]
    fn test_as_int_success() {
        let cell = CellValue::Int(1);
        assert_eq!(cell.as_i32(), Some(1));
    }

    #[test]
    fn test_as_int_failure() {
        let cell = CellValue::Float(1.);
        assert_eq!(cell.as_i32(), None);
        let cell = CellValue::Text("text".into());
        assert_eq!(cell.as_i32(), None);
        let cell = CellValue::Null;
        assert_eq!(cell.as_i32(), None);
    }

    #[test]
    fn test_as_float_success() {
        let cell = CellValue::Float(1.);
        assert_eq!(cell.as_f64(), Some(1.));
    }

    #[test]
    fn test_as_float_failure() {
        let cell = CellValue::Int(1);
        assert_eq!(cell.as_f64(), None);
        let cell = CellValue::Text("text".into());
        assert_eq!(cell.as_f64(), None);
        let cell = CellValue::Null;
        assert_eq!(cell.as_f64(), None);
    }

    #[test]
    fn test_as_str_success() {
        let cell = CellValue::Text("test".into());
        assert_eq!(cell.as_str(), Some("test"));
    }

    #[test]
    fn test_as_str_failure() {
        let cell = CellValue::Int(1);
        assert_eq!(cell.as_str(), None);
        let cell = CellValue::Float(1.);
        assert_eq!(cell.as_str(), None);
        let cell = CellValue::Null;
        assert_eq!(cell.as_str(), None);
    }
}
