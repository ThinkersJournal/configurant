// SPDX-License-Identifier: MIT OR Apache-2.0
//! Executing a `DataTransfer` (feature `persistant`, `docs/TRANSFER.md`). Known-bad first: a target that
//! already holds records, a source that cannot say what expires, a copy that fails halfway, a copy that is
//! wrong, a secret endpoint in an error.

#![cfg(feature = "persistant")]

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use configurant::Needs;
use configurant::persistant::{
    Durability, Endpoint, PersistantModule, TransferEnd, TransferError, TransferOptions,
    TransferPhase, TransferProgress, copy_between, copy_between_with_progress,
};
use persistant::capability::provider::ProviderNeeds;
use persistant::kv::{Defect, KeyPage, KvBackend, KvError, MemoryKv, ValuePage};
use persistant::registry;

fn run<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(f)
}

fn fill(kv: &MemoryKv, n: usize) {
    run(async {
        for i in 0..n {
            kv.put(&format!("key{i:04}"), vec![i as u8, 0, 255], None)
                .await
                .unwrap();
        }
    });
}

fn all(kv: &MemoryKv) -> Vec<(String, Vec<u8>)> {
    run(async {
        kv.ordered_scan_values("", None, 10_000)
            .await
            .unwrap()
            .entries
    })
}

/// A backend that wraps a `MemoryKv` and is wrong in a chosen way.
struct Wrap<'a> {
    inner: &'a MemoryKv,
    /// `put` fails once this many have succeeded.
    fail_put_after: Option<usize>,
    puts: AtomicUsize,
    /// `ttl_remaining` is `Unsupported`.
    no_ttl: bool,
    /// `get` of this key returns the value with its first byte flipped.
    flip_get: Option<&'static str>,
}

impl<'a> Wrap<'a> {
    fn new(inner: &'a MemoryKv) -> Self {
        Wrap {
            inner,
            fail_put_after: None,
            puts: AtomicUsize::new(0),
            no_ttl: false,
            flip_get: None,
        }
    }
}

impl KvBackend for Wrap<'_> {
    fn backend(&self) -> &str {
        self.inner.backend()
    }
    fn version(&self) -> &str {
        self.inner.version()
    }
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError> {
        let mut v = self.inner.get(key).await?;
        if self.flip_get == Some(key)
            && let Some(bytes) = v.as_mut()
        {
            bytes[0] ^= 0xFF;
        }
        Ok(v)
    }
    async fn put(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), KvError> {
        if let Some(n) = self.fail_put_after
            && self.puts.fetch_add(1, Ordering::SeqCst) >= n
        {
            return Err(KvError::Backend("disk full".to_owned()));
        }
        self.inner.put(key, value, ttl).await
    }
    async fn delete(&self, key: &str) -> Result<(), KvError> {
        self.inner.delete(key).await
    }
    async fn put_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<bool, KvError> {
        self.inner.put_if_absent(key, value, ttl).await
    }
    async fn ordered_scan(
        &self,
        prefix: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<KeyPage, KvError> {
        self.inner.ordered_scan(prefix, after, limit).await
    }
    async fn ordered_scan_values(
        &self,
        prefix: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ValuePage, KvError> {
        self.inner.ordered_scan_values(prefix, after, limit).await
    }
    async fn ttl_remaining(&self, key: &str) -> Result<Option<Duration>, KvError> {
        if self.no_ttl {
            return Err(KvError::Unsupported("ttl_remaining".to_owned()));
        }
        self.inner.ttl_remaining(key).await
    }
}

#[test]
fn a_target_that_already_holds_records_is_refused_and_left_as_it_was() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 3);
    run(dst.put("existing", b"mine".to_vec(), None)).unwrap();
    let err = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap_err();
    assert_eq!(err, TransferError::TargetNotEmpty);
    assert_eq!(all(&dst), vec![("existing".to_owned(), b"mine".to_vec())]);
}

#[test]
fn allow_existing_keeps_the_value_already_there() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 3);
    run(dst.put("key0001", b"mine".to_vec(), None)).unwrap();
    let opts = TransferOptions::default().allow_existing(true);
    let report = run(copy_between(&src, &dst, &opts)).unwrap();
    assert_eq!((report.copied, report.skipped), (2, 1), "{report:?}");
    assert_eq!(run(dst.get("key0001")).unwrap(), Some(b"mine".to_vec()));
    assert_eq!(run(dst.get("key0002")).unwrap(), Some(vec![2, 0, 255]));
}

#[test]
fn every_record_arrives_across_pages_and_the_source_is_unchanged() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 25);
    run(src.put("empty-value", Vec::new(), None)).unwrap();
    let before = all(&src);
    let report = run(copy_between(
        &src,
        &dst,
        &TransferOptions::default().page_size(4),
    ))
    .unwrap();
    assert_eq!(report.copied, before.len(), "{report:?}");
    assert_eq!(report.verified, before.len());
    assert_eq!(all(&dst), before);
    assert_eq!(all(&src), before, "the source must not change");
}

#[test]
fn a_time_to_live_goes_with_the_record() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    run(src.put("soon", b"x".to_vec(), Some(Duration::from_secs(3600)))).unwrap();
    run(src.put("forever", b"y".to_vec(), None)).unwrap();
    let report = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap();
    assert_eq!(report.with_ttl, 1, "{report:?}");
    let left = run(dst.ttl_remaining("soon"))
        .unwrap()
        .expect("kept its expiry");
    assert!(left > Duration::from_secs(3000) && left <= Duration::from_secs(3600));
    assert_eq!(run(dst.ttl_remaining("forever")).unwrap(), None);
}

#[test]
fn a_source_that_cannot_report_expiry_is_refused_before_anything_is_written() {
    let (inner, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&inner, 3);
    let src = Wrap {
        no_ttl: true,
        ..Wrap::new(&inner)
    };
    let err = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap_err();
    assert!(
        matches!(&err, TransferError::Unsupported { side: "source", op } if op == "ttl_remaining"),
        "{err:?}"
    );
    assert!(all(&dst).is_empty(), "nothing may be written");
}

#[test]
fn a_failure_halfway_is_an_error_and_the_source_is_unchanged() {
    let (src, inner) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 10);
    let before = all(&src);
    let dst = Wrap {
        fail_put_after: Some(4),
        ..Wrap::new(&inner)
    };
    let err = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap_err();
    assert!(
        matches!(
            &err,
            TransferError::Backend {
                side: "target",
                op: "put",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(all(&src), before, "the source must not change");
}

#[test]
fn a_copy_that_does_not_read_back_equal_is_an_error_naming_the_key() {
    let (src, inner) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 5);
    let dst = Wrap {
        flip_get: Some("key0002"),
        ..Wrap::new(&inner)
    };
    let err = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap_err();
    match err {
        TransferError::Mismatch {
            differing, sample, ..
        } => {
            assert_eq!(differing, 1);
            assert_eq!(sample, vec!["key0002".to_owned()]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_target_that_acknowledges_and_loses_a_write_is_caught_by_the_read_back() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::with_defect(Defect::PutIsLost));
    fill(&src, 3);
    let err = run(copy_between(&src, &dst, &TransferOptions::default())).unwrap_err();
    assert!(
        matches!(err, TransferError::Mismatch { missing: 3, .. }),
        "{err:?}"
    );
}

fn module() -> PersistantModule {
    PersistantModule::new().min_durability(Durability::Volatile)
}

fn needs() -> Needs {
    Needs {
        required: vec!["kv.get".into(), "kv.put".into()],
        order: Vec::new(),
    }
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "configurant-transfer-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn the_same_backend_on_both_ends_is_refused() {
    let end = || TransferEnd::new("memory", Endpoint::None);
    let err =
        run(module().transfer(&end(), &end(), &needs(), &TransferOptions::default())).unwrap_err();
    assert_eq!(err, TransferError::SameBackend("memory".to_owned()));
}

#[test]
fn an_unknown_backend_id_is_refused_naming_the_side() {
    let from = TransferEnd::new("nonesuch", Endpoint::None);
    let to = TransferEnd::new("memory", Endpoint::None);
    let err =
        run(module().transfer(&from, &to, &needs(), &TransferOptions::default())).unwrap_err();
    assert!(
        matches!(&err, TransferError::Open { side: "source", reason } if reason.contains("nonesuch")),
        "{err:?}"
    );
}

#[test]
fn a_wrong_kind_of_endpoint_is_refused_before_anything_is_created() {
    let dir = scratch_dir("wrong");
    let from = TransferEnd::new("fs", Endpoint::None); // fs needs a path
    let to = TransferEnd::new("memory", Endpoint::path(&dir));
    let err =
        run(module().transfer(&from, &to, &needs(), &TransferOptions::default())).unwrap_err();
    assert!(
        matches!(err, TransferError::Open { side: "source", .. }),
        "{err:?}"
    );
    assert!(!dir.exists(), "nothing may be created");
}

#[test]
fn a_secret_endpoint_is_in_no_error_and_no_debug() {
    let url = "postgres://appuser:hunter2hunter2@db.internal/prod";
    // An fs backend refuses a URL endpoint; the URL must not come back in the refusal.
    let from = TransferEnd::new("fs", Endpoint::Url(url.to_owned()));
    let to = TransferEnd::new("memory", Endpoint::None);
    let err =
        run(module().transfer(&from, &to, &needs(), &TransferOptions::default())).unwrap_err();
    for text in [
        format!("{err}"),
        format!("{err:?}"),
        format!("{from:?}"),
        format!("{to:?}"),
    ] {
        assert!(
            !text.contains("hunter2") && !text.contains("db.internal"),
            "leaked: {text}"
        );
    }
}

#[test]
fn records_move_from_a_real_fs_backend_through_the_registry_and_read_back() {
    let dir = scratch_dir("fs");
    let n = 1000;
    run(async {
        let gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .expect("open fs");
        for i in 0..n {
            let value = if i % 7 == 0 {
                Vec::new()
            } else {
                vec![i as u8, 0, 255, (i >> 8) as u8]
            };
            h.kv()
                .put(&format!("rec/{i:05}"), value, None)
                .await
                .unwrap();
        }
    });
    let from = TransferEnd::new("fs", Endpoint::path(&dir));
    let to = TransferEnd::new("memory", Endpoint::None);
    let report = run(module().transfer(&from, &to, &needs(), &TransferOptions::default()));
    let _ = std::fs::remove_dir_all(&dir);
    let report = report.expect("transfer");
    assert_eq!((report.copied, report.verified), (n, n), "{report}");
    assert!(
        report.to_string().contains("leases"),
        "the report says what is not copied"
    );
}

#[cfg(feature = "persistant-blocking")]
#[test]
fn the_blocking_wrapper_runs_the_same_transfer() {
    let dir = scratch_dir("blocking");
    run(async {
        let gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .unwrap();
        h.kv().put("a", b"1".to_vec(), None).await.unwrap();
    });
    let from = TransferEnd::new("fs", Endpoint::path(&dir));
    let to = TransferEnd::new("memory", Endpoint::None);
    let report = module().transfer_blocking(&from, &to, &needs(), &TransferOptions::default());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(report.expect("transfer").verified, 1);
}

/// Every report of a progress run, as `(phase, done, total)`.
fn progress_of(
    src: &MemoryKv,
    dst: &MemoryKv,
    options: &TransferOptions,
) -> (Vec<(TransferPhase, u64, Option<u64>)>, bool) {
    let mut seen = Vec::new();
    let result = run(copy_between_with_progress(
        src,
        dst,
        options,
        &mut |p: TransferProgress| {
            seen.push((p.phase, p.done, p.total));
        },
    ));
    (seen, result.is_ok())
}

fn expected(n: u64, total: Option<u64>) -> Vec<(TransferPhase, u64, Option<u64>)> {
    let mut out = Vec::new();
    for phase in [TransferPhase::Copy, TransferPhase::Verify] {
        for done in 0..=n {
            out.push((phase, done, total));
        }
    }
    out
}

#[test]
fn progress_counts_records_through_the_copy_and_then_the_read_back() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 5);
    let (seen, ok) = progress_of(&src, &dst, &TransferOptions::default());
    assert!(ok, "the copy finished");
    assert!(
        seen == expected(5, None),
        "start of each phase, then every record"
    );
}

#[test]
fn progress_has_a_total_only_when_asked_for() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 4);
    let (seen, ok) = progress_of(&src, &dst, &TransferOptions::default().count_first(true));
    assert!(ok, "the copy finished");
    assert!(
        seen == expected(4, Some(4)),
        "total known from the first report"
    );
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 4);
    let (seen, _) = progress_of(&src, &dst, &TransferOptions::default());
    assert!(
        seen.iter().all(|(_, _, total)| total.is_none()),
        "no total without count_first"
    );
}

#[test]
fn a_copy_that_fails_halfway_reported_how_far_it_got_and_a_refusal_reports_nothing() {
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 6);
    let mut wrap = Wrap::new(&dst);
    wrap.fail_put_after = Some(2);
    let mut seen = Vec::new();
    let result = run(copy_between_with_progress(
        &src,
        &wrap,
        &TransferOptions::default(),
        &mut |p| seen.push((p.phase, p.done)),
    ));
    assert!(result.is_err(), "the third write fails");
    assert!(
        seen.last() == Some(&(TransferPhase::Copy, 2)),
        "the last report is the two that were written"
    );
    assert!(
        seen.iter().all(|(phase, _)| *phase == TransferPhase::Copy),
        "it never reached the read-back"
    );
    // A target that already holds records is refused before any work: nothing to report.
    let (src, dst) = (MemoryKv::new(), MemoryKv::new());
    fill(&src, 2);
    fill(&dst, 1);
    let (seen, ok) = progress_of(&src, &dst, &TransferOptions::default());
    assert!(!ok, "refused");
    assert!(seen.is_empty(), "no report before a refusal");
}

#[cfg(feature = "persistant-blocking")]
#[test]
fn the_blocking_wrapper_reports_progress_to_its_caller() {
    let dir = scratch_dir("blocking-progress");
    run(async {
        let gate = ProviderNeeds::new(Vec::new(), Durability::Volatile);
        let h = registry::open_named("fs", &Endpoint::path(&dir), &gate)
            .await
            .unwrap();
        for k in ["a", "b", "c"] {
            h.kv().put(k, b"1".to_vec(), None).await.unwrap();
        }
    });
    let from = TransferEnd::new("fs", Endpoint::path(&dir));
    let to = TransferEnd::new("memory", Endpoint::None);
    let mut last_copy = 0;
    let report = module().transfer_blocking_with_progress(
        &from,
        &to,
        &needs(),
        &TransferOptions::default().count_first(true),
        &mut |p| {
            if p.phase == TransferPhase::Copy {
                last_copy = p.done;
            }
            assert!(p.total == Some(3), "the total is 3 in every report");
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(report.expect("transfer").copied, 3);
    assert_eq!(last_copy, 3);
}
