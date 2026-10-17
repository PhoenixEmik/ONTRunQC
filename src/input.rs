use std::{
    collections::BTreeSet,
    io::{self, Read},
    str::FromStr,
};

use anyhow::{Context, Result, bail, ensure};
use csv::{ByteRecord, Reader, ReaderBuilder, StringRecord};

use crate::record::Record;

const MODERN_REQUIRED: [&str; 8] = [
    "run_id",
    "channel",
    "mux",
    "start_time",
    "duration",
    "sequence_length_template",
    "mean_qscore_template",
    "end_reason",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSchema {
    Auto,
    Modern,
    Legacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize)]
pub enum ReadRepresentation {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "template")]
    Template,
    #[value(name = "2d")]
    #[serde(rename = "2d")]
    TwoD,
}

struct Columns {
    schema: InputSchema,
    representation: ReadRepresentation,
    channel: usize,
    start_time: usize,
    length: usize,
    qscore: usize,
    run_id: Option<usize>,
    mux: Option<usize>,
    duration: Option<usize>,
    end_reason: Option<usize>,
    template_duration: Option<usize>,
    passes_filtering: Option<usize>,
    flow_cell_id: Option<usize>,
    control: Option<usize>,
    events: Option<usize>,
}

impl Columns {
    fn new(
        headers: &StringRecord,
        requested_schema: InputSchema,
        requested_representation: ReadRepresentation,
    ) -> Result<Self> {
        ensure!(
            !headers.is_empty(),
            "line 1: empty input; expected a TSV header"
        );
        let mut seen = BTreeSet::new();
        for header in headers {
            ensure!(!header.is_empty(), "line 1: empty column name");
            ensure!(seen.insert(header), "line 1: duplicate column {header:?}");
        }
        let find = |name: &str| headers.iter().position(|h| h == name);
        let required = |name: &str| {
            find(name).with_context(|| format!("line 1: missing required column {name:?}"))
        };
        let representation = match requested_representation {
            ReadRepresentation::Auto
                if find("sequence_length_2d").is_some() || find("mean_qscore_2d").is_some() =>
            {
                ReadRepresentation::TwoD
            }
            ReadRepresentation::Auto => ReadRepresentation::Template,
            value => value,
        };
        let schema = match requested_schema {
            InputSchema::Auto
                if representation == ReadRepresentation::Template
                    && MODERN_REQUIRED.iter().all(|name| find(name).is_some()) =>
            {
                InputSchema::Modern
            }
            InputSchema::Auto => InputSchema::Legacy,
            value => value,
        };
        if schema == InputSchema::Modern {
            ensure!(
                representation == ReadRepresentation::Template,
                "modern schema requires template reads; use --input-schema legacy for 2D or --read-representation template"
            );
            for name in MODERN_REQUIRED {
                required(name)?;
            }
        }
        let (length_name, quality_name) = if representation == ReadRepresentation::TwoD {
            ("sequence_length_2d", "mean_qscore_2d")
        } else {
            ("sequence_length_template", "mean_qscore_template")
        };
        Ok(Self {
            schema,
            representation,
            channel: required("channel")?,
            start_time: required("start_time")?,
            length: required(length_name)?,
            qscore: required(quality_name)?,
            run_id: find("run_id"),
            mux: find("mux"),
            duration: find("duration"),
            end_reason: find("end_reason"),
            template_duration: find("template_duration"),
            passes_filtering: find("passes_filtering"),
            flow_cell_id: find("flow_cell_id"),
            control: find("calibration_strand_genome_template"),
            events: find("num_events_template"),
        })
    }

    fn text<'a>(&self, row: &'a ByteRecord, index: usize, name: &str) -> Result<&'a str> {
        std::str::from_utf8(&row[index]).with_context(|| format!("column {name:?}: expected UTF-8"))
    }

    fn optional<'a>(
        &self,
        row: &'a ByteRecord,
        index: Option<usize>,
        name: &str,
    ) -> Result<Option<&'a str>> {
        let value = index.map(|i| self.text(row, i, name)).transpose()?;
        if self.schema == InputSchema::Legacy {
            return Ok(value.filter(|v| !v.trim().is_empty() && *v != "NA"));
        }
        if let Some(value) = value {
            ensure!(
                !value.trim().is_empty(),
                "column {name:?}: must not be empty"
            );
        }
        Ok(value)
    }

    fn parse<'a>(&self, row: &'a ByteRecord, line: u64) -> Result<Record<'a>> {
        let channel = number::<u32>(self.text(row, self.channel, "channel")?, "channel")?;
        ensure!(channel > 0, "column \"channel\": must be positive");
        let mux = self
            .optional(row, self.mux, "mux")?
            .map(|v| number::<u32>(v, "mux"))
            .transpose()?;
        ensure!(
            mux.is_none_or(|v| v > 0),
            "column \"mux\": must be positive"
        );
        let start_time =
            nonnegative_finite(self.text(row, self.start_time, "start_time")?, "start_time")?;
        if let Some(value) = self.optional(row, self.duration, "duration")? {
            nonnegative_finite(value, "duration")?;
        }
        let passed = self
            .optional(row, self.passes_filtering, "passes_filtering")?
            .map(|v| match v {
                v if v.eq_ignore_ascii_case("true") => Ok(true),
                v if v.eq_ignore_ascii_case("false") => Ok(false),
                v => bail!("column \"passes_filtering\": expected TRUE or FALSE, found {v:?}"),
            })
            .transpose()?;
        let (length_name, quality_name) = if self.representation == ReadRepresentation::TwoD {
            ("sequence_length_2d", "mean_qscore_2d")
        } else {
            ("sequence_length_template", "mean_qscore_template")
        };
        let length = number(self.text(row, self.length, length_name)?, length_name)?;
        let qscore = if self.schema == InputSchema::Modern {
            nonnegative_finite(self.text(row, self.qscore, quality_name)?, quality_name)?
        } else {
            let q = number::<f64>(self.text(row, self.qscore, quality_name)?, quality_name)?;
            ensure!(
                q.is_finite(),
                "column {quality_name:?}: expected a finite number"
            );
            q
        };
        let template = self.representation == ReadRepresentation::Template;
        Ok(Record {
            line,
            channel,
            mux,
            start_time,
            length,
            qscore,
            run_id: self.optional(row, self.run_id, "run_id")?,
            flow_cell_id: self.optional(row, self.flow_cell_id, "flow_cell_id")?,
            end_reason: self.optional(row, self.end_reason, "end_reason")?,
            passed: if template { passed } else { None },
            is_control: self.optional(row, self.control, "calibration_strand_genome_template")?
                == Some("YHR174W"),
            num_events_template: if template {
                self.events
                    .and_then(|i| std::str::from_utf8(&row[i]).ok()?.parse::<u64>().ok())
            } else {
                None
            },
            template_duration: if template {
                self.template_duration.and_then(|i| {
                    std::str::from_utf8(&row[i])
                        .ok()?
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.0)
                })
            } else {
                None
            },
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
    #[cfg(test)]
    pub fn new(source: R) -> Result<Self> {
        Self::with_options(source, InputSchema::Auto, ReadRepresentation::Auto)
    }

    pub fn with_options(
        source: R,
        schema: InputSchema,
        representation: ReadRepresentation,
    ) -> Result<Self> {
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
        let columns = Columns::new(headers, schema, representation)?;
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

    pub fn schema(&self) -> InputSchema {
        self.columns.schema
    }
    pub fn representation(&self) -> ReadRepresentation {
        self.columns.representation
    }

    pub fn has_native_filtering(&self) -> bool {
        self.columns.passes_filtering.is_some()
            && self.columns.representation == ReadRepresentation::Template
    }

    pub fn has_template_duration(&self) -> bool {
        self.columns.template_duration.is_some()
            && self.columns.representation == ReadRepresentation::Template
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
        if let Some(observed) = record.run_id {
            if let Some(run_id) = &self.run_id {
                ensure!(
                    !self.enforce_single_run || run_id == observed,
                    "line {line}: inconsistent run_id {observed:?}; expected {run_id:?}; use --split-runs to analyze multiple runs"
                );
            } else {
                self.run_id = Some(observed.to_owned());
            }
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
            assert_eq!(record.end_reason, Some("unknown_new_reason"));
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
