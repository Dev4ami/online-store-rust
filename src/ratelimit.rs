//! Rate-limit sederhana per-IP (fixed window, in-memory).
//!
//! Dipakai membatasi percobaan login/registrasi agar brute-force password
//! tak leluasa (argon2 memperlambat per-percobaan, ini membatasi lajunya).
//! Cukup untuk satu instance; untuk multi-instance/HA pakai store bersama
//! (mis. Redis). Di belakang reverse proxy IP soket = IP proxy; setel
//! `TRUST_PROXY_HEADERS=true` agar `client_ip` membaca IP klien asli dari
//! `X-Forwarded-For` (lihat fn tsb).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tentukan IP klien untuk rate-limit.
///
/// `peer` = IP soket (yang terlihat langsung; di belakang proxy = IP proxy).
/// Bila `trust_proxy` true DAN `forwarded_for` ada, ambil entri **paling kanan**
/// dari `X-Forwarded-For`. Alasan keamanan: dengan satu reverse proxy tepercaya
/// (Traefik/Coolify), proxy meng-append IP soket yang ia terima di posisi paling
/// kanan → itulah IP klien asli. Entri di kiri bisa dipalsukan klien, jadi TAK
/// dipakai. Bila `trust_proxy` false, header diabaikan sepenuhnya (pakai `peer`)
/// agar app yang terekspos langsung tak bisa dikelabui header palsu.
pub fn client_ip(peer: IpAddr, forwarded_for: Option<&str>, trust_proxy: bool) -> IpAddr {
    if trust_proxy {
        if let Some(xff) = forwarded_for {
            if let Some(ip) = xff
                .rsplit(',')
                .map(str::trim)
                .find_map(|s| s.parse::<IpAddr>().ok())
            {
                return ip;
            }
        }
    }
    peer
}

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

    #[test]
    fn client_ip_ignores_header_when_untrusted() {
        let peer: IpAddr = "10.0.0.9".parse().unwrap();
        // Header ada tapi trust_proxy=false → wajib pakai peer.
        let got = client_ip(peer, Some("1.2.3.4"), false);
        assert_eq!(got, peer);
    }

    #[test]
    fn client_ip_uses_peer_when_no_header() {
        let peer: IpAddr = "10.0.0.9".parse().unwrap();
        assert_eq!(client_ip(peer, None, true), peer);
    }

    #[test]
    fn client_ip_takes_rightmost_forwarded() {
        let peer: IpAddr = "10.0.0.9".parse().unwrap();
        // Klien memalsu "1.1.1.1" di kiri; proxy tepercaya append "203.0.113.7"
        // di kanan → itu yang dipakai.
        let got = client_ip(peer, Some("1.1.1.1, 203.0.113.7"), true);
        assert_eq!(got, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn client_ip_falls_back_on_garbage_header() {
        let peer: IpAddr = "10.0.0.9".parse().unwrap();
        // Nilai tak valid → skip, jatuh ke peer.
        assert_eq!(client_ip(peer, Some("not-an-ip"), true), peer);
    }
}
