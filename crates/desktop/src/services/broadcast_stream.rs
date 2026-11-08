//! Runtime-independent asynchronous event bridges. No GUI event-loop ownership.
use futures::{Stream, channel::mpsc};
use std::sync::OnceLock;
use tokio::sync::broadcast;

pub fn channel<T, F, Fut>(capacity: usize, run: F) -> impl Stream<Item = T>
where
    T: Send + 'static,
    F: FnOnce(mpsc::Sender<T>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel(capacity);
    tokio::spawn(run(tx));
    rx
}

pub fn from_once_lock<T: Clone + Send + 'static>(
    source: &'static OnceLock<broadcast::Receiver<T>>,
    label: &'static str,
) -> impl Stream<Item = T> {
    from_receiver(source.get().map(|rx| rx.resubscribe()), label)
}

pub fn from_receiver<T: Clone + Send + 'static>(
    receiver: Option<broadcast::Receiver<T>>,
    label: &'static str,
) -> impl Stream<Item = T> {
    futures::stream::unfold(receiver, move |receiver| async move {
        let mut receiver = receiver?;
        loop {
            match receiver.recv().await {
                Ok(event) => return Some((event, Some(receiver))),
                Err(broadcast::error::RecvError::Closed) => return None,
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    tracing::warn!(count, label, "event receiver lagged")
                }
            }
        }
    })
}
