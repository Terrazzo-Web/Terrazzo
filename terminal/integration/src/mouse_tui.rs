//! Deterministic mouse-aware TUI for browser regressions. Not a production binary.
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::Mutex;

struct RawMode(libc::termios);
impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.0);
        }
        print!("\x1b[?1000l\x1b[?1006l\x1b[?1049l");
        let _ = std::io::stdout().flush();
    }
}

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("socket path");
    let listener = UnixListener::bind(&path)?;
    let mut saved = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut saved) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut raw = saved;
    unsafe {
        libc::cfmakeraw(&mut raw);
    }
    if unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let _raw = RawMode(saved);
    print!("\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[2J\x1b[HLEFT:0\x1b[1;41HRIGHT:0");
    std::io::stdout().flush()?;
    let (mut control, _) = listener.accept()?;
    let events = Arc::new(Mutex::new(control.try_clone()?));
    writeln!(
        control,
        "{{\"event\":\"ready\",\"pid\":{},\"split_column\":41}}",
        std::process::id()
    )?;
    let output_events = events.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(control).lines() {
            let Ok(line) = line else {
                break;
            };
            if line.contains("emit_output") {
                // More than either configured replay capacity. No mode changes.
                for _ in 0..20000 {
                    print!("\x1b[5;1H{}", "x".repeat(70));
                }
                print!("\x1b[6;1HOUTPUT_COMPLETE");
                std::io::stdout().flush().unwrap();
                writeln!(
                    output_events.lock().unwrap(),
                    "{{\"event\":\"output_complete\"}}"
                )
                .unwrap();
            } else if line.contains("disable_mouse") {
                print!("\x1b[?1000l\x1b[?1006l");
                std::io::stdout().flush().unwrap();
            }
        }
    });
    let mut pending = Vec::new();
    let mut counters = [0i32; 2];
    let mut input = [0u8; 256];
    loop {
        let n = std::io::stdin().read(&mut input)?;
        if n == 0 {
            break;
        }
        pending.extend_from_slice(&input[..n]);
        while let Some(end) = pending.iter().position(|b| *b == b'M' || *b == b'm') {
            let report: Vec<_> = pending.drain(..=end).collect();
            let Some(start) = report.windows(3).position(|b| b == b"\x1b[<") else {
                continue;
            };
            let text = String::from_utf8_lossy(&report[start + 3..report.len() - 1]);
            let fields: Vec<u16> = text.split(';').filter_map(|n| n.parse().ok()).collect();
            if fields.len() != 3 || !matches!(fields[0], 64 | 65) {
                continue;
            }
            let panel = usize::from(fields[1] >= 41);
            counters[panel] += if fields[0] == 64 { -1 } else { 1 };
            print!(
                "\x1b[1;{}H{}:{}   ",
                if panel == 0 { 1 } else { 41 },
                if panel == 0 { "LEFT" } else { "RIGHT" },
                counters[panel]
            );
            std::io::stdout().flush()?;
            writeln!(
                events.lock().unwrap(),
                "{{\"event\":\"mouse_report\",\"button\":{},\"column\":{},\"row\":{},\"panel\":{},\"counter\":{}}}",
                fields[0],
                fields[1],
                fields[2],
                panel,
                counters[panel]
            )?;
        }
        if pending.len() > 4096 {
            pending.clear();
        }
    }
    let _ = std::fs::remove_file(path);
    Ok(())
}
