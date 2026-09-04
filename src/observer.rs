//! Observe the results of each iteration
//!
//! This modules defines a trait that can be implemented on any type which intends to observe the
//! results of each iteration of benchmark. The observer can do whatever it wishes with the
//! results.
//!
//! # Overview
//! [`Observer`] receives a reference to [`IterReport`] via [`BenchResult`]. It can handle the
//! results as it sees fit. [`Observer`]s can also be chained by calling [`ObserverExt::with`]
//! on any observer. This will call the newer observer first and then the original observer.
use futures::channel::mpsc;
use futures::future::OptionFuture;

use crate::{BenchError, IterReport};

/// This defines a type that is interested in results of each iteration of a bench. It will get a
/// notification of those results
pub trait Observer {
    /// This method will be called when one iteration of a bench is complete with the reference
    /// of iteration results
    fn notify(&self, result: Result<&IterReport, &BenchError>) -> impl Future<Output = ()> + Send;
}

/// An extension trait for [`Observer`] so that it can add more layers to the observer as needed
pub trait ObserverExt: Sized {
    /// Add another layer of [`Observer`] to observe the results of a [`crate::BenchSuite`]
    /// iteration and hence process [`IterReport`]
    ///
    /// # Example
    /// ```
    /// # use rlt::observer::ObserverExt as _;
    ///
    /// let empty = Some(());
    /// let chain = empty.with(());
    /// ```
    fn with<L: Observer>(self, layer: L) -> Layered<L, Self> {
        Layered { current: layer, inner: self }
    }
}

impl<T: Observer> ObserverExt for T {}

/// An layer in observation stack. It holds the current [`Observer`] and the lower stack. So will
/// pass the result to this layer and then the lower stack
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layered<L, I> {
    current: L,
    inner: I,
}

impl<L, I> Observer for Layered<L, I>
where
    L: Observer,
    I: Observer,
{
    fn notify(&self, result: Result<&IterReport, &BenchError>) -> impl Future<Output = ()> + Send {
        let current = self.current.notify(result.clone());
        let inner = self.inner.notify(result);
        async move {
            current.await;
            inner.await
        }
    }
}

impl Observer for () {
    async fn notify(&self, _: Result<&IterReport, &BenchError>) {}
}

impl<T> Observer for Option<T>
where
    T: Observer,
{
    fn notify(&self, result: Result<&IterReport, &BenchError>) -> impl Future<Output = ()> + Send {
        let fut: OptionFuture<_> = self.as_ref().map(|v| v.notify(result)).into();
        async move {
            fut.await;
        }
    }
}

#[derive(Debug, derive_more::From, Clone)]
pub(crate) struct MpscObserver(mpsc::UnboundedSender<Result<IterReport, String>>);

impl Observer for MpscObserver {
    async fn notify(&self, result: Result<&IterReport, &BenchError>) {
        let result = result.map(Clone::clone).map_err(ToString::to_string);
        if let Err(error) = self.0.unbounded_send(result) {
            log::warn!("Failed to send IterReport; error={error}");
        }
    }
}
