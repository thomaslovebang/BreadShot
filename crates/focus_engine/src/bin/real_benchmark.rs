use focusflow_domain::{CameraTimeline, FocusReason};
use focusflow_focus_engine::SmartFocusEngine;
use serde::Deserialize;
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::ExitCode,
};

const EPSILON: f64 = 0.000_001;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    name: String,
    target_cases: usize,
    required_publishable: usize,
    cases: Vec<RealCase>,
}

#[derive(Debug, Deserialize)]
struct RealCase {
    id: String,
    title: String,
    trace: String,
    focus_policy: FocusPolicy,
    max_focus_entries: usize,
    max_zoom: f64,
    required_reason: Option<String>,
    review: Review,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FocusPolicy {
    Required,
    Optional,
    Forbidden,
}

#[derive(Debug, Deserialize)]
struct Review {
    publishable: Option<bool>,
}

#[derive(Debug)]
enum CaseStatus {
    Missing,
    Passed,
    Failed(String),
}

fn default_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks")
        .join("real-v2")
        .join("manifest.json")
}

fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let file = File::open(path)
        .map_err(|error| format!("cannot open manifest {}: {error}", path.display()))?;
    let manifest: Manifest = serde_json::from_reader(BufReader::new(file))
        .map_err(|error| format!("invalid manifest {}: {error}", path.display()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported manifest schema version {}",
            manifest.schema_version
        ));
    }
    if manifest.cases.len() != manifest.target_cases {
        return Err(format!(
            "manifest declares {} target cases but contains {}",
            manifest.target_cases,
            manifest.cases.len()
        ));
    }
    if manifest.required_publishable > manifest.target_cases {
        return Err("required publishable count exceeds target case count".to_string());
    }
    let mut ids = HashSet::new();
    for case in &manifest.cases {
        if !ids.insert(&case.id) {
            return Err(format!("duplicate case id: {}", case.id));
        }
        if !case.max_zoom.is_finite() || case.max_zoom < 1.0 {
            return Err(format!("{} has an invalid max_zoom", case.id));
        }
    }
    Ok(())
}

fn focus_entry_count(timeline: &CameraTimeline) -> usize {
    timeline
        .segments
        .iter()
        .filter(|segment| segment.from.zoom <= 1.0 + EPSILON && segment.to.zoom > 1.0 + EPSILON)
        .count()
}

fn reason_name(reason: FocusReason) -> &'static str {
    match reason {
        FocusReason::FullFrame => "full_frame",
        FocusReason::Dwell => "dwell",
        FocusReason::Click => "click",
        FocusReason::DoubleClick => "double_click",
        FocusReason::Drag => "drag",
        FocusReason::Typing => "typing",
        FocusReason::Scroll => "scroll",
        FocusReason::Transition => "transition",
    }
}

fn evaluate(case: &RealCase, timeline: &CameraTimeline) -> Result<(), String> {
    for (index, segment) in timeline.segments.iter().enumerate() {
        if segment.start_ms >= segment.end_ms || segment.end_ms > timeline.duration_ms {
            return Err(format!("segment {index} has invalid timing"));
        }
        if index > 0 && timeline.segments[index - 1].end_ms > segment.start_ms {
            return Err(format!("segment {index} overlaps its predecessor"));
        }
        for pose in [segment.from, segment.to] {
            if !pose.zoom.is_finite()
                || pose.zoom < 1.0
                || !pose.center.x.is_finite()
                || !pose.center.y.is_finite()
                || !(0.0..=1.0).contains(&pose.center.x)
                || !(0.0..=1.0).contains(&pose.center.y)
            {
                return Err(format!("segment {index} has an invalid camera pose"));
            }
        }
    }

    let entries = focus_entry_count(timeline);
    match case.focus_policy {
        FocusPolicy::Required if entries == 0 => {
            return Err("expected at least one focus entry".to_string());
        }
        FocusPolicy::Forbidden if entries != 0 => {
            return Err(format!("expected no focus, found {entries} entries"));
        }
        FocusPolicy::Required | FocusPolicy::Optional | FocusPolicy::Forbidden => {}
    }
    if entries > case.max_focus_entries {
        return Err(format!(
            "focus entry count {entries} exceeds limit {}",
            case.max_focus_entries
        ));
    }
    let actual_max_zoom = timeline
        .segments
        .iter()
        .flat_map(|segment| [segment.from.zoom, segment.to.zoom])
        .fold(1.0_f64, f64::max);
    if actual_max_zoom > case.max_zoom + EPSILON {
        return Err(format!(
            "max zoom {actual_max_zoom:.2} exceeds limit {:.2}",
            case.max_zoom
        ));
    }
    if let Some(required) = &case.required_reason
        && !timeline
            .segments
            .iter()
            .any(|segment| reason_name(segment.reason) == required)
    {
        return Err(format!("required reason '{required}' was not produced"));
    }
    Ok(())
}

fn run(manifest_path: &Path, require_gate: bool) -> Result<bool, String> {
    let manifest = load_manifest(manifest_path)?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| "manifest has no parent directory".to_string())?;
    let engine = SmartFocusEngine::default();
    let mut results = Vec::with_capacity(manifest.cases.len());

    for case in &manifest.cases {
        let trace_path = root.join(&case.trace);
        let status = if !trace_path.is_file() {
            CaseStatus::Missing
        } else {
            let trace = File::open(&trace_path)
                .map_err(|error| format!("cannot open {}: {error}", trace_path.display()))?;
            match engine
                .analyze_trace(BufReader::new(trace))
                .and_then(|timeline| evaluate(case, &timeline))
            {
                Ok(()) => CaseStatus::Passed,
                Err(error) => CaseStatus::Failed(error),
            }
        };
        results.push((case, status));
    }

    println!("# {} evidence report\n", manifest.name);
    println!("| Case | Structural | Human review | Title |");
    println!("| --- | --- | --- | --- |");
    for (case, status) in &results {
        let structural = match status {
            CaseStatus::Missing => "MISSING".to_string(),
            CaseStatus::Passed => "PASS".to_string(),
            CaseStatus::Failed(error) => format!("FAIL: {error}"),
        };
        let review = match case.review.publishable {
            Some(true) => "PUBLISHABLE",
            Some(false) => "REJECTED",
            None => "UNREVIEWED",
        };
        println!("| {} | {structural} | {review} | {} |", case.id, case.title);
    }

    let captured = results
        .iter()
        .filter(|(_, status)| !matches!(status, CaseStatus::Missing))
        .count();
    let structural_pass = results
        .iter()
        .filter(|(_, status)| matches!(status, CaseStatus::Passed))
        .count();
    let publishable = results
        .iter()
        .filter(|(case, status)| {
            matches!(status, CaseStatus::Passed) && case.review.publishable == Some(true)
        })
        .count();
    let gate_passed = captured == manifest.target_cases
        && structural_pass == manifest.target_cases
        && publishable >= manifest.required_publishable;

    println!("\nCaptured: {captured}/{}", manifest.target_cases);
    println!("Structural pass: {structural_pass}/{}", manifest.target_cases);
    println!(
        "Human publishable: {publishable}/{} required",
        manifest.required_publishable
    );
    println!("Release gate: {}", if gate_passed { "PASS" } else { "NOT READY" });

    if require_gate && !gate_passed {
        return Ok(false);
    }
    if results
        .iter()
        .any(|(_, status)| matches!(status, CaseStatus::Failed(_)))
    {
        return Ok(false);
    }
    Ok(true)
}

fn main() -> ExitCode {
    let mut manifest = default_manifest_path();
    let mut require_gate = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--manifest" => {
                let Some(path) = args.next() else {
                    eprintln!("--manifest requires a path");
                    return ExitCode::FAILURE;
                };
                manifest = PathBuf::from(path);
            }
            "--require-gate" => require_gate = true,
            "--help" | "-h" => {
                println!(
                    "Usage: real_benchmark [--manifest PATH] [--require-gate]\n\
                     Missing traces are reported but only fail with --require-gate."
                );
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("unknown argument: {argument}");
                return ExitCode::FAILURE;
            }
        }
    }

    match run(&manifest, require_gate) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_manifest_is_a_unique_twenty_case_gate() {
        let manifest = load_manifest(&default_manifest_path()).unwrap();
        assert_eq!(manifest.target_cases, 20);
        assert_eq!(manifest.required_publishable, 18);
        assert_eq!(manifest.cases.len(), 20);
    }
}
