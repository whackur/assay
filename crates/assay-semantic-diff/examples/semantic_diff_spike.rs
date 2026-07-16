use std::{env, fs, hint::black_box, time::Instant};

use assay_semantic_diff::{
    Language, NativeTreeSitterEngine, SemanticDiffEngine, SemanticDiffInput,
};

const SAMPLES_DEFAULT: usize = 30;

fn main() {
    let samples = parse_samples();
    let engine = NativeTreeSitterEngine::new();
    let fixtures = fixtures();

    let cold_started = Instant::now();
    let cold_operations = run_bundle(&engine, &fixtures);
    let cold_microseconds = cold_started.elapsed().as_micros();

    let mut warm_microseconds = Vec::with_capacity(samples);
    let mut observed_operations = None;
    for _ in 0..samples {
        let started = Instant::now();
        let operations = run_bundle(&engine, &fixtures);
        warm_microseconds.push(started.elapsed().as_micros());
        observed_operations.get_or_insert(operations);
        assert_eq!(observed_operations, Some(operations));
    }
    warm_microseconds.sort_unstable();

    let max_rss_kib = linux_peak_rss_kib()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned());
    println!(
        concat!(
            "{{\"engine\":\"native-tree-sitter-1\",",
            "\"fixture_pairs_per_bundle\":{},\"operations_per_bundle\":{},",
            "\"samples\":{},\"cold_microseconds\":{},",
            "\"warm_median_microseconds\":{},\"warm_p95_microseconds\":{},",
            "\"max_rss_kib\":{}}}"
        ),
        fixtures.len(),
        cold_operations,
        samples,
        cold_microseconds,
        percentile(&warm_microseconds, 50),
        percentile(&warm_microseconds, 95),
        max_rss_kib,
    );
}

fn linux_peak_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let mut fields = line.split_ascii_whitespace();
    if fields.next()? != "VmHWM:" || fields.next_back()? != "kB" {
        return None;
    }
    fields.next()?.parse().ok()
}

fn parse_samples() -> usize {
    let mut arguments = env::args().skip(1);
    match (
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next(),
    ) {
        (None, None, None) => SAMPLES_DEFAULT,
        (Some("--samples"), Some(value), None) => value
            .parse::<usize>()
            .ok()
            .filter(|samples| *samples > 0)
            .expect("--samples must be a positive integer"),
        _ => panic!("usage: semantic_diff_spike [--samples POSITIVE_INTEGER]"),
    }
}

fn run_bundle(engine: &NativeTreeSitterEngine, fixtures: &[Fixture]) -> usize {
    fixtures
        .iter()
        .map(|fixture| {
            black_box(
                engine
                    .analyze(SemanticDiffInput::new(
                        fixture.language,
                        fixture.before,
                        fixture.after,
                    ))
                    .operations()
                    .len(),
            )
        })
        .sum()
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = (percentile * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

struct Fixture {
    language: Language,
    before: &'static [u8],
    after: &'static [u8],
}

fn fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();
    push_language(
        &mut fixtures,
        Language::JavaScript,
        include_bytes!("../tests/fixtures/javascript/before.js"),
        [
            include_bytes!("../tests/fixtures/javascript/format.js").as_slice(),
            include_bytes!("../tests/fixtures/javascript/modified.js").as_slice(),
            include_bytes!("../tests/fixtures/javascript/moved.js").as_slice(),
            include_bytes!("../tests/fixtures/javascript/renamed.js").as_slice(),
        ],
    );
    push_language(
        &mut fixtures,
        Language::TypeScript,
        include_bytes!("../tests/fixtures/typescript/before.ts"),
        [
            include_bytes!("../tests/fixtures/typescript/format.ts").as_slice(),
            include_bytes!("../tests/fixtures/typescript/modified.ts").as_slice(),
            include_bytes!("../tests/fixtures/typescript/moved.ts").as_slice(),
            include_bytes!("../tests/fixtures/typescript/renamed.ts").as_slice(),
        ],
    );
    push_language(
        &mut fixtures,
        Language::Python,
        include_bytes!("../tests/fixtures/python/before.py"),
        [
            include_bytes!("../tests/fixtures/python/format.py").as_slice(),
            include_bytes!("../tests/fixtures/python/modified.py").as_slice(),
            include_bytes!("../tests/fixtures/python/moved.py").as_slice(),
            include_bytes!("../tests/fixtures/python/renamed.py").as_slice(),
        ],
    );
    fixtures
}

fn push_language(
    fixtures: &mut Vec<Fixture>,
    language: Language,
    before: &'static [u8],
    variants: [&'static [u8]; 4],
) {
    fixtures.extend(variants.into_iter().map(|after| Fixture {
        language,
        before,
        after,
    }));
}
