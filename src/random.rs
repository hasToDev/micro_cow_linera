// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Mutex, OnceLock};
use rand::{rngs::StdRng, Rng, SeedableRng};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

pub fn custom_getrandom(buf: &mut [u8], seed: [u8; 32]) -> Result<(), getrandom::Error> {
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock")
        .fill(buf);
    Ok(())
}

pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}