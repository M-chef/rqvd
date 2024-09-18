# Read Qlik Sense .qvd files 🛠
This is a fork of the python library to read qvd's in python but modified to be used as a rust lib.

Original repo: https://github.com/SBentley/qvd-utils

## Usage

The main struct to read files and process content is `QvdDocument`. This can be used to access columns or to return data row wise.

```
use rqvd::QvdDocument;

fn main() {

    // read a .qvd file
    let doc = rqvd::QvdDocument::read("file.qvd").unwrap();
    
    // get all values of first column
    let column_values = doc.columns().first().unwrap().as_values();

    // write data to csv file
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("export.csv")
        .unwrap();
    // iter over rows
    for row in doc.rows() {
        let row_str = row.iter().map(|cell| cell.to_string()).collect::<Vec<_>>().join(",");
        file.write_all(row_str.as_bytes()).unwrap();
        file.write_all(b"\n").unwrap();
    }

}

```



## Notes

While based on and heavily inspired by the original code, reading performance of .qvd files is improved by reusing buffer and using rayon for parallel processing. Currently columns are processed in parallel, so tables with more columns benefit more from parallel processing.

The internal layout when reading a file to memory is kept in columnar representation to mirror the datalayout of .qvd files. Also the structure of symbol table and index map is used. 

## Todos

| Priority   | Task                                          |
| :--------: | --------------------------------------------- |
| Low        | Reimplement python support                    | 
| Medium     | Implement more efficient in memory data model |
| Medium     | struct and function documentation             |

## QVD File Structure

A QVD file is split into 3 parts; XML Metdata, Symbols table and the bit
stuffed binary indexes.

### XML Metadata

This section is at the top of the file and is in human readable XML. This
section contains metadata about the file in gneneral such as table name, number
of records, size of records as well as data about individual fields including
field name, length offset in symbol table.

### Symbol table

Directly after the xml section is the symbol table. This is a table of every
unique value contained within each column. The columns are in the order
described in the metadata fields section. In the metadata we can find the byte
offset from the start of the symbols section for each column. Symbol types
cannot be determined from the metadata and are instead determined by a flag
byte preceding each symbol. These types are:

* 1 - 4 byte signed int (u32) - little endiand
* 2 - 8 byte signed float (f64) - little endian
* 4 - null terminated string
* 5 - 4 bytes of junk follwed by a null terminated string representing an integer
* 6 - 8 bytes of junk followed by a null terminated string representing a float

### Binary Indexes

After the symbol table are the binary indexes that map to the symbols for each
row. They are bit stuffed and reversed binary numbers that point to the index
of the symbol in the symbols table for each field.
