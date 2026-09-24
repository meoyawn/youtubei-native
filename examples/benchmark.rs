use std::time::Instant;
use youtubei_native::Youtube;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut youtube = Youtube::new()?;
    let startup = started.elapsed();
    let input = serde_json::json!({ "simpleText": "native ✓ 🎵\u{0}text" });
    let iterations = 1_000;
    let started = Instant::now();
    for _ in 0..iterations {
        assert_eq!(youtube.parse_text(&input)?, "native ✓ 🎵\u{0}text");
    }
    let parsing = started.elapsed();
    let sessions = 100;
    let started = Instant::now();
    for _ in 0..sessions {
        assert_eq!(youtube.session_client_name()?, "WEB");
    }
    let creating_sessions = started.elapsed();
    println!(
        "{}",
        serde_json::json!({
            "startup_ms": startup.as_secs_f64() * 1_000.0,
            "parse_iterations": iterations,
            "parse_us_per_call": parsing.as_secs_f64() * 1_000_000.0 / iterations as f64,
            "session_iterations": sessions,
            "session_us_per_call": creating_sessions.as_secs_f64() * 1_000_000.0 / sessions as f64,
        })
    );
    Ok(())
}
