//! Future composition operations with fuel tracking
//!
//! This module provides fuel-aware combinators for composing futures,
//! enabling complex async workflows with deterministic fuel consumption.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use wrt_foundation::{
    operations::{Type as OperationType, record_global_operation},
    verification::VerificationLevel,
};

use crate::{
    async_::{
        fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
        fuel_aware_waker::create_fuel_aware_waker,
    },
    prelude::*,
};

/// Fuel costs for future operations
const FUTURE_SELECT_FUEL: u64 = 8;
const FUTURE_CHAIN_FUEL: u64 = 6;
const FUTURE_JOIN_FUEL: u64 = 10;
const FUTURE_RACE_FUEL: u64 = 12;
const FUTURE_MAP_FUEL: u64 = 4;
const FUTURE_TIMEOUT_FUEL: u64 = 15;

/// A future that selects the first of two futures to complete
pub struct FuelSelect<F1, F2> {
    future1: Option<F1>,
    future2: Option<F2>,
    fuel_consumed: u64,
    fuel_budget: u64,
    verification_level: VerificationLevel,
}

impl<F1, F2> FuelSelect<F1, F2>
where
    F1: Future,
    F2: Future<Output = F1::Output>,
{
    /// Create a new select combinator
    pub fn new(
        future1: F1,
        future2: F2,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            future1: Some(future1),
            future2: Some(future2),
            fuel_consumed: 0,
            fuel_budget,
            verification_level,
        }
    }
}

impl<F1, F2> Future for FuelSelect<F1, F2>
where
    F1: Future + Unpin,
    F2: Future<Output = F1::Output> + Unpin,
{
    type Output = Result<F1::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Consume fuel for select operation
        if let Err(e) = self.consume_fuel(FUTURE_SELECT_FUEL) {
            return Poll::Ready(Err(e));
        }

        // Poll first future
        if let Some(future1) = &mut self.future1 {
            match Pin::new(future1).poll(cx) {
                Poll::Ready(output) => {
                    self.future1 = None;
                    self.future2 = None;
                    return Poll::Ready(Ok(output));
                },
                Poll::Pending => {},
            }
        }

        // Poll second future
        if let Some(future2) = &mut self.future2 {
            match Pin::new(future2).poll(cx) {
                Poll::Ready(output) => {
                    self.future1 = None;
                    self.future2 = None;
                    return Poll::Ready(Ok(output));
                },
                Poll::Pending => {},
            }
        }

        Poll::Pending
    }
}

impl<F1, F2> FuelSelect<F1, F2> {
    fn consume_fuel(&mut self, base_cost: u64) -> Result<()> {
        let adjusted_cost = OperationType::fuel_cost_for_operation(
            OperationType::FutureOperation,
            self.verification_level,
        )?;

        let total_cost = base_cost.saturating_add(adjusted_cost);

        if self.fuel_consumed.saturating_add(total_cost) > self.fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Future combinator fuel budget exceeded",
            ));
        }

        self.fuel_consumed = self.fuel_consumed.saturating_add(total_cost);
        record_global_operation(OperationType::FutureOperation, self.verification_level);
        Ok(())
    }
}

/// A future that chains two futures sequentially
pub struct FuelChain<F1, F2, T> {
    state: ChainState<F1, F2, T>,
    fuel_consumed: u64,
    fuel_budget: u64,
    verification_level: VerificationLevel,
}

enum ChainState<F1, F2, T> {
    First(F1, fn(T) -> F2),
    Second(F2),
    Done,
}

impl<F1, F2, T> FuelChain<F1, F2, T>
where
    F1: Future<Output = T>,
    F2: Future,
{
    /// Create a new chain combinator
    pub fn new(
        future1: F1,
        map_fn: fn(T) -> F2,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            state: ChainState::First(future1, map_fn),
            fuel_consumed: 0,
            fuel_budget,
            verification_level,
        }
    }
}

impl<F1, F2, T> Future for FuelChain<F1, F2, T>
where
    F1: Future<Output = T> + Unpin,
    F2: Future + Unpin,
    T: Unpin,
{
    type Output = Result<F2::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Consume fuel for chain operation
        if let Err(e) = self.consume_fuel(FUTURE_CHAIN_FUEL) {
            return Poll::Ready(Err(e));
        }

        loop {
            match &mut self.state {
                ChainState::First(future1, map_fn) => match Pin::new(future1).poll(cx) {
                    Poll::Ready(output) => {
                        let map_fn = *map_fn;
                        let future2 = map_fn(output);
                        self.state = ChainState::Second(future2);
                    },
                    Poll::Pending => return Poll::Pending,
                },
                ChainState::Second(future2) => match Pin::new(future2).poll(cx) {
                    Poll::Ready(output) => {
                        self.state = ChainState::Done;
                        return Poll::Ready(Ok(output));
                    },
                    Poll::Pending => return Poll::Pending,
                },
                ChainState::Done => {
                    panic!("FuelChain polled after completion");
                },
            }
        }
    }
}

impl<F1, F2, T> FuelChain<F1, F2, T> {
    fn consume_fuel(&mut self, base_cost: u64) -> Result<()> {
        let adjusted_cost = OperationType::fuel_cost_for_operation(
            OperationType::FutureOperation,
            self.verification_level,
        )?;

        let total_cost = base_cost.saturating_add(adjusted_cost);

        if self.fuel_consumed.saturating_add(total_cost) > self.fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Future combinator fuel budget exceeded",
            ));
        }

        self.fuel_consumed = self.fuel_consumed.saturating_add(total_cost);
        record_global_operation(OperationType::FutureOperation, self.verification_level);
        Ok(())
    }
}

/// A future that joins two futures and waits for both
pub struct FuelJoin<F1, F2>
where
    F1: Future,
    F2: Future,
{
    future1: Option<F1>,
    future2: Option<F2>,
    result1: Option<F1::Output>,
    result2: Option<F2::Output>,
    fuel_consumed: u64,
    fuel_budget: u64,
    verification_level: VerificationLevel,
}

impl<F1, F2> FuelJoin<F1, F2>
where
    F1: Future,
    F2: Future,
{
    /// Create a new join combinator
    pub fn new(
        future1: F1,
        future2: F2,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            future1: Some(future1),
            future2: Some(future2),
            result1: None,
            result2: None,
            fuel_consumed: 0,
            fuel_budget,
            verification_level,
        }
    }
}

impl<F1, F2> Future for FuelJoin<F1, F2>
where
    F1: Future + Unpin,
    F2: Future + Unpin,
{
    type Output = Result<(F1::Output, F2::Output)>;

    #[allow(unsafe_code)] // Required for Pin-based Future implementation
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: FuelJoin contains only Unpin futures (F1: Unpin, F2: Unpin)
        // and other simple types (Option, u64, VerificationLevel) which are all Unpin.
        // It's safe to get mutable access without moving anything out.
        let this = unsafe { self.get_unchecked_mut() };

        // Consume fuel for join operation
        if let Err(e) = this.consume_fuel(FUTURE_JOIN_FUEL) {
            return Poll::Ready(Err(e));
        }

        // Poll first future if not complete
        if this.result1.is_none() {
            if let Some(future1) = &mut this.future1 {
                match Pin::new(future1).poll(cx) {
                    Poll::Ready(output) => {
                        this.result1 = Some(output);
                        this.future1 = None;
                    },
                    Poll::Pending => {},
                }
            }
        }

        // Poll second future if not complete
        if this.result2.is_none() {
            if let Some(future2) = &mut this.future2 {
                match Pin::new(future2).poll(cx) {
                    Poll::Ready(output) => {
                        this.result2 = Some(output);
                        this.future2 = None;
                    },
                    Poll::Pending => {},
                }
            }
        }

        // Check if both are complete
        if let (Some(result1), Some(result2)) = (this.result1.take(), this.result2.take()) {
            Poll::Ready(Ok((result1, result2)))
        } else {
            Poll::Pending
        }
    }
}

impl<F1, F2> FuelJoin<F1, F2>
where
    F1: Future,
    F2: Future,
{
    fn consume_fuel(&mut self, base_cost: u64) -> Result<()> {
        let adjusted_cost = OperationType::fuel_cost_for_operation(
            OperationType::FutureOperation,
            self.verification_level,
        )?;

        let total_cost = base_cost.saturating_add(adjusted_cost);

        if self.fuel_consumed.saturating_add(total_cost) > self.fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Future combinator fuel budget exceeded",
            ));
        }

        self.fuel_consumed = self.fuel_consumed.saturating_add(total_cost);
        record_global_operation(OperationType::FutureOperation, self.verification_level);
        Ok(())
    }
}

/// Extension trait for futures to add fuel-aware combinators
pub trait FuelFutureExt: Future + Sized {
    /// Select between this future and another
    fn fuel_select<F>(
        self,
        other: F,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> FuelSelect<Self, F>
    where
        F: Future<Output = Self::Output>,
    {
        FuelSelect::new(self, other, fuel_budget, verification_level)
    }

    /// Chain this future with another
    fn fuel_chain<F, T>(
        self,
        map_fn: fn(Self::Output) -> F,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> FuelChain<Self, F, Self::Output>
    where
        F: Future,
    {
        FuelChain::new(self, map_fn, fuel_budget, verification_level)
    }

    /// Join this future with another
    fn fuel_join<F>(
        self,
        other: F,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> FuelJoin<Self, F>
    where
        F: Future,
    {
        FuelJoin::new(self, other, fuel_budget, verification_level)
    }
}

impl<T: Future> FuelFutureExt for T {}

/// Component Model future wrapper for async operations
pub struct ComponentFuture<T> {
    /// Inner future
    inner: Pin<Box<dyn Future<Output = T> + Send + 'static>>,
    /// Component instance ID
    component_id: u64,
    /// Fuel tracking
    fuel_consumed: u64,
    fuel_budget: u64,
    /// Verification level
    verification_level: VerificationLevel,
}

impl<T> ComponentFuture<T> {
    /// Create a new component future
    pub fn new(
        future: impl Future<Output = T> + Send + 'static,
        component_id: u64,
        fuel_budget: u64,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            inner: Box::pin(future),
            component_id,
            fuel_consumed: 0,
            fuel_budget,
            verification_level,
        }
    }
}

impl<T> Future for ComponentFuture<T> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Check fuel budget
        if self.fuel_consumed >= self.fuel_budget {
            return Poll::Ready(Err(Error::resource_limit_exceeded(
                "Component future fuel exhausted",
            )));
        }

        // Poll inner future
        match self.inner.as_mut().poll(cx) {
            Poll::Ready(output) => Poll::Ready(Ok(output)),
            Poll::Pending => {
                self.fuel_consumed = self.fuel_consumed.saturating_add(1);
                Poll::Pending
            },
        }
    }
}
