//! Engine tracing is muted across the process while a solver owns a guard.
//! A process-wide counter also covers Rayon workers and overlapping searches.
use std::sync::atomic::{AtomicUsize, Ordering};

static MUTE_DEPTH: AtomicUsize = AtomicUsize::new(0);

#[must_use = "dropping the guard restores engine output"]
pub(crate) struct MuteGuard<'a> {
    depth: &'a AtomicUsize,
}

impl<'a> MuteGuard<'a> {
    fn new(depth: &'a AtomicUsize) -> Self {
        depth
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .expect("engine output mute depth overflow");
        Self { depth }
    }
}

impl Drop for MuteGuard<'_> {
    fn drop(&mut self) {
        self.depth.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn mute() -> MuteGuard<'static> {
    MuteGuard::new(&MUTE_DEPTH)
}

pub(crate) fn enabled() -> bool {
    MUTE_DEPTH.load(Ordering::SeqCst) == 0
}

// Keep argument evaluation inside the branch: formatting can be expensive and
// may borrow game data, even when nothing will be printed.
macro_rules! println {
    ($($args:tt)*) => {
        if $crate::output::enabled() {
            std::println!($($args)*)
        }
    };
}
pub(crate) use println;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_restore_nested_and_unwinding_scopes() {
        // An isolated counter avoids races with solver tests using the global.
        let depth = AtomicUsize::new(0);
        let outer = MuteGuard::new(&depth);
        assert_eq!(depth.load(Ordering::SeqCst), 1);
        let result = std::panic::catch_unwind(|| {
            let _inner = MuteGuard::new(&depth);
            assert_eq!(depth.load(Ordering::SeqCst), 2);
            panic!("test guard unwinding");
        });
        assert!(result.is_err());
        assert_eq!(depth.load(Ordering::SeqCst), 1);
        drop(outer);
        assert_eq!(depth.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn muted_arguments_are_lazy_across_threads() {
        fn forbidden_argument() -> u8 {
            panic!("muted formatting argument evaluated")
        }
        let _guard = mute();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                assert!(!enabled());
                println!("{}", forbidden_argument());
            });
        });
    }
}
