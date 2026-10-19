//! Source guards for streaming/repeated reads; not a filesystem snapshot or lock.
use anyhow::{Context, Result, ensure};
use flate2::bufread::MultiGzDecoder;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata},
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

const CHECK_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    changed: (i64, i64),
}
impl Stamp {
    fn new(metadata: &Metadata) -> Result<Self> {
        ensure!(metadata.is_file(), "source is no longer a regular file");
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified()?,
            #[cfg(unix)]
            identity: (metadata.dev(), metadata.ino()),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Membership {
    runs: BTreeMap<String, u64>,
    unidentified: u64,
}
impl Membership {
    pub fn observe(&mut self, run: Option<&str>) {
        if let Some(id) = run {
            if let Some(count) = self.runs.get_mut(id) {
                *count += 1;
            } else {
                self.runs.insert(id.to_owned(), 1);
            }
        } else {
            self.unidentified += 1;
        }
    }
}

pub struct Source {
    path: PathBuf,
    stamp: Stamp,
    verify_content: bool,
    digest: Mutex<Option<[u8; 32]>>,
    membership: Mutex<Option<Membership>>,
}
impl Source {
    pub fn capture_all(paths: &[PathBuf], verify_content: bool) -> Result<Vec<Self>> {
        #[cfg(unix)]
        let mut identities = std::collections::BTreeSet::new();
        let mut sources = Vec::new();
        for path in paths {
            let source = Self::capture(path, verify_content).with_context(|| {
                format!(
                    "cannot freeze input {}; stop writers and retry with stable inputs",
                    path.display()
                )
            })?;
            #[cfg(unix)]
            ensure!(
                identities.insert(source.stamp.identity),
                "source identity duplicated during discovery: {}; retry with stable inputs",
                path.display()
            );
            sources.push(source);
        }
        Ok(sources)
    }
    pub fn reject_outputs(sources: &[Self], destinations: &[&Path]) -> Result<()> {
        for destination in destinations {
            let canonical =
                fs::canonicalize(destination.parent().context("output has no parent")?)?
                    .join(destination.file_name().context("output has no filename")?);
            ensure!(
                !sources.iter().any(|s| s.path == canonical),
                "output would overwrite input {}; choose a different --output directory",
                canonical.display()
            );
            #[cfg(unix)]
            match fs::metadata(destination) {
                Ok(metadata) => {
                    use std::os::unix::fs::MetadataExt;
                    ensure!(
                        !sources
                            .iter()
                            .any(|s| s.stamp.identity == (metadata.dev(), metadata.ino())),
                        "output aliases input file {}; choose a different --output directory",
                        destination.display()
                    );
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "cannot verify output {} against inputs",
                            destination.display()
                        )
                    });
                }
            }
        }
        Ok(())
    }
    pub fn capture(path: &Path, verify_content: bool) -> Result<Self> {
        let canonical = fs::canonicalize(path)?;
        ensure!(
            canonical == path,
            "source path changed during discovery: {}; retry with stable inputs",
            path.display()
        );
        let stamp = Stamp::new(&fs::symlink_metadata(path)?)?;
        Ok(Self {
            path: canonical,
            stamp,
            verify_content,
            digest: Mutex::new(None),
            membership: Mutex::new(None),
        })
    }
    fn context(&self, stage: &str) -> String {
        format!(
            "input changed or cannot be verified: {} during {stage}; stop writers and retry from a stable copy/snapshot (on NFS consider --verify-input-content)",
            self.path.display()
        )
    }
    pub fn check(&self, stage: &str) -> Result<()> {
        let checked = || -> Result<()> {
            ensure!(
                fs::canonicalize(&self.path)? == self.path,
                "canonical path changed"
            );
            ensure!(
                Stamp::new(&fs::symlink_metadata(&self.path)?)? == self.stamp,
                "file identity, size or timestamps changed"
            );
            Ok(())
        };
        checked().with_context(|| self.context(stage))
    }
    fn check_handle(&self, file: &File, stage: &str) -> Result<()> {
        ensure!(
            Stamp::new(&file.metadata()?).with_context(|| self.context(stage))? == self.stamp,
            "{}: open-file identity, size or timestamps changed",
            self.context(stage)
        );
        self.check(stage)
    }
    pub fn open(&self, stage: &'static str) -> Result<Pass<'_>> {
        self.check(stage)?;
        let file = File::open(&self.path).with_context(|| self.context(stage))?;
        self.check_handle(&file, stage)?;
        Ok(Pass {
            source: self,
            file,
            stage,
            bytes: 0,
            checked_at: 0,
            eof: false,
            hash: self.verify_content.then(Sha256::new),
        })
    }
    /// Decode above the raw-byte guard: identities, byte counts and digests
    /// always refer to the original file, including its gzip headers/trailers.
    pub fn open_summary(&self, stage: &'static str) -> Result<SummaryPass<'_>> {
        let mut input = BufReader::with_capacity(64 * 1024, self.open(stage)?);
        let magic = input.fill_buf().with_context(|| {
            format!(
                "cannot inspect input {} during {stage}",
                self.path.display()
            )
        })?;
        let gzip = self.path.extension().is_some_and(|ext| ext == "gz")
            || magic.starts_with(&[0x1f, 0x8b]);
        let reader = if gzip {
            Stream::Gzip(Box::new(MultiGzDecoder::new(input)))
        } else {
            Stream::Plain(input)
        };
        Ok(SummaryPass {
            reader,
            source: self,
            stage,
            complete: false,
            failed: false,
        })
    }
    pub fn validate_membership(&self, membership: Membership, stage: &str) -> Result<()> {
        let mut saved = self
            .membership
            .lock()
            .map_err(|_| anyhow::anyhow!("source membership guard poisoned"))?;
        if let Some(expected) = saved.as_ref() {
            ensure!(
                expected == &membership,
                "{}: run-ID membership/counts changed",
                self.context(stage)
            );
        } else {
            *saved = Some(membership);
        }
        Ok(())
    }
    /// Called after rendering, immediately before the first staged rename.
    /// Strong mode rereads contributing sources once here; ordinary mode stats only.
    pub fn verify_for_publication(&self) -> Result<()> {
        if self.verify_content {
            let mut pass = self.open_summary("publication content verification")?;
            io::copy(&mut pass, &mut io::sink())?;
            pass.finish()?;
        } else {
            self.check("publication")?;
        }
        Ok(())
    }
}

// One decoder per pass; never a decompressed file or retained read dataset.
enum Stream<'a> {
    Plain(BufReader<Pass<'a>>),
    Gzip(Box<MultiGzDecoder<BufReader<Pass<'a>>>>),
}

pub struct SummaryPass<'a> {
    reader: Stream<'a>,
    source: &'a Source,
    stage: &'static str,
    complete: bool,
    failed: bool,
}
impl Read for SummaryPass<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (kind, result) = match &mut self.reader {
            Stream::Plain(reader) => ("plain TSV", reader.read(output)),
            Stream::Gzip(reader) => ("gzip", reader.read(output)),
        };
        match result {
            Ok(n) => {
                self.complete |= n == 0;
                Ok(n)
            }
            Err(error) => {
                self.failed = true;
                Err(io::Error::new(
                    error.kind(),
                    format!(
                        "cannot read {kind} input {} during {}: {error}",
                        self.source.path.display(),
                        self.stage
                    ),
                ))
            }
        }
    }
}
impl SummaryPass<'_> {
    pub fn finish(self) -> Result<()> {
        ensure!(
            self.complete && !self.failed,
            "{}: summary stream was not completely validated",
            self.source.context(self.stage)
        );
        let input = match self.reader {
            Stream::Plain(input) => input,
            Stream::Gzip(input) => input.into_inner(),
        };
        // Decoder EOF alone is insufficient: no unread compressed suffix may
        // remain, and the raw guard must have observed EOF on the actual file.
        ensure!(
            input.buffer().is_empty(),
            "{}: unread source bytes remain after decoding",
            self.source.context(self.stage)
        );
        input.into_inner().finish()
    }
}

pub struct Pass<'a> {
    source: &'a Source,
    file: File,
    stage: &'static str,
    bytes: u64,
    checked_at: u64,
    eof: bool,
    hash: Option<Sha256>,
}
impl Read for Pass<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let n = self.file.read(output)?;
        self.eof |= n == 0;
        self.bytes += n as u64;
        if let Some(hash) = &mut self.hash {
            hash.update(&output[..n]);
        }
        if self.bytes - self.checked_at >= CHECK_BYTES {
            self.source
                .check_handle(&self.file, self.stage)
                .map_err(io::Error::other)?;
            self.checked_at = self.bytes;
        }
        Ok(n)
    }
}
impl Pass<'_> {
    pub fn finish(self) -> Result<()> {
        ensure!(
            self.eof && self.bytes == self.source.stamp.size,
            "{}: incomplete read or byte-count mismatch",
            self.source.context(self.stage)
        );
        self.source.check_handle(&self.file, self.stage)?;
        if let Some(hash) = self.hash {
            let digest: [u8; 32] = hash.finalize().into();
            let mut saved = self
                .source
                .digest
                .lock()
                .map_err(|_| anyhow::anyhow!("source digest guard poisoned"))?;
            if let Some(expected) = *saved {
                ensure!(
                    digest == expected,
                    "{}: SHA-256 differs from the first completed pass",
                    self.source.context(self.stage)
                );
            } else {
                *saved = Some(digest);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::OpenOptions,
        io::{Seek, SeekFrom, Write},
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(bytes: &[u8]) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let directory = std::env::temp_dir().join(format!(
                "ontrunqc-integrity-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            let path = directory.join("summary.tsv");
            fs::write(&path, bytes).unwrap();
            Self(path)
        }
        fn source(&self, strong: bool) -> Source {
            Source::capture(&fs::canonicalize(&self.0).unwrap(), strong).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().unwrap());
        }
    }
    fn consume(source: &Source, stage: &'static str) -> Result<()> {
        let mut pass = source.open(stage)?;
        io::copy(&mut pass, &mut io::sink())?;
        pass.finish()
    }

    #[test]
    fn unchanged_passes_and_publication_keep_one_digest() {
        let fixture = Fixture::new(b"abc");
        let source = fixture.source(true);
        for stage in [
            "independent-component discovery",
            "aggregation pass",
            "fitted-trend pass",
            "MinION visualization pass",
        ] {
            consume(&source, stage).unwrap();
        }
        source.verify_for_publication().unwrap();
        let expected: [u8; 32] = Sha256::digest(b"abc").into();
        assert_eq!(*source.digest.lock().unwrap(), Some(expected));
    }
    #[test]
    fn replacements_deletions_and_truncations_between_passes_fail_actionably() {
        for stage in [
            "independent-component discovery",
            "aggregation pass",
            "fitted-trend pass",
            "MinION visualization pass",
        ] {
            for mutation in [0, 1, 2] {
                let fixture = Fixture::new(b"original");
                let source = fixture.source(false);
                consume(&source, "aggregation pass").unwrap();
                match mutation {
                    0 => {
                        let replacement = fixture.0.with_extension("new");
                        fs::write(&replacement, b"replaced").unwrap();
                        fs::rename(replacement, &fixture.0).unwrap();
                    }
                    1 => fs::write(&fixture.0, b"x").unwrap(),
                    _ => fs::remove_file(&fixture.0).unwrap(),
                }
                let error = consume(&source, stage).unwrap_err().to_string();
                assert!(
                    error.contains(stage)
                        && error.contains("summary.tsv")
                        && error.contains("stable"),
                    "{error}"
                );
            }
        }
    }
    #[test]
    fn open_handle_does_not_hide_atomic_replacement_or_appending() {
        for replace in [false, true] {
            let fixture = Fixture::new(b"old");
            let source = fixture.source(false);
            let mut pass = source.open("aggregation pass").unwrap();
            let mut bytes = Vec::new();
            pass.read_to_end(&mut bytes).unwrap();
            if replace {
                let next = fixture.0.with_extension("next");
                fs::write(&next, b"new").unwrap();
                fs::rename(next, &fixture.0).unwrap();
            } else {
                OpenOptions::new()
                    .append(true)
                    .open(&fixture.0)
                    .unwrap()
                    .write_all(b"added")
                    .unwrap();
            }
            assert!(pass.finish().is_err());
        }
    }
    #[test]
    fn mutation_during_streaming_is_detected_before_end_of_large_parse() {
        let fixture = Fixture::new(&vec![b'a'; CHECK_BYTES as usize + 65536]);
        let source = fixture.source(false);
        let mut pass = source.open("aggregation pass").unwrap();
        let mut buffer = [0; 65536];
        pass.read_exact(&mut buffer).unwrap();
        let mut writer = OpenOptions::new().write(true).open(&fixture.0).unwrap();
        writer.seek(SeekFrom::End(-1)).unwrap();
        writer.write_all(b"b").unwrap();
        // Force a distinct mtime without sleeps or filesystem clock assumptions.
        writer.set_modified(SystemTime::UNIX_EPOCH).unwrap();
        let error = io::copy(&mut pass, &mut io::sink())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("aggregation pass") && error.contains("timestamps changed"),
            "{error}"
        );
    }
    #[test]
    fn sha256_detects_equal_length_changes_when_metadata_appears_unchanged() {
        let fixture = Fixture::new(b"first");
        let mut source = fixture.source(true);
        consume(&source, "aggregation pass").unwrap();
        fs::write(&fixture.0, b"other").unwrap();
        // Simulate an NFS metadata cache returning an unchanged accepted stamp.
        // Production never updates its frozen stamp.
        source.stamp = Stamp::new(&fs::metadata(&fixture.0).unwrap()).unwrap();
        source.check("test cached stamp").unwrap();
        let error = consume(&source, "fitted-trend pass")
            .unwrap_err()
            .to_string();
        assert!(error.contains("SHA-256"), "{error}");
        assert!(source.verify_for_publication().is_err());
    }
    #[test]
    fn membership_changes_reject_existing_ids_missing_ids_and_count_drift() {
        let fixture = Fixture::new(b"data");
        let source = fixture.source(false);
        let mut first = Membership::default();
        for id in [Some("a"), Some("b"), None] {
            first.observe(id);
        }
        source.validate_membership(first, "discovery").unwrap();
        for values in [
            vec![Some("a"), Some("a"), None],
            vec![Some("a"), Some("b")],
            vec![Some("a"), Some("b"), Some("c")],
        ] {
            let mut next = Membership::default();
            for id in values {
                next.observe(id);
            }
            assert!(
                source
                    .validate_membership(next, "MinION visualization pass")
                    .unwrap_err()
                    .to_string()
                    .contains("membership")
            );
        }
    }
    #[test]
    fn incomplete_pass_is_not_accepted() {
        let fixture = Fixture::new(b"not consumed");
        let source = fixture.source(true);
        assert!(source.open("aggregation pass").unwrap().finish().is_err());
        assert!(source.digest.lock().unwrap().is_none());
    }
    #[test]
    #[cfg(unix)]
    fn replacing_a_canonical_source_with_a_symlink_is_rejected() {
        let fixture = Fixture::new(b"same");
        let source = fixture.source(false);
        let moved = fixture.0.with_extension("moved");
        fs::rename(&fixture.0, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &fixture.0).unwrap();
        assert!(source.check("publication").is_err());
    }
    #[test]
    #[cfg(unix)]
    fn freezing_rejects_sources_that_became_inode_aliases_after_discovery() {
        let fixture = Fixture::new(b"data");
        let alias = fixture.0.with_extension("alias");
        fs::hard_link(&fixture.0, &alias).unwrap();
        let paths = [
            fs::canonicalize(&fixture.0).unwrap(),
            fs::canonicalize(alias).unwrap(),
        ];
        let result = Source::capture_all(&paths, false);
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("identity duplicated")
        );
    }
    #[test]
    #[cfg(unix)]
    fn hardlinked_output_aliases_are_rejected_before_publication() {
        let fixture = Fixture::new(b"data");
        let output = fixture.0.with_file_name("report.json");
        fs::hard_link(&fixture.0, &output).unwrap();
        let sources = [fixture.source(false)];
        assert!(
            Source::reject_outputs(&sources, &[&output])
                .unwrap_err()
                .to_string()
                .contains("aliases input")
        );
    }

    fn gzip_bytes(bytes: &[u8], mtime: u32) -> Vec<u8> {
        let mut encoder = flate2::GzBuilder::new()
            .mtime(mtime)
            .write(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn decoded(source: &Source, stage: &'static str) -> Result<Vec<u8>> {
        let mut pass = source.open_summary(stage)?;
        let mut bytes = Vec::new();
        pass.read_to_end(&mut bytes)?;
        pass.finish()?;
        Ok(bytes)
    }

    #[test]
    fn gzip_all_passes_hash_compressed_bytes_including_headers() {
        let payload =
            b"channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t10\t8\n";
        let compressed = gzip_bytes(payload, 1);
        // No extension: magic-byte detection must also use the original guard.
        let fixture = Fixture::new(&compressed);
        let mut source = fixture.source(true);
        for stage in [
            "independent-component discovery",
            "aggregation pass",
            "fitted-trend pass",
            "MinION visualization pass",
        ] {
            assert_eq!(decoded(&source, stage).unwrap(), payload);
        }
        source.verify_for_publication().unwrap();
        assert_eq!(
            *source.digest.lock().unwrap(),
            Some(Sha256::digest(&compressed).into())
        );
        let changed = gzip_bytes(payload, 2);
        assert_eq!(changed.len(), compressed.len());
        fs::write(&fixture.0, changed).unwrap();
        source.stamp = Stamp::new(&fs::metadata(&fixture.0).unwrap()).unwrap();
        let error = decoded(&source, "fitted-trend pass")
            .unwrap_err()
            .to_string();
        assert!(error.contains("SHA-256"), "{error}");
        assert!(source.verify_for_publication().is_err());
    }

    #[test]
    fn gzip_mutations_between_and_during_passes_reject_publication() {
        for replace in [false, true] {
            let fixture = Fixture::new(&gzip_bytes(b"same payload", 0));
            let source = fixture.source(true);
            decoded(&source, "aggregation pass").unwrap();
            let mut pass = source.open_summary("MinION visualization pass").unwrap();
            let mut bytes = Vec::new();
            pass.read_to_end(&mut bytes).unwrap();
            if replace {
                let next = fixture.0.with_extension("next");
                fs::write(&next, gzip_bytes(b"same payload", 0)).unwrap();
                fs::rename(next, &fixture.0).unwrap();
            } else {
                fs::write(&fixture.0, b"truncated").unwrap();
            }
            assert!(pass.finish().is_err());
            let error = decoded(&source, "fitted-trend pass")
                .unwrap_err()
                .to_string();
            assert!(error.contains("fitted-trend pass"), "{error}");
            assert!(source.verify_for_publication().is_err());
        }
    }

    #[test]
    fn gzip_requires_decoded_eof_even_if_raw_bytes_are_buffered() {
        let fixture = Fixture::new(&gzip_bytes(&vec![b'x'; 1_000_000], 0));
        let source = fixture.source(true);
        let mut pass = source.open_summary("aggregation pass").unwrap();
        pass.read_exact(&mut [0; 4]).unwrap();
        assert!(pass.finish().is_err());
        assert!(source.digest.lock().unwrap().is_none());
    }

    #[test]
    fn gzip_finish_never_accepts_a_swallowed_crc_error() {
        let mut bytes = gzip_bytes(b"payload", 0);
        let n = bytes.len();
        bytes[n - 8] ^= 1;
        let fixture = Fixture::new(&bytes);
        let source = fixture.source(true);
        let mut pass = source.open_summary("aggregation pass").unwrap();
        assert!(io::copy(&mut pass, &mut io::sink()).is_err());
        // The decoder may report EOF after its error; it is still a failed pass.
        let _ = io::copy(&mut pass, &mut io::sink());
        assert!(pass.finish().is_err());
        assert!(source.digest.lock().unwrap().is_none());
        assert!(source.verify_for_publication().is_err());
    }
}
