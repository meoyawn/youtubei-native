use std::time::Instant;
use youtubei_native::Youtube;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args().nth(1).ok_or("Expected playlist ID")?;
    let started = Instant::now();
    let mut youtube = Youtube::new()?;
    let startup_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut scans = Vec::new();
    for _ in 0..2 {
        let scan = Instant::now();
        let videos = youtube.playlist(&id)?;
        scans.push(serde_json::json!({
            "scan_ms": scan.elapsed().as_secs_f64() * 1000.0,
            "videos": videos,
        }));
    }
    println!(
        "{}",
        serde_json::json!({"startup_ms": startup_ms, "scans": scans})
    );
    Ok(())
}
