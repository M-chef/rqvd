#![cfg_attr(RUSTC_IS_NIGHTLY, feature(test))]

pub(crate) mod qvd_structure;
pub mod types;
pub(crate) mod reader;
pub mod error;

pub use types::{QvdDocument, Header, Column, CellValue};

    

#[cfg(all(RUSTC_IS_NIGHTLY, test))]
mod tests {
    use crate::QvdDocument;

    extern crate test;


    #[bench]
    fn read_test_file_to_row_iter(b: &mut test::Bencher) {        
        let qvd = QvdDocument::read("tests/big_file.qvd").unwrap();
        b.iter(|| {
            let mut rows = qvd.rows();
            while let Some(row) = rows.next() {
                
            }
        })
    }

    #[bench]
    fn read_test_file_to_row_iter_par(b: &mut test::Bencher) {        
        let qvd = QvdDocument::read("tests/big_file.qvd").unwrap();
        b.iter(|| {
            let mut rows = qvd.rows_par();
            while let Some(row) = rows.next() {
                
            }
        })
    }

    #[bench]
    fn read_test_file_to_row_iter_alt(b: &mut test::Bencher) {        
        let qvd = QvdDocument::read("tests/big_file.qvd").unwrap();
        b.iter(|| {
            let mut rows = qvd.rows_alt();
            while let Some(row) = rows.next() {
                
            }
        })
    }

}