// Minimal Prometheus text exporter (SPEC.md §6) — no client crate:
// counters + a render-duration histogram + cache gauge.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub struct Metrics {
    // format: 0=png 1=webp 2=svg 3=gif ; cache: 0=miss 1=hit
    requests: [[AtomicU64; 2]; 4],
    bad_requests: AtomicU64,
    render_nanos: Mutex<(u64, [u64; BUCKETS.len() + 1])>, // (count, bucket sums)
    cache_entries: AtomicU64,
}

pub const FORMATS: [&str; 4] = ["png", "webp", "svg", "gif"];
const BUCKETS: [f64; 8] = [1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 125.0, 250.0];

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests: Default::default(),
            bad_requests: AtomicU64::new(0),
            render_nanos: Mutex::new((0, [0; BUCKETS.len() + 1])),
            cache_entries: AtomicU64::new(0),
        }
    }

    pub fn request(&self, format: usize, cache_hit: bool) {
        self.requests[format][cache_hit as usize].fetch_add(1, Ordering::Relaxed);
    }
    pub fn bad_request(&self) {
        self.bad_requests.fetch_add(1, Ordering::Relaxed);
    }
    pub fn observe_render(&self, start: Instant) {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut g = self.render_nanos.lock().unwrap();
        g.0 += 1;
        for (i, b) in BUCKETS.iter().enumerate() {
            if ms <= *b {
                g.1[i] += 1;
            }
        }
        *g.1.last_mut().unwrap() += 1; // +Inf
    }
    pub fn set_cache_entries(&self, n: u64) {
        self.cache_entries.store(n, Ordering::Relaxed);
    }

    pub fn render(&self) -> String {
        let mut o = String::with_capacity(1024);
        o.push_str("# HELP boardimage_requests_total Rendered requests.\n# TYPE boardimage_requests_total counter\n");
        for (fi, f) in FORMATS.iter().enumerate() {
            for (ci, c) in ["miss", "hit"].iter().enumerate() {
                let v = self.requests[fi][ci].load(Ordering::Relaxed);
                if v > 0 {
                    o.push_str(&format!(
                        "boardimage_requests_total{{format=\"{f}\",cache=\"{c}\"}} {v}\n"
                    ));
                }
            }
        }
        o.push_str(&format!(
            "# HELP boardimage_bad_requests_total Invalid requests.\n# TYPE boardimage_bad_requests_total counter\nboardimage_bad_requests_total {}\n",
            self.bad_requests.load(Ordering::Relaxed)
        ));
        o.push_str("# HELP boardimage_render_seconds Render duration.\n# TYPE boardimage_render_seconds histogram\n");
        let g = self.render_nanos.lock().unwrap();
        for (i, b) in BUCKETS.iter().enumerate() {
            o.push_str(&format!(
                "boardimage_render_seconds_bucket{{le=\"{b}\"}} {}\n",
                g.1[i]
            ));
        }
        o.push_str(&format!(
            "boardimage_render_seconds_bucket{{le=\"+Inf\"}} {}\n",
            g.1[BUCKETS.len()]
        ));
        o.push_str(&format!("boardimage_render_seconds_count {}\n", g.0));
        o.push_str(&format!(
            "# HELP boardimage_cache_entries Cached results.\n# TYPE boardimage_cache_entries gauge\nboardimage_cache_entries {}\n",
            self.cache_entries.load(Ordering::Relaxed)
        ));
        o
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
