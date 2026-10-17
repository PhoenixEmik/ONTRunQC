use std::{
    collections::BTreeSet,
    io::{self, Read},
    str::FromStr,
};

use anyhow::{Context, Result, bail, ensure};
use csv::{ByteRecord, Reader, ReaderBuilder, StringRecord};

use crate::record::Record;

const REQUIRED: [&str; 8] = [
    "run_id",
    "channel",
    "mux",
    "start_time",
    "duration",
    "sequence_length_template",
    "mean_qscore_template",
    "end_reason",
];

struct Columns {
    required: [usize; REQUIRED.len()],
    template_duration: Option<usize>,
    passes_filtering: Option<usize>,
    flow_cell_id: Option<usize>,
}

impl Columns {
    fn new(headers: &StringRecord) -> Result<Self> {
        ensure!(
            !headers.is_empty(),
            "line 1: empty input; expected a TSV header"
        );
        let mut seen = BTreeSet::new();
        for header in headers {
            ensure!(!header.is_empty(), "line 1: empty column name");
            ensure!(seen.insert(header), "line 1: duplicate column {header:?}");
        }
        let mut positions = [0; REQUIRED.len()];
        for (index, name) in REQUIRED.iter().enumerate() {
            positions[index] = headers
                .iter()
                .position(|header| header == *name)
                .with_context(|| format!("line 1: missing required column {name:?}"))?;
        }
        Ok(Self {
            required: positions,
            template_duration: headers.iter().position(|h| h == "template_duration"),
            passes_filtering: headers.iter().position(|h| h == "passes_filtering"),
            flow_cell_id: headers.iter().position(|h| h == "flow_cell_id"),
        })
    }

    fn parse<'a>(&self, row: &'a ByteRecord, line: u64) -> Result<Record<'a>> {
        let field = |index: usize| -> Result<&'a str> {
            std::str::from_utf8(&row[self.required[index]])
                .with_context(|| format!("column {:?}: expected UTF-8", REQUIRED[index]))
        };
        let run_id = field(0)?;
        ensure!(
            !run_id.trim().is_empty(),
            "column \"run_id\": must not be empty"
        );
        let channel = number::<u32>(field(1)?, REQUIRED[1])?;
        ensure!(channel > 0, "column \"channel\": must be positive");
        let mux = number::<u32>(field(2)?, REQUIRED[2])?;
        ensure!(mux > 0, "column \"mux\": must be positive");
        let start_time = nonnegative_finite(field(3)?, REQUIRED[3])?;
        // Validate duration even though this milestone attributes yield to start time.
        nonnegative_finite(field(4)?, REQUIRED[4])?;
        let passed = self
            .passes_filtering
            .map(|i| -> Result<bool> {
                let value = std::str::from_utf8(&row[i])
                    .context("column \"passes_filtering\": expected UTF-8")?;
                match value {
                    v if v.eq_ignore_ascii_case("true") => Ok(true),
                    v if v.eq_ignore_ascii_case("false") => Ok(false),
                    v => bail!("column \"passes_filtering\": expected TRUE or FALSE, found {v:?}"),
                }
            })
            .transpose()?;
        let length = number(field(5)?, REQUIRED[5])?;
        let qscore = nonnegative_finite(field(6)?, REQUIRED[6])?;
        let end_reason = field(7)?;
        ensure!(
            !end_reason.trim().is_empty(),
            "column \"end_reason\": must not be empty"
        );
        Ok(Record {
            line,
            run_id,
            flow_cell_id: self
                .flow_cell_id
                .map(|i| -> Result<&str> {
                    let value = std::str::from_utf8(&row[i])
                        .context("column flow_cell_id: expected UTF-8")?;
                    ensure!(
                        !value.trim().is_empty(),
                        "column flow_cell_id: must not be empty"
                    );
                    Ok(value)
                })
                .transpose()?,
            channel,
            mux,
            start_time,
            // Optional diagnostics never replace validation of required fields.
            // Blank, malformed, nonpositive, or nonfinite durations are ineligible.
            template_duration: self.template_duration.and_then(|i| {
                std::str::from_utf8(&row[i])
                    .ok()?
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
            }),
            passed,
            length,
            qscore,
            end_reason,
        })
    }
}

fn number<T: FromStr>(value: &str, column: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("column {column:?}: invalid numeric value {value:?}"))
}

fn nonnegative_finite(value: &str, column: &str) -> Result<f64> {
    let parsed = number::<f64>(value, column)?;
    ensure!(
        parsed.is_finite() && parsed >= 0.0,
        "column {column:?}: expected a finite nonnegative number, found {value:?}"
    );
    Ok(parsed)
}

// csv deliberately ignores empty lines. Check them in the input stream so a
// malformed blank record is reported at its actual physical line, even at EOF.
struct NonblankLines<R> {
    source: R,
    line: u64,
    has_content: bool,
    partial_line: bool,
}

impl<R: Read> Read for NonblankLines<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let count = self.source.read(buffer)?;
        for part in buffer[..count].split_inclusive(|byte| *byte == b'\n') {
            let ended = part.ends_with(b"\n");
            let content = if ended { &part[..part.len() - 1] } else { part };
            self.has_content |= content.iter().any(|byte| *byte != b'\r');
            self.partial_line = !ended;
            if ended {
                if !self.has_content {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("line {}: blank TSV lines are not allowed", self.line),
                    ));
                }
                self.line += 1;
                self.has_content = false;
            }
        }
        if count == 0 && self.partial_line && !self.has_content {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: blank TSV lines are not allowed", self.line),
            ));
        }
        Ok(count)
    }
}

pub struct SummaryReader<R: Read> {
    reader: Reader<NonblankLines<R>>,
    columns: Columns,
    buffer: ByteRecord,
    run_id: Option<String>,
    next_line: u64,
    enforce_single_run: bool,
}

impl<R: Read> SummaryReader<R> {
    pub fn new(source: R) -> Result<Self> {
        // MinKNOW summaries are plain TSV: quotes are literal, not CSV escapes.
        let mut reader = ReaderBuilder::new()
            .delimiter(b'\t')
            .quoting(false)
            .buffer_capacity(64 * 1024)
            .from_reader(NonblankLines {
                source,
                line: 1,
                has_content: false,
                partial_line: false,
            });
        let headers = reader
            .headers()
            .context("cannot read TSV header at line 1")?;
        let columns = Columns::new(headers)?;
        Ok(Self {
            reader,
            columns,
            buffer: ByteRecord::new(),
            run_id: None,
            next_line: 2,
            enforce_single_run: true,
        })
    }

    #[cfg(test)]
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    pub fn allow_multiple_runs(&mut self) {
        self.enforce_single_run = false;
    }

    pub fn has_native_filtering(&self) -> bool {
        self.columns.passes_filtering.is_some()
    }

    pub fn has_template_duration(&self) -> bool {
        self.columns.template_duration.is_some()
    }

    pub fn next_record(&mut self) -> Result<Option<Record<'_>>> {
        let has_record = self
            .reader
            .read_byte_record(&mut self.buffer)
            .with_context(|| format!("line {}: malformed TSV record", self.next_line))?;
        if !has_record {
            return Ok(None);
        }
        // With blank and multiline records excluded, each row is one physical
        // line. csv positions can lag when CRLF is split across short reads.
        let line = self.next_line;
        self.next_line += 1;
        let record = self
            .columns
            .parse(&self.buffer, line)
            .with_context(|| format!("line {line}"))?;
        if let Some(run_id) = &self.run_id {
            ensure!(
                !self.enforce_single_run || run_id == record.run_id,
                "line {line}: inconsistent run_id {:?}; expected {:?}; use --split-runs to analyze multiple runs",
                record.run_id,
                run_id
            );
        } else {
            self.run_id = Some(record.run_id.to_owned());
        }
        Ok(Some(record))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SmallChunks<'a> {
        bytes: &'a [u8],
        chunk_size: usize,
    }

    impl Read for SmallChunks<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let length = buffer.len().min(self.chunk_size);
            self.bytes.read(&mut buffer[..length])
        }
    }

    #[test]
    fn short_reads_and_crlf_boundaries_preserve_records() {
        let text = include_str!("../tests/fixtures/reordered.tsv").replace('\n', "\r\n");
        for chunk_size in [1, 2, 7, 31] {
            let mut reader = SummaryReader::new(SmallChunks {
                bytes: text.as_bytes(),
                chunk_size,
            })
            .unwrap();
            let record = reader.next_record().unwrap().unwrap();
            assert_eq!(record.line, 2);
            assert_eq!(record.length, 9);
            assert_eq!(record.qscore, 12.125);
            assert_eq!(record.end_reason, "unknown_new_reason");
            assert!(reader.next_record().unwrap().is_none());
            assert_eq!(reader.run_id(), Some("synthetic-run"));
        }
    }

    #[test]
    fn blank_line_detection_crosses_buffer_boundaries() {
        for text in ["valid\r\n\r\n", "valid\n\nlast", "valid\n\r", "\nheader"] {
            for chunk_size in [1, 2, 7] {
                let mut source = NonblankLines {
                    source: SmallChunks {
                        bytes: text.as_bytes(),
                        chunk_size,
                    },
                    line: 1,
                    has_content: false,
                    partial_line: false,
                };
                let error = source.read_to_end(&mut Vec::new()).unwrap_err();
                assert!(error.to_string().contains("blank TSV lines"));
            }
        }
    }
}
