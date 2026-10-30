use bytes::Bytes;
use parquet::{
    basic::{Repetition, Type},
    column::reader::ColumnReader,
    file::reader::{FileReader, RowGroupReader, SerializedFileReader},
};

use super::{Testcase, Testcases};

pub struct Parser;

impl Parser {
    pub fn parse(input: Vec<u8>) -> Result<Testcases, String> {
        let reader = SerializedFileReader::new(Bytes::from(input))
            .map_err(|e| format!("Failed to open Parquet test cases: {e}"))?;
        let fields = reader.metadata().file_metadata().schema().get_fields();
        if fields.len() < 2 {
            return Err("Expected input and output columns".into());
        }
        for (field, name) in fields.iter().zip(["input", "output"]) {
            let info = field.get_basic_info();
            if field.name() != name
                || !field.is_primitive()
                || field.get_physical_type() != Type::BYTE_ARRAY
                || !info.has_repetition()
                || info.repetition() != Repetition::REQUIRED
            {
                return Err(format!("Expected required BYTE_ARRAY column '{name}'"));
            }
        }

        let mut testcases = Vec::new();
        testcases.reserve(reader.num_row_groups());
        for index in 0..reader.num_row_groups() {
            let group = reader
                .get_row_group(index)
                .map_err(|e| format!("Failed to read row group {index}: {e}"))?;
            let inputs = Self::read_column(group.as_ref(), 0)
                .map_err(|e| format!("Row group {index}, input: {e}"))?;
            let outputs = Self::read_column(group.as_ref(), 1)
                .map_err(|e| format!("Row group {index}, output: {e}"))?;
            testcases.extend(
                inputs
                    .into_iter()
                    .zip(outputs)
                    .map(|(input, answer)| Testcase { input, answer }),
            );
        }
        Ok(testcases)
    }

    fn read_column(group: &dyn RowGroupReader, index: usize) -> Result<Vec<String>, String> {
        let expected_rows = usize::try_from(group.metadata().num_rows())
            .map_err(|_| "Invalid row count".to_string())?;
        let ColumnReader::ByteArrayColumnReader(mut column) =
            group.get_column_reader(index).map_err(|e| e.to_string())?
        else {
            return Err("Expected a BYTE_ARRAY column".into());
        };
        let mut result = Vec::new();
        let mut values = Vec::new();
        loop {
            values.clear();
            let (records, count, _) = column
                .read_records(1024, None, None, &mut values)
                .map_err(|e| e.to_string())?;
            if records != count || count != values.len() {
                return Err("Each row must contain a non-null value".into());
            }
            if records == 0 {
                break;
            }
            if records > expected_rows.saturating_sub(result.len()) {
                return Err("Column contains more values than the row count".into());
            }
            for value in &values {
                let text = std::str::from_utf8(value.data())
                    .map_err(|e| format!("Row {} is not valid UTF-8: {e}", result.len()))?;
                result.push(text.to_owned());
            }
        }
        if result.len() != expected_rows {
            return Err(format!(
                "Expected {expected_rows} values, found {}",
                result.len()
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parquet::{
        basic::Compression,
        data_type::{ByteArray, ByteArrayType},
        file::{properties::WriterProperties, writer::SerializedFileWriter},
        schema::parser::parse_message_type,
    };

    use super::Parser;

    const SCHEMA: &str = "message test_cases {
        required binary input;
        required binary output;
    }";

    fn parquet_file(schema: &str, groups: &[Vec<(&[u8], &[u8])>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let schema = Arc::new(parse_message_type(schema).unwrap());
        let column_count = schema.get_fields().len();
        let props = Arc::new(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        );
        let mut writer = SerializedFileWriter::new(&mut bytes, schema, props).unwrap();
        for rows in groups {
            let mut group = writer.next_row_group().unwrap();
            for index in 0..column_count {
                let values: Vec<_> = rows
                    .iter()
                    .map(|(input, output)| {
                        ByteArray::from(match index {
                            0 => *input,
                            1 => *output,
                            _ => b"ignored",
                        })
                    })
                    .collect();
                let mut column = group.next_column().unwrap().unwrap();
                column
                    .typed::<ByteArrayType>()
                    .write_batch(&values, None, None)
                    .unwrap();
                column.close().unwrap();
            }
            group.close().unwrap();
        }
        writer.close().unwrap();
        bytes
    }

    #[test]
    fn parses_testcases() {
        let bytes = parquet_file(SCHEMA, &[vec![(b"1 2", b"3"), (b"100 200", b"300")]]);
        let cases = Parser::parse(bytes).unwrap();
        let actual: Vec<_> = cases
            .iter()
            .map(|case| (case.input.as_str(), case.answer.as_str()))
            .collect();
        assert_eq!(actual, vec![("1 2", "3"), ("100 200", "300")]);
    }

    #[test]
    #[should_panic(expected = "Expected input and output columns")]
    fn rejects_missing_output_field() {
        let schema = "message test_cases { required binary input; }";
        Parser::parse(parquet_file(schema, &[])).unwrap();
    }
}
