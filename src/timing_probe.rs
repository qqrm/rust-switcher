#![allow(
    clippy::multiple_crate_versions,
    reason = "The Windows stack currently requires parallel transitive versions."
)]

use std::{fmt::Write as _, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrialKind {
    Double,
    Triple,
}

impl TrialKind {
    fn target_count(self) -> usize {
        match self {
            Self::Double => 2,
            Self::Triple => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Double => "double",
            Self::Triple => "triple",
        }
    }
}

#[derive(Clone, Debug)]
struct TrialReport {
    kind: TrialKind,
    releases_ms: Vec<u64>,
}

impl TrialReport {
    fn intervals_ms(&self) -> Vec<u64> {
        self.releases_ms
            .windows(2)
            .map(|pair| pair[1].saturating_sub(pair[0]))
            .collect()
    }

    fn valid(&self) -> bool {
        self.releases_ms.len() == self.kind.target_count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Stats {
    count: usize,
    min: u64,
    p50: u64,
    p75: u64,
    p90: u64,
    p95: u64,
    max: u64,
    mean: u64,
}

fn summarize(values: &[u64]) -> Option<Stats> {
    if values.is_empty() {
        return None;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let sum: u64 = sorted.iter().sum();

    Some(Stats {
        count: sorted.len(),
        min: sorted[0],
        p50: percentile_nearest_rank(&sorted, 50),
        p75: percentile_nearest_rank(&sorted, 75),
        p90: percentile_nearest_rank(&sorted, 90),
        p95: percentile_nearest_rank(&sorted, 95),
        max: *sorted.last().expect("values is not empty"),
        mean: (sum as f64 / sorted.len() as f64).round() as u64,
    })
}

fn percentile_nearest_rank(sorted: &[u64], percentile: u64) -> u64 {
    debug_assert!(!sorted.is_empty());
    let rank = ((percentile as f64 / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn double_gaps(reports: &[TrialReport]) -> Vec<u64> {
    reports
        .iter()
        .filter(|trial| trial.kind == TrialKind::Double && trial.valid())
        .filter_map(|trial| trial.intervals_ms().first().copied())
        .collect()
}

fn triple_gap_12(reports: &[TrialReport]) -> Vec<u64> {
    reports
        .iter()
        .filter(|trial| trial.kind == TrialKind::Triple && trial.valid())
        .filter_map(|trial| trial.intervals_ms().first().copied())
        .collect()
}

fn triple_gap_23(reports: &[TrialReport]) -> Vec<u64> {
    reports
        .iter()
        .filter(|trial| trial.kind == TrialKind::Triple && trial.valid())
        .filter_map(|trial| trial.intervals_ms().get(1).copied())
        .collect()
}

fn triple_spans(reports: &[TrialReport]) -> Vec<u64> {
    reports
        .iter()
        .filter(|trial| trial.kind == TrialKind::Triple && trial.valid())
        .filter_map(|trial| {
            let first = trial.releases_ms.first()?;
            let last = trial.releases_ms.last()?;
            Some(last.saturating_sub(*first))
        })
        .collect()
}

fn round_up_to(value: u64, step: u64) -> u64 {
    if step == 0 {
        return value;
    }
    value.div_ceil(step) * step
}

fn clamp(value: u64, min: u64, max: u64) -> u64 {
    value.max(min).min(max)
}

fn recommended_sequence_gap_ms(reports: &[TrialReport]) -> Option<u64> {
    let mut gaps = double_gaps(reports);
    gaps.extend(triple_gap_12(reports));
    gaps.extend(triple_gap_23(reports));

    let stats = summarize(&gaps)?;
    Some(clamp(round_up_to(stats.p95 + 75, 25), 225, 700))
}

fn recommended_defer_after_second_ms(reports: &[TrialReport]) -> Option<u64> {
    let stats = summarize(&triple_gap_23(reports))?;
    Some(clamp(round_up_to(stats.p95 + 60, 25), 175, 600))
}

fn render_text_report(key_label: &str, reports: &[TrialReport]) -> String {
    let mut out = String::new();
    let valid = reports.iter().filter(|trial| trial.valid()).count();
    let invalid = reports.len().saturating_sub(valid);

    writeln!(&mut out, "RustSwitcher hotkey timing probe").ok();
    writeln!(&mut out, "Key: {key_label}").ok();
    writeln!(&mut out, "Valid trials: {valid}").ok();
    writeln!(&mut out, "Invalid trials: {invalid}").ok();
    writeln!(&mut out).ok();

    write_stats(&mut out, "Double gap 1->2", &double_gaps(reports));
    write_stats(&mut out, "Triple gap 1->2", &triple_gap_12(reports));
    write_stats(&mut out, "Triple gap 2->3", &triple_gap_23(reports));
    write_stats(&mut out, "Triple span 1->3", &triple_spans(reports));

    writeln!(&mut out).ok();
    match recommended_sequence_gap_ms(reports) {
        Some(ms) => {
            writeln!(
                &mut out,
                "Recommended max_gap_ms if using one shared setting: {ms} ms"
            )
            .ok();
        }
        None => {
            writeln!(
                &mut out,
                "Recommended max_gap_ms if using one shared setting: not enough valid samples"
            )
            .ok();
        }
    }
    match recommended_defer_after_second_ms(reports) {
        Some(ms) => {
            writeln!(&mut out, "Recommended deferred double-tap cap: {ms} ms").ok();
        }
        None => {
            writeln!(
                &mut out,
                "Recommended deferred double-tap cap: not enough valid triple samples"
            )
            .ok();
        }
    }

    writeln!(&mut out).ok();
    writeln!(&mut out, "Trials:").ok();
    for (idx, trial) in reports.iter().enumerate() {
        let intervals = trial
            .intervals_ms()
            .iter()
            .map(|ms| format!("{ms} ms"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            &mut out,
            "{:02}. {:6} releases={} valid={} intervals=[{}]",
            idx + 1,
            trial.kind.label(),
            trial.releases_ms.len(),
            trial.valid(),
            intervals
        )
        .ok();
    }

    out
}

fn write_stats(out: &mut String, label: &str, values: &[u64]) {
    match summarize(values) {
        Some(stats) => {
            writeln!(
                out,
                "{label}: n={} min={} p50={} p75={} p90={} p95={} max={} mean={} ms",
                stats.count,
                stats.min,
                stats.p50,
                stats.p75,
                stats.p90,
                stats.p95,
                stats.max,
                stats.mean
            )
            .ok();
        }
        None => {
            writeln!(out, "{label}: no valid samples").ok();
        }
    }
}

fn render_json_report(key_label: &str, reports: &[TrialReport]) -> String {
    let mut out = String::new();
    writeln!(&mut out, "{{").ok();
    writeln!(&mut out, "  \"key\": \"{}\",", json_escape(key_label)).ok();
    writeln!(
        &mut out,
        "  \"recommended_current_max_gap_ms\": {},",
        optional_u64_json(recommended_sequence_gap_ms(reports))
    )
    .ok();
    writeln!(
        &mut out,
        "  \"recommended_split_defer_after_second_ms\": {},",
        optional_u64_json(recommended_defer_after_second_ms(reports))
    )
    .ok();
    writeln!(&mut out, "  \"trials\": [").ok();

    for (idx, trial) in reports.iter().enumerate() {
        let comma = if idx + 1 == reports.len() { "" } else { "," };
        writeln!(&mut out, "    {{").ok();
        writeln!(&mut out, "      \"kind\": \"{}\",", trial.kind.label()).ok();
        writeln!(&mut out, "      \"valid\": {},", trial.valid()).ok();
        writeln!(
            &mut out,
            "      \"releases_ms\": {},",
            json_u64_array(&trial.releases_ms)
        )
        .ok();
        writeln!(
            &mut out,
            "      \"intervals_ms\": {}",
            json_u64_array(&trial.intervals_ms())
        )
        .ok();
        writeln!(&mut out, "    }}{comma}").ok();
    }

    writeln!(&mut out, "  ]").ok();
    writeln!(&mut out, "}}").ok();
    out
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn json_u64_array(values: &[u64]) -> String {
    let values = values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn write_reports(
    report_dir: &Path,
    key_label: &str,
    reports: &[TrialReport],
) -> std::io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    std::fs::create_dir_all(report_dir)?;
    let text_path = report_dir.join("latest.txt");
    let json_path = report_dir.join("latest.json");
    std::fs::write(&text_path, render_text_report(key_label, reports))?;
    std::fs::write(&json_path, render_json_report(key_label, reports))?;
    Ok((text_path, json_path))
}

#[cfg(windows)]
mod win {
    use std::{
        io::{self, Write},
        sync::{
            Mutex, OnceLock,
            atomic::{AtomicIsize, AtomicU32, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                MAPVK_VSC_TO_VK_EX, MapVirtualKeyW, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
                VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT,
            },
            WindowsAndMessaging::{
                CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
                LLKHF_EXTENDED, LLKHF_INJECTED, MSG, PostThreadMessageW, SetWindowsHookExW,
                TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
                WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
            },
        },
    };

    use super::{TrialKind, TrialReport, render_text_report, write_reports};

    static EVENTS: OnceLock<Mutex<Vec<KeyEvent>>> = OnceLock::new();
    static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

    const DEFAULT_DOUBLE_TRIALS: usize = 8;
    const DEFAULT_TRIPLE_TRIALS: usize = 8;
    const DEFAULT_QUIET_AFTER_MS: u64 = 550;
    const DEFAULT_TIMEOUT_MS: u64 = 5_000;

    #[derive(Clone, Copy, Debug)]
    enum KeyFilter {
        Left,
        Right,
        Any,
    }

    impl KeyFilter {
        fn label(self) -> &'static str {
            match self {
                Self::Left => "Left Shift",
                Self::Right => "Right Shift",
                Self::Any => "Any Shift",
            }
        }

        fn matches(self, vk: u32) -> bool {
            match self {
                Self::Left => vk == u32::from(VK_LSHIFT.0),
                Self::Right => vk == u32::from(VK_RSHIFT.0),
                Self::Any => vk == u32::from(VK_LSHIFT.0) || vk == u32::from(VK_RSHIFT.0),
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct KeyEvent {
        vk: u32,
        up: bool,
        time_ms: u64,
    }

    #[derive(Clone, Copy, Debug)]
    struct Options {
        key: KeyFilter,
        double_trials: usize,
        triple_trials: usize,
        quiet_after_ms: u64,
        timeout_ms: u64,
    }

    impl Default for Options {
        fn default() -> Self {
            Self {
                key: KeyFilter::Left,
                double_trials: DEFAULT_DOUBLE_TRIALS,
                triple_trials: DEFAULT_TRIPLE_TRIALS,
                quiet_after_ms: DEFAULT_QUIET_AFTER_MS,
                timeout_ms: DEFAULT_TIMEOUT_MS,
            }
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let options = parse_args()?;
        print_intro(options);

        let _hook = HookThread::install()?;
        let mut reports = Vec::new();
        let mut trial_no = 1usize;
        let total_trials = options.double_trials + options.triple_trials;

        for _ in 0..options.double_trials {
            if !run_trial_prompt(
                trial_no,
                total_trials,
                TrialKind::Double,
                options,
                &mut reports,
            )? {
                break;
            }
            trial_no += 1;
        }
        for _ in 0..options.triple_trials {
            if !run_trial_prompt(
                trial_no,
                total_trials,
                TrialKind::Triple,
                options,
                &mut reports,
            )? {
                break;
            }
            trial_no += 1;
        }

        let report_dir = std::env::current_dir()?.join("target").join("timing-probe");
        let (text_path, json_path) = write_reports(&report_dir, options.key.label(), &reports)?;

        println!();
        println!("{}", render_text_report(options.key.label(), &reports));
        println!("Report written:");
        println!("  {}", text_path.display());
        println!("  {}", json_path.display());
        println!();
        println!("Send latest.txt back into the thread and we can tighten the runtime delay.");

        Ok(())
    }

    fn parse_args() -> Result<Options, Box<dyn std::error::Error>> {
        let mut options = Options::default();
        let mut args = std::env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                "--key" => {
                    let value = args.next().ok_or("--key requires a value")?;
                    options.key = match value.as_str() {
                        "left" | "left-shift" | "lshift" => KeyFilter::Left,
                        "right" | "right-shift" | "rshift" => KeyFilter::Right,
                        "any" | "any-shift" => KeyFilter::Any,
                        _ => {
                            return Err(format!(
                                "invalid --key value '{value}', expected left/right/any"
                            )
                            .into());
                        }
                    };
                }
                "--double-trials" => {
                    options.double_trials = parse_usize_arg(&arg, args.next())?;
                }
                "--triple-trials" => {
                    options.triple_trials = parse_usize_arg(&arg, args.next())?;
                }
                "--quiet-ms" => {
                    options.quiet_after_ms = parse_u64_arg(&arg, args.next())?;
                }
                "--timeout-ms" => {
                    options.timeout_ms = parse_u64_arg(&arg, args.next())?;
                }
                _ => return Err(format!("unknown argument '{arg}'").into()),
            }
        }

        if options.double_trials == 0 && options.triple_trials == 0 {
            return Err("at least one trial is required".into());
        }

        Ok(options)
    }

    fn parse_usize_arg(
        name: &str,
        value: Option<String>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        value
            .ok_or_else(|| format!("{name} requires a value"))?
            .parse::<usize>()
            .map_err(|e| format!("invalid {name} value: {e}").into())
    }

    fn parse_u64_arg(name: &str, value: Option<String>) -> Result<u64, Box<dyn std::error::Error>> {
        value
            .ok_or_else(|| format!("{name} requires a value"))?
            .parse::<u64>()
            .map_err(|e| format!("invalid {name} value: {e}").into())
    }

    fn print_help() {
        println!("Usage: rust-switcher-timing [options]");
        println!();
        println!("Options:");
        println!("  --key left|right|any      Shift key to measure (default: left)");
        println!(
            "  --double-trials N         Number of 2-tap trials (default: {DEFAULT_DOUBLE_TRIALS})"
        );
        println!(
            "  --triple-trials N         Number of 3-tap trials (default: {DEFAULT_TRIPLE_TRIALS})"
        );
        println!(
            "  --quiet-ms N              Quiet window after target tap count (default: {DEFAULT_QUIET_AFTER_MS})"
        );
        println!("  --timeout-ms N            Timeout per trial (default: {DEFAULT_TIMEOUT_MS})");
    }

    fn print_intro(options: Options) {
        println!("RustSwitcher hotkey timing probe");
        println!();
        println!("Measured key: {}", options.key.label());
        println!(
            "Plan: {} double-tap trials, {} triple-tap trials.",
            options.double_trials, options.triple_trials
        );
        println!(
            "The probe records Shift key-up timestamps because modifier-only hotkeys are matched on key release."
        );
        println!("Type q and press Enter at a trial prompt to stop early.");
        println!();
    }

    fn run_trial_prompt(
        trial_no: usize,
        total_trials: usize,
        kind: TrialKind,
        options: Options,
        reports: &mut Vec<TrialReport>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        println!(
            "[{trial_no}/{total_trials}] Press Enter, then tap {} {} times.",
            options.key.label(),
            kind.target_count()
        );
        print!("Ready> ");
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if line.trim().eq_ignore_ascii_case("q") {
            return Ok(false);
        }

        clear_events();
        println!("GO");
        let releases = collect_releases(kind.target_count(), options)?;
        let report = TrialReport {
            kind,
            releases_ms: normalize_release_times(&releases),
        };

        if report.valid() {
            println!(
                "Captured {}: intervals [{}]",
                kind.label(),
                format_intervals(&report.intervals_ms())
            );
        } else {
            println!(
                "Invalid {} trial: captured {} releases, expected {}.",
                kind.label(),
                report.releases_ms.len(),
                kind.target_count()
            );
        }

        reports.push(report);
        println!();
        Ok(true)
    }

    fn collect_releases(
        target_count: usize,
        options: Options,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        let started = Instant::now();
        let mut target_reached_at = None;

        loop {
            let releases = matching_releases(options.key);
            if releases.len() >= target_count && target_reached_at.is_none() {
                target_reached_at = Some(Instant::now());
            }
            if let Some(reached_at) = target_reached_at
                && reached_at.elapsed() >= Duration::from_millis(options.quiet_after_ms)
            {
                return Ok(releases);
            }
            if started.elapsed() >= Duration::from_millis(options.timeout_ms) {
                return Ok(releases);
            }

            thread::sleep(Duration::from_millis(5));
        }
    }

    fn normalize_release_times(releases: &[u64]) -> Vec<u64> {
        let Some(first) = releases.first().copied() else {
            return Vec::new();
        };
        releases
            .iter()
            .map(|release| release.saturating_sub(first))
            .collect()
    }

    fn format_intervals(intervals: &[u64]) -> String {
        intervals
            .iter()
            .map(|ms| format!("{ms} ms"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn matching_releases(key: KeyFilter) -> Vec<u64> {
        events()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|event| event.up && key.matches(event.vk))
            .map(|event| event.time_ms)
            .collect()
    }

    fn clear_events() {
        events().lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn events() -> &'static Mutex<Vec<KeyEvent>> {
        EVENTS.get_or_init(|| Mutex::new(Vec::new()))
    }

    struct HookThread {
        handle: Option<thread::JoinHandle<()>>,
    }

    impl HookThread {
        fn install() -> Result<Self, Box<dyn std::error::Error>> {
            events();
            let (tx, rx) = mpsc::channel();
            let handle = thread::spawn(move || {
                let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
                HOOK_THREAD_ID.store(thread_id, Ordering::Release);

                let hook =
                    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) } {
                        Ok(hook) => hook,
                        Err(e) => {
                            let _ = tx.send(Err(format!("failed to install keyboard hook: {e}")));
                            return;
                        }
                    };

                HOOK_HANDLE.store(hook.0 as isize, Ordering::Release);
                let _ = tx.send(Ok(()));

                let mut msg = MSG::default();
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }

                let raw = HOOK_HANDLE.swap(0, Ordering::AcqRel);
                if raw != 0 {
                    unsafe {
                        let _ = UnhookWindowsHookEx(HHOOK(raw as *mut _));
                    }
                }
            });

            match rx.recv_timeout(Duration::from_secs(3)) {
                Ok(Ok(())) => Ok(Self {
                    handle: Some(handle),
                }),
                Ok(Err(e)) => {
                    let _ = handle.join();
                    Err(e.into())
                }
                Err(e) => {
                    let _ = handle.join();
                    Err(format!("keyboard hook thread did not initialize: {e}").into())
                }
            }
        }
    }

    impl Drop for HookThread {
        fn drop(&mut self) {
            let thread_id = HOOK_THREAD_ID.load(Ordering::Acquire);
            if thread_id != 0 {
                unsafe {
                    let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
                }
            }

            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hook = current_hook();
        if code == HC_ACTION.cast_signed() {
            let Ok(msg) = u32::try_from(wparam.0) else {
                return unsafe { CallNextHookEx(hook, code, wparam, lparam) };
            };

            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN || msg == WM_KEYUP || msg == WM_SYSKEYUP {
                let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
                if !kb.flags.contains(LLKHF_INJECTED) {
                    let vk = normalize_vk(kb);
                    if vk == u32::from(VK_LSHIFT.0) || vk == u32::from(VK_RSHIFT.0) {
                        events()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(KeyEvent {
                                vk,
                                up: msg == WM_KEYUP || msg == WM_SYSKEYUP,
                                time_ms: u64::from(kb.time),
                            });
                    }
                }
            }
        }

        unsafe { CallNextHookEx(hook, code, wparam, lparam) }
    }

    fn current_hook() -> Option<HHOOK> {
        let raw = HOOK_HANDLE.load(Ordering::Acquire);
        (raw != 0).then_some(HHOOK(raw as *mut _))
    }

    fn normalize_vk(kb: &KBDLLHOOKSTRUCT) -> u32 {
        let vk = kb.vkCode;
        let extended = kb.flags.contains(LLKHF_EXTENDED);

        match vk {
            x if x == u32::from(VK_SHIFT.0) => {
                let mapped = unsafe { MapVirtualKeyW(kb.scanCode, MAPVK_VSC_TO_VK_EX) };
                if mapped != 0 { mapped } else { vk }
            }
            x if x == u32::from(VK_CONTROL.0) => {
                if extended {
                    u32::from(VK_RCONTROL.0)
                } else {
                    u32::from(VK_LCONTROL.0)
                }
            }
            x if x == u32::from(VK_MENU.0) => {
                if extended {
                    u32::from(VK_RMENU.0)
                } else {
                    u32::from(VK_LMENU.0)
                }
            }
            _ => vk,
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(e) = win::run() {
        eprintln!("rust-switcher-timing failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rust-switcher-timing is only available on Windows.");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_use_nearest_rank_percentiles() {
        let stats = summarize(&[100, 120, 130, 160, 300]).expect("stats");

        assert_eq!(stats.count, 5);
        assert_eq!(stats.min, 100);
        assert_eq!(stats.p50, 130);
        assert_eq!(stats.p75, 160);
        assert_eq!(stats.p90, 300);
        assert_eq!(stats.p95, 300);
        assert_eq!(stats.max, 300);
        assert_eq!(stats.mean, 162);
    }

    #[test]
    fn recommendations_use_double_and_triple_release_gaps() {
        let reports = vec![
            TrialReport {
                kind: TrialKind::Double,
                releases_ms: vec![0, 130],
            },
            TrialReport {
                kind: TrialKind::Triple,
                releases_ms: vec![0, 120, 260],
            },
        ];

        assert_eq!(recommended_sequence_gap_ms(&reports), Some(225));
        assert_eq!(recommended_defer_after_second_ms(&reports), Some(200));
    }

    #[test]
    fn invalid_trials_are_excluded_from_recommendations() {
        let reports = vec![
            TrialReport {
                kind: TrialKind::Double,
                releases_ms: vec![0, 100, 200],
            },
            TrialReport {
                kind: TrialKind::Triple,
                releases_ms: vec![0, 100],
            },
        ];

        assert_eq!(recommended_sequence_gap_ms(&reports), None);
        assert_eq!(recommended_defer_after_second_ms(&reports), None);
    }

    #[test]
    fn json_report_contains_trials() {
        let reports = vec![TrialReport {
            kind: TrialKind::Double,
            releases_ms: vec![0, 125],
        }];

        let json = render_json_report("Left Shift", &reports);

        assert!(json.contains("\"key\": \"Left Shift\""));
        assert!(json.contains("\"kind\": \"double\""));
        assert!(json.contains("\"intervals_ms\": [125]"));
    }
}
