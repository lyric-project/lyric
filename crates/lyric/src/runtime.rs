use lyric_utils::err::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime as TokioRawRuntime};

#[derive(Clone, Debug)]
pub struct TokioRuntime {
    pub runtime: Arc<TokioRawRuntime>,
}

impl TokioRuntime {
    pub fn new(worker_threads: usize) -> Result<Self, Error> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .enable_all()
            .thread_name_fn(|| {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("lyric-event-thread-{}", id)
            })
            .build()
            .map_err(|e| Error::InternalError(e.to_string()))?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }
}
