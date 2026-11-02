//! Automatic Director benchmark v1.
//!
//! These synthetic traces protect structural camera behavior while the v0.4
//! semantic director is developed. Passing this suite is necessary, but does
//! not by itself satisfy the roadmap's visual publishability gate.

use focusflow_domain::{CameraTimeline, FocusReason, MouseEvent, MouseEventKind, NormalizedPoint};
use focusflow_focus_engine::{FocusEngine, SmartFocusEngine};

#[derive(Clone, Copy)]
enum Expectation {
    NoFocus,
    ClickFocus,
    DwellFocus,
    OneFocusEntry,
    LatestIntentOnRight,
    FocusPan,
    MaxZoom(f64),
    TimelineIntegrity,
}

struct BenchmarkCase {
    name: &'static str,
    duration_ms: u64,
    events: Vec<MouseEvent>,
    expectation: Expectation,
}

fn event(time_ms: u64, x: f64, y: f64, kind: MouseEventKind) -> MouseEvent {
    MouseEvent {
        time_ms,
        x_px: 0,
        y_px: 0,
        position: NormalizedPoint { x, y },
        speed_px_s: 0.0,
        kind,
    }
}

fn move_to(time_ms: u64, x: f64, y: f64) -> MouseEvent {
    event(time_ms, x, y, MouseEventKind::Move)
}

fn left_click(time_ms: u64, x: f64, y: f64) -> MouseEvent {
    event(time_ms, x, y, MouseEventKind::LeftClick)
}

fn right_click(time_ms: u64, x: f64, y: f64) -> MouseEvent {
    event(time_ms, x, y, MouseEventKind::RightClick)
}

fn focus_entry_count(timeline: &CameraTimeline) -> usize {
    timeline
        .segments
        .iter()
        .filter(|segment| segment.from.zoom == 1.0 && segment.to.zoom > 1.0)
        .count()
}

fn assert_case(case: &BenchmarkCase, timeline: &CameraTimeline) {
    let has_click = timeline
        .segments
        .iter()
        .any(|segment| segment.reason == FocusReason::Click && segment.to.zoom > 1.0);
    let has_dwell = timeline
        .segments
        .iter()
        .any(|segment| segment.reason == FocusReason::Dwell && segment.to.zoom > 1.0);

    match case.expectation {
        Expectation::NoFocus => assert!(
            timeline.segments.iter().all(|segment| segment.to.zoom == 1.0),
            "{} unexpectedly created focus: {timeline:#?}",
            case.name
        ),
        Expectation::ClickFocus => assert!(has_click, "{} lost its click intent", case.name),
        Expectation::DwellFocus => assert!(has_dwell, "{} lost its dwell intent", case.name),
        Expectation::OneFocusEntry => assert_eq!(
            focus_entry_count(timeline),
            1,
            "{} should form one stable camera sequence",
            case.name
        ),
        Expectation::LatestIntentOnRight => {
            let centers: Vec<_> = timeline
                .segments
                .iter()
                .filter(|segment| segment.to.zoom > 1.0)
                .map(|segment| segment.to.center.x)
                .collect();
            assert!(!centers.is_empty(), "{} did not focus", case.name);
            assert!(
                centers.iter().all(|x| *x > 0.5),
                "{} retained an obsolete target: {centers:?}",
                case.name
            );
        }
        Expectation::FocusPan => assert!(
            timeline.segments.iter().any(|segment| {
                segment.from.zoom > 1.0
                    && segment.to.zoom > 1.0
                    && segment.from.center != segment.to.center
                    && segment.end_ms > segment.start_ms
            }),
            "{} should pan between active targets",
            case.name
        ),
        Expectation::MaxZoom(expected) => {
            let actual = timeline
                .segments
                .iter()
                .map(|segment| segment.to.zoom)
                .fold(1.0_f64, f64::max);
            assert!(
                (actual - expected).abs() < 0.000_001,
                "{} expected max zoom {expected}, got {actual}",
                case.name
            );
        }
        Expectation::TimelineIntegrity => {
            assert_eq!(timeline.duration_ms, case.duration_ms, "{} duration drifted", case.name);
            assert!(
                timeline
                    .segments
                    .windows(2)
                    .all(|pair| pair[0].end_ms <= pair[1].start_ms),
                "{} contains overlapping or reversed segments",
                case.name
            );
            assert!(
                timeline.segments.iter().all(|segment| {
                    segment.start_ms < segment.end_ms
                        && segment.end_ms <= case.duration_ms
                        && segment.from.zoom >= 1.0
                        && segment.to.zoom >= 1.0
                        && (0.0..=1.0).contains(&segment.from.center.x)
                        && (0.0..=1.0).contains(&segment.from.center.y)
                        && (0.0..=1.0).contains(&segment.to.center.x)
                        && (0.0..=1.0).contains(&segment.to.center.y)
                }),
                "{} violates timeline bounds",
                case.name
            );
        }
    }
}

fn benchmark_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase { name: "01_idle_screen", duration_ms: 3_000, events: vec![], expectation: Expectation::NoFocus },
        BenchmarkCase { name: "02_motion_without_intent", duration_ms: 3_000, events: vec![move_to(100, 0.1, 0.1), move_to(300, 0.8, 0.8)], expectation: Expectation::NoFocus },
        BenchmarkCase { name: "03_stable_left_click", duration_ms: 3_000, events: vec![left_click(1_000, 0.5, 0.5)], expectation: Expectation::ClickFocus },
        BenchmarkCase { name: "04_stable_right_click", duration_ms: 3_000, events: vec![right_click(1_000, 0.5, 0.5)], expectation: Expectation::ClickFocus },
        BenchmarkCase { name: "05_fast_search_click_ignored", duration_ms: 3_000, events: vec![move_to(880, 0.10, 0.5), move_to(920, 0.18, 0.5), move_to(960, 0.27, 0.5), move_to(990, 0.34, 0.5), left_click(1_000, 0.36, 0.5)], expectation: Expectation::NoFocus },
        BenchmarkCase { name: "06_click_after_settle", duration_ms: 3_000, events: vec![move_to(700, 0.1, 0.5), move_to(800, 0.5, 0.5), left_click(1_000, 0.5, 0.5)], expectation: Expectation::ClickFocus },
        BenchmarkCase { name: "07_left_edge_click", duration_ms: 3_000, events: vec![left_click(1_000, 0.0, 0.5)], expectation: Expectation::TimelineIntegrity },
        BenchmarkCase { name: "08_right_edge_click", duration_ms: 3_000, events: vec![left_click(1_000, 1.0, 0.5)], expectation: Expectation::TimelineIntegrity },
        BenchmarkCase { name: "09_top_edge_click", duration_ms: 3_000, events: vec![left_click(1_000, 0.5, 0.0)], expectation: Expectation::TimelineIntegrity },
        BenchmarkCase { name: "10_bottom_edge_click", duration_ms: 3_000, events: vec![left_click(1_000, 0.5, 1.0)], expectation: Expectation::TimelineIntegrity },
        BenchmarkCase { name: "11_repeated_nearby_clicks", duration_ms: 6_000, events: vec![left_click(1_000, 0.55, 0.4), left_click(2_500, 0.55, 0.4), left_click(4_000, 0.55, 0.4)], expectation: Expectation::OneFocusEntry },
        BenchmarkCase { name: "12_rapid_conflicting_clicks", duration_ms: 3_500, events: vec![left_click(1_000, 0.2, 0.5), left_click(1_100, 0.8, 0.5)], expectation: Expectation::LatestIntentOnRight },
        BenchmarkCase { name: "13_distant_active_targets", duration_ms: 4_000, events: vec![left_click(1_000, 0.25, 0.5), left_click(1_600, 0.75, 0.5)], expectation: Expectation::FocusPan },
        BenchmarkCase { name: "14_deliberate_dwell", duration_ms: 4_000, events: vec![move_to(100, 0.3, 0.4), move_to(2_100, 0.31, 0.4)], expectation: Expectation::DwellFocus },
        BenchmarkCase { name: "15_short_pause_not_dwell", duration_ms: 2_000, events: vec![move_to(100, 0.3, 0.4), move_to(500, 0.31, 0.4)], expectation: Expectation::NoFocus },
        BenchmarkCase { name: "16_click_zoom_strength", duration_ms: 3_000, events: vec![left_click(1_000, 0.6, 0.5)], expectation: Expectation::MaxZoom(1.34) },
        BenchmarkCase { name: "17_dwell_zoom_strength", duration_ms: 4_000, events: vec![move_to(100, 0.6, 0.5), move_to(2_100, 0.61, 0.5)], expectation: Expectation::MaxZoom(1.24) },
        BenchmarkCase { name: "18_mixed_click_and_dwell", duration_ms: 4_000, events: vec![move_to(100, 0.6, 0.5), left_click(850, 0.6, 0.5), move_to(2_000, 0.61, 0.5)], expectation: Expectation::MaxZoom(1.34) },
        BenchmarkCase { name: "19_long_multi_target_trace", duration_ms: 12_000, events: vec![left_click(1_000, 0.2, 0.2), move_to(3_000, 0.5, 0.5), left_click(4_000, 0.5, 0.5), move_to(7_000, 0.8, 0.7), right_click(8_000, 0.8, 0.7)], expectation: Expectation::TimelineIntegrity },
        BenchmarkCase { name: "20_zero_duration", duration_ms: 0, events: vec![left_click(0, 0.5, 0.5)], expectation: Expectation::NoFocus },
    ]
}

#[test]
fn automatic_director_benchmark_v1() {
    let cases = benchmark_cases();
    assert_eq!(cases.len(), 20, "the v1 benchmark is a fixed 20-case gate");

    let engine = SmartFocusEngine::default();
    for case in &cases {
        let timeline = engine
            .analyze(case.duration_ms, &case.events)
            .unwrap_or_else(|error| panic!("{} failed analysis: {error}", case.name));
        assert_case(case, &timeline);
    }
}

