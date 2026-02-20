//! Stream handling with fuel tracking for Component Model async operations
//!
//! This module provides fuel-aware stream processing for WebAssembly
//! components, enabling deterministic async streaming with timing guarantees.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use wrt_foundation::{
    CrateId, MemoryProvider,
    collections::{StaticMap as BoundedMap, StaticQueue as BoundedDeque, StaticVec as BoundedVec},
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::{Checksum, VerificationLevel},
};

use wrt_foundation::safe_memory::NoStdProvider;

use crate::{
    async_::{
        fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
        fuel_aware_waker::create_fuel_aware_waker,
    },
    prelude::*,
};

/// Maximum items in a stream buffer
const MAX_STREAM_BUFFER: usize = 256;

/// Fuel costs for stream operations
const STREAM_CREATE_FUEL: u64 = 10;
const STREAM_POLL_FUEL: u64 = 5;
const STREAM_YIELD_FUEL: u64 = 3;
const STREAM_CLOSE_FUEL: u64 = 8;
const STREAM_ITEM_FUEL: u64 = 2;

/// Stream state for async processing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is active and can produce items
    Active,
    /// Stream is waiting for items
    Waiting,
    /// Stream has completed
    Completed,
    /// Stream encountered an error
    Failed,
    /// Stream was cancelled
    Cancelled,
}

/// A fuel-tracked stream for Component Model
#[derive(Debug)]
pub struct FuelStream<T> {
    /// Stream identifier
    pub id: u64,
    /// Stream state
    pub state: StreamState,
    /// Buffered items
    pub buffer: BoundedDeque<T, MAX_STREAM_BUFFER>,
    /// Total fuel consumed by this stream
    pub fuel_consumed: u64,
    /// Fuel budget for this stream
    pub fuel_budget: u64,
    /// Verification level for fuel tracking
    pub verification_level: VerificationLevel,
    /// Waker for async notifications
    pub waker: Option<core::task::Waker>,
}

impl<T> FuelStream<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    /// Create a new fuel-tracked stream
    pub fn new(id: u64, fuel_budget: u64, verification_level: VerificationLevel) -> Result<Self> {
        let buffer = BoundedDeque::new();

        // Record stream creation
        record_global_operation(OperationType::StreamCreate, verification_level);

        Ok(Self {
            id,
            state: StreamState::Active,
            buffer,
            fuel_consumed: STREAM_CREATE_FUEL,
            fuel_budget,
            verification_level,
            waker: None,
        })
    }

    /// Poll the stream for the next item
    pub fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        // Consume fuel for polling
        if self.consume_fuel(STREAM_POLL_FUEL).is_err() {
            self.state = StreamState::Failed;
            return Poll::Ready(None);
        }

        // Check stream state
        match self.state {
            StreamState::Completed | StreamState::Failed | StreamState::Cancelled => {
                return Poll::Ready(None);
            },
            _ => {},
        }

        // Check for buffered items
        if let Some(item) = self.buffer.pop() {
            if self.consume_fuel(STREAM_ITEM_FUEL).is_err() {
                self.state = StreamState::Failed;
                return Poll::Ready(None);
            }
            return Poll::Ready(Some(item));
        }

        // No items available, register waker
        self.waker = Some(cx.waker().clone());
        self.state = StreamState::Waiting;
        Poll::Pending
    }

    /// Yield an item to the stream
    pub fn yield_item(&mut self, item: T) -> Result<()> {
        // Check if stream is active
        if self.state != StreamState::Active && self.state != StreamState::Waiting {
            return Err(Error::async_error("Cannot yield to inactive stream"));
        }

        // Consume fuel for yielding
        self.consume_fuel(STREAM_YIELD_FUEL)?;

        // Buffer the item
        self.buffer.push(item)?;

        // Wake any waiting consumers
        if let Some(waker) = self.waker.take() {
            self.state = StreamState::Active;
            waker.wake();
        }

        Ok(())
    }

    /// Complete the stream
    pub fn complete(&mut self) -> Result<()> {
        self.consume_fuel(STREAM_CLOSE_FUEL)?;
        self.state = StreamState::Completed;

        // Wake any waiting consumers
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }

        Ok(())
    }

    /// Cancel the stream
    pub fn cancel(&mut self) -> Result<()> {
        self.consume_fuel(STREAM_CLOSE_FUEL)?;
        self.state = StreamState::Cancelled;

        // Clear buffer
        self.buffer.clear();

        // Wake any waiting consumers
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }

        Ok(())
    }

    /// Consume fuel with verification level adjustment
    fn consume_fuel(&mut self, base_cost: u64) -> Result<()> {
        let adjusted_cost = OperationType::fuel_cost_for_operation(
            OperationType::StreamOperation,
            self.verification_level,
        )?;

        let total_cost = base_cost.saturating_add(adjusted_cost);

        if self.fuel_consumed.saturating_add(total_cost) > self.fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Stream fuel budget exceeded",
            ));
        }

        self.fuel_consumed = self.fuel_consumed.saturating_add(total_cost);
        Ok(())
    }
}

/// Stream adapter for async iteration
pub struct FuelStreamAdapter<T> {
    stream: FuelStream<T>,
}

impl<T> FuelStreamAdapter<T> {
    /// Create a new stream adapter
    pub fn new(stream: FuelStream<T>) -> Self {
        Self { stream }
    }
}

impl<T> Future for FuelStreamAdapter<T>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    type Output = Option<T>;

    #[allow(unsafe_code)] // Required for Pin-based Future implementation
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: FuelStreamNext contains only Unpin types,
        // so it's safe to get mutable access without moving anything out.
        let this = unsafe { self.get_unchecked_mut() };
        this.stream.poll_next(cx)
    }
}

/// Component Model stream type for cross-component streaming
#[derive(Debug)]
pub struct ComponentStream {
    /// Stream identifier
    pub id: u64,
    /// Source component instance
    pub source_component: u64,
    /// Target component instance
    pub target_component: u64,
    /// Stream of component values
    pub value_stream:
        FuelStream<wrt_foundation::component_value::ComponentValue<NoStdProvider<4096>>>,
    /// Stream metadata
    pub metadata: StreamMetadata,
}

/// Metadata for component streams
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    /// Stream name
    pub name: String,
    /// Expected item type
    pub item_type: String,
    /// Whether the stream is bounded
    pub is_bounded: bool,
    /// Maximum number of items (if bounded)
    pub max_items: Option<usize>,
}

impl ComponentStream {
    /// Create a new component stream
    pub fn new(
        id: u64,
        source: u64,
        target: u64,
        fuel_budget: u64,
        verification_level: VerificationLevel,
        metadata: StreamMetadata,
    ) -> Result<Self> {
        let value_stream = FuelStream::new(id, fuel_budget, verification_level)?;

        Ok(Self {
            id,
            source_component: source,
            target_component: target,
            value_stream,
            metadata,
        })
    }

    /// Send a value through the stream
    pub fn send(
        &mut self,
        value: wrt_foundation::component_value::ComponentValue<NoStdProvider<4096>>,
    ) -> Result<()> {
        // Check bounded stream limits
        if self.metadata.is_bounded {
            if let Some(max_items) = self.metadata.max_items {
                if self.value_stream.buffer.len() >= max_items {
                    return Err(Error::resource_limit_exceeded(
                        "Stream buffer limit exceeded",
                    ));
                }
            }
        }

        self.value_stream.yield_item(value)
    }

    /// Receive a value from the stream
    pub async fn receive(
        &mut self,
    ) -> Option<wrt_foundation::component_value::ComponentValue<NoStdProvider<4096>>> {
        // Poll the stream directly using core::future::poll_fn
        core::future::poll_fn(|cx| self.value_stream.poll_next(cx)).await
    }
}

impl Default for ComponentStream {
    fn default() -> Self {
        Self {
            id: 0,
            source_component: 0,
            target_component: 0,
            value_stream: FuelStream {
                id: 0,
                buffer: BoundedDeque::new(),
                state: StreamState::Active,
                fuel_budget: 0,
                fuel_consumed: 0,
                verification_level: VerificationLevel::Off,
                waker: None,
            },
            metadata: StreamMetadata {
                name: String::new(),
                item_type: String::new(),
                is_bounded: false,
                max_items: None,
            },
        }
    }
}

impl Clone for ComponentStream {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            source_component: self.source_component,
            target_component: self.target_component,
            value_stream: FuelStream {
                id: self.value_stream.id,
                state: self.value_stream.state,
                buffer: self.value_stream.buffer.clone(),
                fuel_consumed: self.value_stream.fuel_consumed,
                fuel_budget: self.value_stream.fuel_budget,
                verification_level: self.value_stream.verification_level,
                waker: None, // Waker cannot be cloned
            },
            metadata: self.metadata.clone(),
        }
    }
}

impl PartialEq for ComponentStream {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.source_component == other.source_component
            && self.target_component == other.target_component
    }
}

impl Eq for ComponentStream {}

impl Checksummable for ComponentStream {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.id.update_checksum(checksum);
        self.source_component.update_checksum(checksum);
        self.target_component.update_checksum(checksum);
        // Note: value_stream and metadata contain complex types that don't all implement Checksummable
    }
}

impl ToBytes for ComponentStream {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.id.to_bytes_with_provider(writer, provider)?;
        self.source_component.to_bytes_with_provider(writer, provider)?;
        self.target_component.to_bytes_with_provider(writer, provider)?;
        // Note: Complex stream state and metadata are not serialized
        Ok(())
    }
}

impl FromBytes for ComponentStream {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let id = u64::from_bytes_with_provider(reader, provider)?;
        let source_component = u64::from_bytes_with_provider(reader, provider)?;
        let target_component = u64::from_bytes_with_provider(reader, provider)?;

        Ok(Self {
            id,
            source_component,
            target_component,
            value_stream: FuelStream {
                id,
                buffer: BoundedDeque::new(),
                state: StreamState::Active,
                fuel_budget: 0,
                fuel_consumed: 0,
                verification_level: VerificationLevel::Off,
                waker: None,
            },
            metadata: StreamMetadata {
                name: String::new(),
                item_type: String::new(),
                is_bounded: false,
                max_items: None,
            },
        })
    }
}

/// Stream manager for tracking all active streams
pub struct FuelStreamManager {
    /// Active streams by ID
    streams: BoundedMap<u64, ComponentStream, MAX_STREAM_BUFFER>,
    /// Next stream ID
    next_stream_id: u64,
    /// Global fuel budget for all streams
    global_fuel_budget: u64,
    /// Total fuel consumed by all streams
    total_fuel_consumed: u64,
}

impl FuelStreamManager {
    /// Create a new stream manager
    pub fn new(global_fuel_budget: u64) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        let streams = BoundedMap::new();

        Ok(Self {
            streams,
            next_stream_id: 1,
            global_fuel_budget,
            total_fuel_consumed: 0,
        })
    }

    /// Create a new stream
    pub fn create_stream(
        &mut self,
        source: u64,
        target: u64,
        fuel_budget: u64,
        verification_level: VerificationLevel,
        metadata: StreamMetadata,
    ) -> Result<u64> {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 1;

        // Check global fuel budget
        if self.total_fuel_consumed.saturating_add(fuel_budget) > self.global_fuel_budget {
            return Err(Error::resource_limit_exceeded(
                "Global stream fuel budget exceeded",
            ));
        }

        let stream = ComponentStream::new(
            stream_id,
            source,
            target,
            fuel_budget,
            verification_level,
            metadata,
        )?;

        self.streams.insert(stream_id, stream)?;
        self.total_fuel_consumed = self.total_fuel_consumed.saturating_add(fuel_budget);

        Ok(stream_id)
    }

    /// Get a mutable reference to a stream
    pub fn get_stream_mut(&mut self, stream_id: u64) -> Result<&mut ComponentStream> {
        self.streams
            .get_mut(&stream_id)
            .ok_or_else(|| Error::runtime_execution_error("Error occurred"))
    }

    /// Close a stream and reclaim its fuel
    pub fn close_stream(&mut self, stream_id: u64) -> Result<()> {
        if let Some(mut stream) = self.streams.remove(&stream_id) {
            stream.value_stream.complete()?;

            // Reclaim unused fuel
            let unused_fuel = stream
                .value_stream
                .fuel_budget
                .saturating_sub(stream.value_stream.fuel_consumed);
            self.total_fuel_consumed = self.total_fuel_consumed.saturating_sub(unused_fuel);
        }

        Ok(())
    }

    /// Cancel all streams for a component
    pub fn cancel_component_streams(&mut self, component_id: u64) -> Result<()> {
        let stream_ids: Vec<u64> = self
            .streams
            .iter()
            .filter(|(_, stream)| {
                stream.source_component == component_id || stream.target_component == component_id
            })
            .map(|(id, _)| *id)
            .collect();

        for stream_id in stream_ids {
            self.close_stream(stream_id)?;
        }

        Ok(())
    }
}
