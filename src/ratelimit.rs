//! Rate-limit sederhana per-IP (fixed window, in-memory).
//!
//! Dipakai membatasi percobaan login/registrasi agar brute-force password
//! tak leluasa (argon2 memperlambat per-percobaan, ini membatasi lajunya).
//! Cukup untuk satu instance; untuk multi-instance/HA pakai store bersama
//! (mis. Redis). Di belakang reverse proxy, IP yang terlihat = IP proxy —
//! teruskan IP asli via header bila perlu granularitas per-klien.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<IpAddr, (u32, Instant)>>,
}

impl RateLimiter {
    /// `max` percobaan per `window`.
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Catat satu percobaan dari `ip`. `true` bila masih dalam batas,
    /// `false` bila melebihi (harus ditolak). Entri kedaluwarsa dibersihkan
    /// oportunistik tiap panggilan agar map tak tumbuh tanpa batas.
    pub fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut map = self.hits.lock().unwrap_or_else(|e| e.into_inner());

        // Buang entri yang jendelanya sudah lewat.
        map.retain(|_, (_, start)| now.duration_since(*start) < self.window);

        let entry = map.entry(ip).or_insert((0, now));
        // Jendela lewat → reset penghitung.
        if now.duration_since(entry.1) >= self.window {
            *entry = (0, now);
        }
        if entry.0 >= self.max {
            return false;
        }
        entry.0 += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let rl = RateLimiter::new(3, Duration::from_secs(60));
        assert!(rl.check(ip)); // 1
        assert!(rl.check(ip)); // 2
        assert!(rl.check(ip)); // 3
        assert!(!rl.check(ip)); // 4 → ditolak
    }

    #[test]
    fn ips_independent() {
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let b: IpAddr = "10.0.0.2".parse().unwrap();
        let rl = RateLimiter::new(1, Duration::from_secs(60));
        assert!(rl.check(a));
        assert!(!rl.check(a)); // a habis
        assert!(rl.check(b)); // b masih punya jatah sendiri
    }

    #[test]
    fn resets_after_window() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let rl = RateLimiter::new(1, Duration::from_millis(30));
        assert!(rl.check(ip));
        assert!(!rl.check(ip));
        std::thread::sleep(Duration::from_millis(45));
        assert!(rl.check(ip)); // jendela baru
    }
}
