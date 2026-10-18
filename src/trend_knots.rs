//! Exact unique-time quantiles for optional GAM fits. Numeric sorted runs bound
//! RAM independently of read count; no read objects or identifiers are retained.
use anyhow::{Context, Result};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const BUFFER: usize = 262_144; // 4 MiB of (time bits, cohort flags) elements.
const FAN_IN: usize = 64;
const K: usize = 10;
type Entry = (u64, u8);
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct Times {
    buffer: Vec<Entry>,
    directory: Option<PathBuf>,
    runs: Vec<Vec<PathBuf>>,
    next_file: u64,
}
impl Drop for Times {
    fn drop(&mut self) {
        if let Some(path) = &self.directory {
            let _ = fs::remove_dir_all(path);
        }
    }
}
impl Times {
    pub fn add(&mut self, hours: f64, flags: u8) -> Result<()> {
        self.buffer
            .push((if hours == 0.0 { 0 } else { hours.to_bits() }, flags));
        if self.buffer.len() == BUFFER {
            self.spill().context("staging exact GAM knot times")?;
        }
        Ok(())
    }
    fn path(&mut self) -> Result<PathBuf> {
        if self.directory.is_none() {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "poregauge-knots-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                match builder.create(&path) {
                    Ok(()) => {
                        self.directory = Some(path);
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let path = self
            .directory
            .as_ref()
            .unwrap()
            .join(self.next_file.to_string());
        self.next_file += 1;
        Ok(path)
    }
    fn spill(&mut self) -> Result<()> {
        compact(&mut self.buffer);
        let path = self.path()?;
        let mut writer = create(&path)?;
        for &entry in &self.buffer {
            write_entry(&mut writer, entry)?;
        }
        writer.flush()?;
        self.buffer.clear();
        // At most FAN_IN-1 pending files per level; filename metadata also
        // stays bounded (logarithmic), even for arbitrarily many chunks.
        let (mut path, mut level) = (path, 0);
        loop {
            if self.runs.len() <= level {
                self.runs.push(Vec::new());
            }
            self.runs[level].push(path);
            if self.runs[level].len() < FAN_IN {
                break;
            }
            let paths = std::mem::take(&mut self.runs[level]);
            path = self.path()?;
            merge(&paths, &path)?;
            for old in paths {
                fs::remove_file(old)?;
            }
            level += 1;
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<[Option<[f64; K]>; 3]> {
        if self.runs.is_empty() {
            compact(&mut self.buffer);
            let counts = counts(self.buffer.iter().copied());
            return Ok(quantiles(self.buffer.iter().copied(), counts));
        }
        if !self.buffer.is_empty() {
            self.spill()?;
        }
        let mut pending: Vec<_> = std::mem::take(&mut self.runs)
            .into_iter()
            .flatten()
            .collect();
        while pending.len() > 1 {
            let previous = std::mem::take(&mut pending);
            for paths in previous.chunks(FAN_IN) {
                let path = self.path()?;
                merge(paths, &path).context("merging exact GAM knot times")?;
                pending.push(path);
                for old in paths {
                    fs::remove_file(old)?;
                }
            }
        }
        let path = &pending[0];
        let mut reader = BufReader::new(File::open(path)?);
        let mut totals = [0; 3];
        while let Some((_, mask)) = read_entry(&mut reader)? {
            for (i, count) in totals.iter_mut().enumerate() {
                if mask & (1 << i) != 0 {
                    *count += 1;
                }
            }
        }
        let mut reader = BufReader::new(File::open(path)?);
        let mut selection = Selection::new(totals);
        while let Some(entry) = read_entry(&mut reader)? {
            selection.add(entry);
        }
        Ok(selection.finish())
    }
}
fn compact(entries: &mut Vec<Entry>) {
    entries.sort_unstable_by_key(|e| e.0);
    entries.dedup_by(|b, a| {
        if a.0 == b.0 {
            a.1 |= b.1;
            true
        } else {
            false
        }
    });
}
fn create(path: &Path) -> Result<BufWriter<File>> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(BufWriter::new(options.open(path)?))
}
fn write_entry(writer: &mut impl Write, (bits, mask): Entry) -> Result<()> {
    writer.write_all(&bits.to_le_bytes())?;
    writer.write_all(&[mask])?;
    Ok(())
}
fn read_entry(reader: &mut impl Read) -> Result<Option<Entry>> {
    let mut record = [0; 9];
    if reader.read(&mut record[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut record[1..])?;
    Ok(Some((
        u64::from_le_bytes(record[..8].try_into().unwrap()),
        record[8],
    )))
}
fn merge(paths: &[PathBuf], output: &Path) -> Result<()> {
    let mut readers = paths
        .iter()
        .map(|p| File::open(p).map(BufReader::new))
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut heap = BinaryHeap::new();
    for (i, reader) in readers.iter_mut().enumerate() {
        if let Some((time, flags)) = read_entry(reader)? {
            heap.push(Reverse((time, flags, i)));
        }
    }
    let mut writer = create(output)?;
    let mut last: Option<Entry> = None;
    while let Some(Reverse((time, flags, i))) = heap.pop() {
        if let Some(previous) = &mut last {
            if previous.0 == time {
                previous.1 |= flags;
            } else {
                write_entry(&mut writer, *previous)?;
                *previous = (time, flags);
            }
        } else {
            last = Some((time, flags));
        }
        if let Some((time, flags)) = read_entry(&mut readers[i])? {
            heap.push(Reverse((time, flags, i)));
        }
    }
    if let Some(entry) = last {
        write_entry(&mut writer, entry)?;
    }
    writer.flush()?;
    Ok(())
}
fn counts(entries: impl Iterator<Item = Entry>) -> [u64; 3] {
    let mut counts = [0; 3];
    for (_, mask) in entries {
        for (i, count) in counts.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *count += 1;
            }
        }
    }
    counts
}
struct Selection {
    counts: [u64; 3],
    indexes: [u64; 3],
    lower: [[f64; K]; 3],
    upper: [[f64; K]; 3],
}
impl Selection {
    fn new(counts: [u64; 3]) -> Self {
        Self {
            counts,
            indexes: [0; 3],
            lower: [[0.0; K]; 3],
            upper: [[0.0; K]; 3],
        }
    }
    fn add(&mut self, (bits, mask): Entry) {
        let time = f64::from_bits(bits);
        for i in 0..3 {
            if mask & (1 << i) == 0 || self.counts[i] < K as u64 {
                continue;
            }
            for k in 0..K {
                let rank = reference_rank(self.counts[i], k);
                if self.indexes[i] == rank.floor() as u64 {
                    self.lower[i][k] = time;
                }
                if self.indexes[i] == rank.ceil() as u64 {
                    self.upper[i][k] = time;
                }
            }
            self.indexes[i] += 1;
        }
    }
    fn finish(self) -> [Option<[f64; K]>; 3] {
        std::array::from_fn(|i| {
            (self.counts[i] >= K as u64).then(|| {
                std::array::from_fn(|k| {
                    let fraction = reference_rank(self.counts[i], k).fract();
                    if self.lower[i][k] == self.upper[i][k] {
                        self.lower[i][k]
                    } else {
                        self.lower[i][k] * (1.0 - fraction) + self.upper[i][k] * fraction
                    }
                })
            })
        })
    }
}
// Same type-7 quantiles, including the arithmetic order of R's seq()/quantile().
// One-ulp knot differences can rotate cs's numerical null space.
fn reference_rank(count: u64, k: usize) -> f64 {
    let probability = k as f64 * (1.0 / (K - 1) as f64);
    (1.0 + (count - 1) as f64 * probability) - 1.0
}
fn quantiles(entries: impl Iterator<Item = Entry>, counts: [u64; 3]) -> [Option<[f64; K]>; 3] {
    let mut selection = Selection::new(counts);
    for entry in entries {
        selection.add(entry);
    }
    selection.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn type7_arithmetic_matches_pinned_r_sequence_and_one_based_indices() {
        // Actual R 4.4.1 quantile((0:5999)/3600, seq(0,1,length=10)).
        let reference = [
            0.0,
            0.18515432098765433,
            0.370_308_641_975_308_6,
            0.555_462_962_962_962_9,
            0.740_617_283_950_617_3,
            0.9257716049382716,
            1.1109259259259259,
            1.2960802469135801,
            1.4812345679012346,
            1.666_388_888_888_889,
        ];
        let mut times = Times::default();
        for i in 0..6000 {
            times.add(i as f64 / 3600.0, 7).unwrap();
        }
        assert_eq!(times.finish().unwrap()[0].unwrap(), reference);
    }
    #[test]
    fn exact_unique_cohort_quantiles_ignore_duplicates_and_input_order() {
        let mut times = Times::default();
        for n in (0..100).rev() {
            times.add(n as f64, if n % 2 == 0 { 7 } else { 1 }).unwrap();
            times.add(n as f64, 1).unwrap();
        }
        let result = times.finish().unwrap();
        for i in 0..K {
            assert!((result[0].unwrap()[i] - 99.0 * i as f64 / 9.0).abs() < 1e-12);
        }
        assert_eq!(result[1], result[2]);
        assert_eq!(result[1].unwrap()[9], 98.0);
        assert!(
            Times::default()
                .finish()
                .unwrap()
                .iter()
                .all(Option::is_none)
        );
    }
    #[test]
    fn spilled_runs_merge_cohort_flags_and_clean_owned_temporary_files() {
        let mut times = Times::default();
        // Force many tiny runs to exercise both merge levels cheaply.
        for n in 0..FAN_IN + 2 {
            for value in (0..31).rev() {
                times
                    .add(value as f64, if n % 2 == 0 { 1 } else { 7 })
                    .unwrap();
            }
            times.spill().unwrap();
        }
        let directory = times.directory.clone().unwrap();
        let result = times.finish().unwrap();
        assert_eq!(result[0], result[1]);
        assert_eq!(result[1], result[2]);
        assert_eq!(result[0].unwrap()[9], 30.0);
        assert!(!directory.exists());
        let mut times = Times::default();
        times.add(1.0, 1).unwrap();
        times.spill().unwrap();
        let directory = times.directory.clone().unwrap();
        drop(times);
        assert!(!directory.exists());
    }
    #[test]
    fn automatic_spill_is_bounded_and_failure_cleans_scratch() {
        let mut times = Times::default();
        for i in (0..BUFFER + 19).rev() {
            times.add(i as f64, 7).unwrap();
        }
        assert_eq!(times.buffer.len(), 19);
        assert_eq!(times.buffer.capacity(), BUFFER);
        let directory = times.directory.clone().unwrap();
        let result = times.finish().unwrap();
        assert_eq!(result[0].unwrap()[0], 0.0);
        assert_eq!(result[0].unwrap()[9], (BUFFER + 18) as f64);
        assert!(!directory.exists());
        let mut times = Times::default();
        times.add(1.0, 7).unwrap();
        times.spill().unwrap();
        let directory = times.directory.clone().unwrap();
        fs::write(&times.runs[0][0], [1, 2]).unwrap();
        assert!(times.finish().is_err());
        assert!(!directory.exists());
    }
}
