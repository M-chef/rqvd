use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub struct Column {
    pub(crate) header: Header, 
    pub(crate) symbols: Vec<CellValue>,
    pub(crate) indexes: Vec<isize>,
}

impl Column {
    pub fn into_values(self) -> Vec<CellValue> {
        self.indexes.into_iter().map(|idx| {
            match idx {
                i if i < 0 => { CellValue::Null },
                i => self.symbols.get(i as usize).unwrap().clone(),
            }
        }).collect()
    }

    pub fn cell_value(&self, row_index: usize) -> Option<CellValue> {
        let row_index = self.indexes.get(row_index)?;
        let value = match *row_index {
            i if i < 0 => { CellValue::Null },
            i => self.symbols.get(i as usize).unwrap().clone(),
        };
        Some(value)
    }
}


#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Header(pub(crate) String);

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