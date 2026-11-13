//! A small thread-safe memo table for lookups that are expensive to compute and asked for again
//! and again (parsing a prototype, solving a task-wide bound).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// Maps a key to a computed value (`None` included, so "not found" is remembered too).
///
/// Errors are never stored: a failed computation is retried on the next call. When the table
/// holds `limit` entries it is emptied, which bounds memory for unbounded key spaces
/// (e.g. randomly sampled tasks). Cloning gives an empty table.
#[derive(Debug)]
pub struct Memo<T> {
    entries: Mutex<HashMap<String, Option<T>>>,
    limit: usize,
}

impl<T> Memo<T> {
    /// A table that grows without bound (fine when the keys come from a fixed data set).
    pub fn new() -> Self {
        Self::with_limit(usize::MAX)
    }

    /// A table that is emptied when it reaches `limit` entries.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            limit,
        }
    }
}

impl<T: Clone> Memo<T> {
    /// The remembered value for `key`, or the result of `compute`, which is remembered when it
    /// succeeds. `compute` runs without the lock held, so a slow computation blocks no one.
    pub fn get_or_compute<E>(
        &self,
        key: &str,
        compute: impl FnOnce() -> Result<Option<T>, E>,
    ) -> Result<Option<T>, E> {
        if let Some(found) = self.lock().get(key) {
            return Ok(found.clone());
        }
        let value = compute()?;
        let mut entries = self.lock();
        if entries.len() >= self.limit {
            entries.clear();
        }
        entries.insert(key.to_owned(), value.clone());
        Ok(value)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Option<T>>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<T> Default for Memo<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self::with_limit(self.limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_once_and_remembers_absence() {
        let memo = Memo::<u32>::new();
        let mut calls = 0;
        for _ in 0..3 {
            let found: Result<_, ()> = memo.get_or_compute("a", || {
                calls += 1;
                Ok(None)
            });
            assert_eq!(found, Ok(None));
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn errors_are_not_remembered_and_the_limit_empties_the_table() {
        let memo = Memo::<u32>::with_limit(2);
        assert_eq!(memo.get_or_compute("a", || Err("boom")), Err("boom"));
        assert_eq!(
            memo.get_or_compute::<&str>("a", || Ok(Some(1))),
            Ok(Some(1))
        );
        memo.get_or_compute::<&str>("b", || Ok(Some(2))).unwrap();
        memo.get_or_compute::<&str>("c", || Ok(Some(3))).unwrap(); // clears, then stores "c"
        let mut recomputed = false;
        memo.get_or_compute::<&str>("a", || {
            recomputed = true;
            Ok(Some(1))
        })
        .unwrap();
        assert!(recomputed);
    }
}
