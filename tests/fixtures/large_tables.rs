// Deterministic fictional data; also runnable with rustc for report measurements.
use std::io::{self, Write};

pub fn write(mut out: impl Write) -> io::Result<()> {
    writeln!(
        out,
        "run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\ttemplate_duration"
    )?;
    for channel in (1..=3000).rev() {
        let muxes: &[u32] = if channel <= 1000 {
            &[1, 2, 7, 12]
        } else {
            &[1, 2, 7]
        };
        for &mux in muxes {
            let index = (channel - 1) * 4 + mux;
            let length = if channel == 3000 {
                0
            } else {
                1 + (index * 7919) % 250000
            };
            let reason = if index % 7 == 0 {
                "synthetic_other"
            } else if length == 0 {
                "synthetic_zero"
            } else {
                "synthetic_normal"
            };
            for repetition in 0..2 {
                let time = ((index * 17 + repetition * 13) % 96) * 900;
                let passed = if (index + repetition) % 5 == 0 {
                    "FALSE"
                } else {
                    "TRUE"
                };
                writeln!(
                    out,
                    "synthetic-large-tables\t{channel}\t{mux}\t{time}.5\t2\t{passed}\t{length}\t{}.5\t{reason}\t1.5",
                    10 + index % 25
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(not(test))]
fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    write(&mut output)?;
    output.flush()
}
