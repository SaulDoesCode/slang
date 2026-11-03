// Natives for s. This file is placed inside the generated program as `mod ext`,
// so Rt, Val and the runtime helpers are in scope.

use std::time::{SystemTime, UNIX_EPOCH};

fn sqrt(rt: &mut Rt) {
    let x = rt.pop_num("sqrt");
    rt.push_num(x.sqrt());
}

fn now(rt: &mut Rt) {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    rt.push_num(ms as f64);
}

fn env(rt: &mut Rt) {
    let k = rt.pop_text("env");
    rt.push_text(std::env::var(k).unwrap_or_else(|_| "404".into()));
}

fn hex(rt: &mut Rt) {
    let x = rt.pop_num("hex");
    rt.push_text(format!("{:x}", x as i64));
}

fn primes(rt: &mut Rt) {
    let n = rt.pop_num("primes") as usize;
    let mut sieve = vec![true; n + 1];
    for i in 2..=n {
        if sieve[i] {
            rt.push_num(i as f64);
            let mut j = i * i;
            while j <= n {
                sieve[j] = false;
                j += i;
            }
        }
    }
}

pub static NATIVES: &[(&str, NativeFn)] = &[
    ("sqrt", sqrt),
    ("now", now),
    ("env", env),
    ("hex", hex),
    ("primes", primes),
];
