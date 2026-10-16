/// Validated fields borrowed from the parser's reusable record buffer.
#[derive(Debug)]
pub struct Record<'a> {
    pub line: u64,
    pub run_id: &'a str,
    pub channel: u32,
    pub mux: u32,
    pub start_time: f64,
    pub template_duration: Option<f64>,
    pub passed: bool,
    pub length: u64,
    pub qscore: f64,
    pub end_reason: &'a str,
}
