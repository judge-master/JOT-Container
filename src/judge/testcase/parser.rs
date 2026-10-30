pub struct Parser;
use super::{Testcase, Testcases};

impl Parser {
    pub fn parse(input: Vec<u8>) -> Result<Testcases, String> {
        return Ok(vec![
            Testcase {
                input: "1 2".into(),
                answer: "3".into(),
            },
            Testcase {
                input: "100 200".into(),
                answer: "300".into(),
            },
        ]);
    }
}
