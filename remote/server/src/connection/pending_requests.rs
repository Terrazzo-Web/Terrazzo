use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use tower::Service;
use tower::load::CompleteOnResponse;
use tower::load::Load;
use tower::load::completion::TrackCompletionFuture;

/// A [Service] that keeps a count of ongoing requests.
///
/// This is used by the load-balancing algorithm.
#[derive(Clone)]
pub struct PendingRequests<S> {
    service: S,
    ref_count: Arc<()>,
}

impl<S> PendingRequests<S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            ref_count: Arc::default(),
        }
    }
}

impl<S, R> Service<R> for PendingRequests<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TrackCompletionFuture<S::Future, CompleteOnResponse, Arc<()>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: R) -> Self::Future {
        TrackCompletionFuture::new(
            CompleteOnResponse::default(),
            self.ref_count.clone(),
            self.service.call(req),
        )
    }
}

impl<S> Load for PendingRequests<S> {
    type Metric = usize;

    fn load(&self) -> Self::Metric {
        Arc::strong_count(&self.ref_count) - 1
    }
}
