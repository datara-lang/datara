//! tokio reference peer for bench_natvar/RESULTS.md (PLAN_V152 N1.4).
//!
//! Modes:
//!   server <port>            — multi-threaded tokio echo server (task per conn)
//!   client <port> <msgs>     — tokio echo client: <msgs> × 128-byte round trips
//!   sleep <n> <ms>           — spawn <n> tokio tasks, each sleeping <ms>, drain
//!
//! The echo server is shared: both the tokio client and the Datara fiber
//! client (bench_natvar/echo_fiber.dtr) run against it, so the comparison
//! isolates the client-side concurrency machinery.

use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MSG: &[u8; 128] = b"natvar_bench16_!natvar_bench16_!natvar_bench16_!natvar_bench16_!natvar_bench16_!natvar_bench16_!natvar_bench16_!natvar_bench16_!";

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("server") => run_server(args[2].parse().unwrap()).await,
        Some("client") => {
            run_client(args[2].parse().unwrap(), args[3].parse().unwrap()).await
        }
        Some("sleep") => run_sleep(args[2].parse().unwrap(), args[3].parse().unwrap()).await,
        _ => {
            eprintln!("usage: tokio_echo server|client|sleep ...");
            std::process::exit(2);
        }
    }
}

async fn run_server(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    println!("LISTENING {port}");
    loop {
        let (mut sock, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                let n = sock.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                if sock.write_all(&buf[..n]).await.is_err() {
                    return;
                }
            }
        });
    }
}

async fn run_client(port: u16, msgs: u64) {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    sock.set_nodelay(true).unwrap();
    let mut buf = [0u8; 4096];
    let start = Instant::now();
    for _ in 0..msgs {
        sock.write_all(MSG).await.unwrap();
        let n = sock.read(&mut buf).await.unwrap();
        if n != MSG.len() {
            eprintln!("short echo: {n}");
            std::process::exit(1);
        }
    }
    let total = start.elapsed();
    let ms = total.as_millis();
    let rate = if ms > 0 { msgs * 1000 / ms as u64 } else { 0 };
    println!("TOTAL_MS {ms}");
    println!("RATE {rate}");
}

async fn run_sleep(n: usize, ms: u64) {
    let start = Instant::now();
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        handles.push(tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let elapsed = start.elapsed();
    println!("TOTAL_MS {}", elapsed.as_millis());
    println!("PEAK_MB {}", peak_working_set_mb());
}

/// Self-reported peak working set (same counter PowerShell's
/// PeakWorkingSet64 exposes) via K32GetProcessMemoryInfo (kernel32).
#[cfg(windows)]
fn peak_working_set_mb() -> u64 {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct PM_counters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }
    extern "system" {
        fn K32GetProcessMemoryInfo(
            h: *mut core::ffi::c_void,
            p: *mut PM_counters,
            cb: u32,
        ) -> i32;
    }
    let mut pm = PM_counters {
        cb: 0,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    pm.cb = std::mem::size_of::<PM_counters>() as u32;
    let handle = -1isize as *mut core::ffi::c_void; // GetCurrentProcess()
    unsafe {
        if K32GetProcessMemoryInfo(handle, &mut pm, pm.cb) != 0 {
            return (pm.PeakWorkingSetSize as u64) / (1024 * 1024);
        }
    }
    0
}

#[cfg(not(windows))]
fn peak_working_set_mb() -> u64 {
    0
}
