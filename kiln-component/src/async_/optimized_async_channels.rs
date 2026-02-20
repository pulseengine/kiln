//! Optimized async channels for Component Model communication
//!
//! This module provides high-performance async channels with fuel tracking,
//! backpressure handling, and Component Model integration.
//!
//! **REQUIRES**: Allocation support - async channels need heap allocation for Arc/Weak

// In no_std mode, we rely on the alloc crate which is always available in wrt-component
// The alloc crate provides Arc, Weak, Box, and other heap types
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(not(feature = "std"))]
use alloc::sync::Weak;
#[cfg(feature = "std")]
use std::sync::Weak;

use core::{
    future::Future as CoreFuture,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

use wrt_foundation::{
    Arc, CrateId, Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    component_value::ComponentValue,
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ToBytes},
};
use wrt_platform::advanced_sync::Priority;

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{
    ComponentInstanceId,
    async_::{
        fuel_async_executor::AsyncTaskState,
        fuel_aware_waker::{WakeCoalescer, create_fuel_aware_waker},
        task_manager_async_bridge::{ComponentAsyncTaskType, TaskManagerAsyncBridge},
    },
    prelude::*,
};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum channel capacity
const MAX_CHANNEL_CAPACITY: usize = 1024;

/// Maximum channels per component
const MAX_CHANNELS_PER_COMPONENT: usize = 64;

/// Fuel costs for channel operations
const CHANNEL_SEND_FUEL: u64 = 15;
const CHANNEL_RECV_FUEL: u64 = 10;
const CHANNEL_CREATE_FUEL: u64 = 50;
const CHANNEL_CLOSE_FUEL: u64 = 20;

/// Optimized async channels manager
pub struct OptimizedAsyncChannels {
    /// Bridge for task management
    bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
    /// Active channels
    channels: BoundedMap<ChannelId, AsyncChannel, 512>,
    /// Component channel contexts (Box provides indirection to break recursive type)
    component_contexts: BoundedMap<ComponentInstanceId, Box<ComponentChannelContext>, 128>,
    /// Next channel ID
    next_channel_id: AtomicU64,
    /// Channel statistics
    channel_stats: ChannelStatistics,
    /// Global channel configuration
    global_config: ChannelConfiguration,
}

/// Channel identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChannelId(u64);

impl Checksummable for ChannelId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for ChannelId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ChannelId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u64::from_bytes_with_provider(reader, provider)?))
    }
}

/// Sender half of a channel
#[derive(Debug, Clone)]
pub struct ChannelSender {
    channel_id: ChannelId,
    component_id: ComponentInstanceId,
    channels_ref: Weak<Mutex<OptimizedAsyncChannels>>,
}

/// Receiver half of a channel
#[derive(Debug, Clone)]
pub struct ChannelReceiver {
    channel_id: ChannelId,
    component_id: ComponentInstanceId,
    channels_ref: Weak<Mutex<OptimizedAsyncChannels>>,
}

/// Async channel implementation
#[derive(Debug)]
struct AsyncChannel {
    id: ChannelId,
    channel_type: ChannelType,
    capacity: usize,
    buffer: ChannelBuffer,
    sender_wakers: BoundedVec<Waker, 32>,
    receiver_wakers: BoundedVec<Waker, 32>,
    closed: AtomicBool,
    sender_count: AtomicU32,
    receiver_count: AtomicU32,
    total_sent: AtomicU64,
    total_received: AtomicU64,
    fuel_consumed: AtomicU64,
    created_at: u64,
}

/// Type of async channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    /// Unbounded channel (limited by global config)
    Unbounded,
    /// Bounded channel with fixed capacity
    Bounded(usize),
    /// Single-shot channel (oneshot)
    Oneshot,
    /// Broadcast channel (multiple receivers)
    Broadcast(usize),
    /// Priority channel (ordered by priority)
    Priority,
}

impl Default for ChannelType {
    fn default() -> Self {
        Self::Unbounded
    }
}

impl Checksummable for ChannelType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            ChannelType::Unbounded => 0u8.update_checksum(checksum),
            ChannelType::Bounded(cap) => {
                1u8.update_checksum(checksum);
                cap.update_checksum(checksum);
            },
            ChannelType::Oneshot => 2u8.update_checksum(checksum),
            ChannelType::Broadcast(cap) => {
                3u8.update_checksum(checksum);
                cap.update_checksum(checksum);
            },
            ChannelType::Priority => 4u8.update_checksum(checksum),
        }
    }
}

impl ToBytes for ChannelType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            ChannelType::Unbounded => 0u8.to_bytes_with_provider(writer, provider),
            ChannelType::Bounded(cap) => {
                1u8.to_bytes_with_provider(writer, provider)?;
                cap.to_bytes_with_provider(writer, provider)
            },
            ChannelType::Oneshot => 2u8.to_bytes_with_provider(writer, provider),
            ChannelType::Broadcast(cap) => {
                3u8.to_bytes_with_provider(writer, provider)?;
                cap.to_bytes_with_provider(writer, provider)
            },
            ChannelType::Priority => 4u8.to_bytes_with_provider(writer, provider),
        }
    }
}

impl FromBytes for ChannelType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(ChannelType::Unbounded),
            1 => {
                let cap = usize::from_bytes_with_provider(reader, provider)?;
                Ok(ChannelType::Bounded(cap))
            },
            2 => Ok(ChannelType::Oneshot),
            3 => {
                let cap = usize::from_bytes_with_provider(reader, provider)?;
                Ok(ChannelType::Broadcast(cap))
            },
            4 => Ok(ChannelType::Priority),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid ChannelType discriminant",
            )),
        }
    }
}

/// Channel buffer implementation
#[derive(Debug)]
enum ChannelBuffer {
    /// Ring buffer for bounded channels
    Ring {
        data: BoundedVec<ChannelMessage, MAX_CHANNEL_CAPACITY>,
        head: AtomicUsize,
        tail: AtomicUsize,
        len: AtomicUsize,
    },
    /// Vector buffer for unbounded channels
    Vector {
        data: BoundedVec<ChannelMessage, MAX_CHANNEL_CAPACITY>,
    },
    /// Single slot for oneshot channels
    Single {
        data: Option<ChannelMessage>,
        taken: AtomicBool,
    },
    /// Priority queue for priority channels
    Priority {
        data: BoundedVec<PriorityMessage, MAX_CHANNEL_CAPACITY>,
    },
}

/// Message in a channel
#[derive(Debug, Clone, Default)]
pub struct ChannelMessage {
    value: ComponentValue<ComponentProvider>,
    sender_id: ComponentInstanceId,
    sent_at: u64,
    priority: u8,
}

impl PartialEq for ChannelMessage {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.sender_id == other.sender_id
            && self.sent_at == other.sent_at
            && self.priority == other.priority
    }
}

impl Eq for ChannelMessage {}

impl Checksummable for ChannelMessage {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.value.update_checksum(checksum);
        self.sender_id.update_checksum(checksum);
        self.sent_at.update_checksum(checksum);
        self.priority.update_checksum(checksum);
    }
}

impl ToBytes for ChannelMessage {
    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        provider: &P,
    ) -> core::result::Result<(), wrt_error::Error> {
        self.value.to_bytes_with_provider(writer, provider)?;
        self.sender_id.to_bytes_with_provider(writer, provider)?;
        self.sent_at.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ChannelMessage {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        provider: &P,
    ) -> core::result::Result<Self, wrt_error::Error> {
        let value = ComponentValue::from_bytes_with_provider(reader, provider)?;
        let sender_id = ComponentInstanceId::from_bytes_with_provider(reader, provider)?;
        let sent_at = u64::from_bytes_with_provider(reader, provider)?;
        let priority = u8::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            value,
            sender_id,
            sent_at,
            priority,
        })
    }
}

/// Priority message for priority channels
#[derive(Debug, Clone, Default)]
struct PriorityMessage {
    message: ChannelMessage,
    priority: u8,
}

impl Ord for PriorityMessage {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for PriorityMessage {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityMessage {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PriorityMessage {}

impl Checksummable for PriorityMessage {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.message.update_checksum(checksum);
        self.priority.update_checksum(checksum);
    }
}

impl ToBytes for PriorityMessage {
    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        provider: &P,
    ) -> core::result::Result<(), wrt_error::Error> {
        self.message.to_bytes_with_provider(writer, provider)?;
        self.priority.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for PriorityMessage {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        provider: &P,
    ) -> core::result::Result<Self, wrt_error::Error> {
        let message = ChannelMessage::from_bytes_with_provider(reader, provider)?;
        let priority = u8::from_bytes_with_provider(reader, provider)?;
        Ok(Self { message, priority })
    }
}

/// Component channel context
#[derive(Debug)]
struct ComponentChannelContext {
    component_id: ComponentInstanceId,
    /// Channels owned by this component
    owned_channels: BoundedVec<ChannelId, MAX_CHANNELS_PER_COMPONENT>,
    /// Senders held by this component (Box provides indirection to break recursive type)
    senders: BoundedMap<ChannelId, Box<ChannelSender>, MAX_CHANNELS_PER_COMPONENT>,
    /// Receivers held by this component (Box provides indirection to break recursive type)
    receivers: BoundedMap<ChannelId, Box<ChannelReceiver>, MAX_CHANNELS_PER_COMPONENT>,
    /// Channel quotas and limits
    channel_limits: ChannelLimits,
}

/// Channel limits per component
#[derive(Debug, Clone)]
pub struct ChannelLimits {
    max_channels: usize,
    max_total_capacity: usize,
    max_message_size: usize,
    fuel_budget: u64,
}

/// Channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfiguration {
    pub default_capacity: usize,
    pub max_unbounded_size: usize,
    pub enable_backpressure: bool,
    pub enable_priority_channels: bool,
    pub wake_coalescing: bool,
    pub fuel_tracking: bool,
}

impl Default for ChannelConfiguration {
    fn default() -> Self {
        Self {
            default_capacity: 32,
            max_unbounded_size: 1024,
            enable_backpressure: true,
            enable_priority_channels: true,
            wake_coalescing: true,
            fuel_tracking: true,
        }
    }
}

/// Channel statistics
#[derive(Debug, Default)]
struct ChannelStatistics {
    total_channels_created: AtomicU64,
    total_messages_sent: AtomicU64,
    total_messages_received: AtomicU64,
    total_channel_closes: AtomicU64,
    backpressure_events: AtomicU64,
    wake_coalescings: AtomicU64,
    total_fuel_consumed: AtomicU64,
}

impl OptimizedAsyncChannels {
    /// Create new optimized async channels manager
    pub fn new(
        bridge: Arc<Mutex<TaskManagerAsyncBridge>>,
        config: Option<ChannelConfiguration>,
    ) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;
        Ok(Self {
            bridge,
            channels: BoundedMap::new(),
            component_contexts: BoundedMap::new(),
            next_channel_id: AtomicU64::new(1),
            channel_stats: ChannelStatistics::default(),
            global_config: config.unwrap_or_default(),
        })
    }

    /// Initialize component for channel operations
    pub fn initialize_component_channels(
        &mut self,
        component_id: ComponentInstanceId,
        limits: Option<ChannelLimits>,
    ) -> Result<()> {
        let limits = limits.unwrap_or(ChannelLimits {
            max_channels: MAX_CHANNELS_PER_COMPONENT,
            max_total_capacity: MAX_CHANNEL_CAPACITY * 4,
            max_message_size: 1024 * 1024, // 1MB
            fuel_budget: 100_000,
        });

        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        let context = Box::new(ComponentChannelContext {
            component_id,
            owned_channels: BoundedVec::new(),
            senders: BoundedMap::new(),
            receivers: BoundedMap::new(),
            channel_limits: limits,
        });

        self.component_contexts
            .insert(component_id, context)
            .map_err(|_| Error::resource_limit_exceeded("Too many component channel contexts"))?;

        Ok(())
    }

    /// Create a new channel
    pub fn create_channel(
        &mut self,
        component_id: ComponentInstanceId,
        channel_type: ChannelType,
    ) -> Result<(ChannelSender, ChannelReceiver)> {
        // Extract limits before mutable borrow
        let max_channels = self
            .component_contexts
            .get(&component_id)
            .ok_or_else(|| {
                Error::validation_invalid_input("Component not initialized for channels")
            })?
            .channel_limits
            .max_channels;

        let owned_channels_len = self
            .component_contexts
            .get(&component_id)
            .ok_or_else(|| {
                Error::validation_invalid_input("Component not initialized for channels")
            })?
            .owned_channels
            .len();

        // Check limits
        if owned_channels_len >= max_channels {
            return Err(Error::resource_limit_exceeded(
                "Component channel limit exceeded",
            ));
        }

        let channel_id = ChannelId(self.next_channel_id.fetch_add(1, Ordering::AcqRel));
        let capacity = match channel_type {
            ChannelType::Bounded(cap) => cap,
            ChannelType::Oneshot => 1,
            ChannelType::Broadcast(cap) => cap,
            _ => self.global_config.default_capacity,
        };

        // Create channel buffer
        let buffer = self.create_channel_buffer(channel_type, capacity)?;

        let channel = AsyncChannel {
            id: channel_id,
            channel_type,
            capacity,
            buffer,
            sender_wakers: BoundedVec::new(),
            receiver_wakers: BoundedVec::new(),
            closed: AtomicBool::new(false),
            sender_count: AtomicU32::new(1),
            receiver_count: AtomicU32::new(1),
            total_sent: AtomicU64::new(0),
            total_received: AtomicU64::new(0),
            fuel_consumed: AtomicU64::new(CHANNEL_CREATE_FUEL),
            created_at: self.get_timestamp(),
        };

        // Store channel
        self.channels
            .insert(channel_id, channel)
            .map_err(|_| Error::resource_limit_exceeded("Too many active channels"))?;

        // Create sender and receiver with empty Weak references
        // Note: The caller must be holding an Arc<Mutex<Self>> and should set these references properly
        // For now, we create placeholders that won't upgrade
        let channels_weak = Weak::new();

        let sender = ChannelSender {
            channel_id,
            component_id,
            channels_ref: channels_weak.clone(),
        };

        let receiver = ChannelReceiver {
            channel_id,
            component_id,
            channels_ref: channels_weak,
        };

        // Add to component context
        let context = self
            .component_contexts
            .get_mut(&component_id)
            .ok_or_else(|| Error::validation_invalid_input("Component not found"))?;

        context
            .owned_channels
            .push(channel_id)
            .map_err(|_| Error::resource_limit_exceeded("Component channel list full"))?;

        context.senders.insert(channel_id, Box::new(sender.clone())).ok();
        context.receivers.insert(channel_id, Box::new(receiver.clone())).ok();

        // Update statistics
        self.channel_stats.total_channels_created.fetch_add(1, Ordering::Relaxed);

        Ok((sender, receiver))
    }

    /// Send message through channel
    pub fn send_message(
        &mut self,
        channel_id: ChannelId,
        sender_id: ComponentInstanceId,
        message: ComponentValue<ComponentProvider>,
        priority: Option<u8>,
    ) -> Result<SendResult> {
        // Check if channel is closed first
        let is_closed = self
            .channels
            .get(&channel_id)
            .ok_or_else(|| Error::validation_invalid_input("Channel not found"))?
            .closed
            .load(Ordering::Acquire);

        if is_closed {
            return Ok(SendResult::Closed);
        }

        let channel_message = ChannelMessage {
            value: message,
            sender_id,
            sent_at: self.get_timestamp(),
            priority: priority.unwrap_or(0),
        };

        // Try to send message
        let channel = self
            .channels
            .get_mut(&channel_id)
            .ok_or_else(|| Error::validation_invalid_input("Channel not found"))?;
        let send_result = match &mut channel.buffer {
            ChannelBuffer::Ring {
                data,
                head,
                tail,
                len,
            } => {
                let current_len = len.load(Ordering::Acquire);
                if current_len >= channel.capacity {
                    if self.global_config.enable_backpressure {
                        self.channel_stats.backpressure_events.fetch_add(1, Ordering::Relaxed);
                        SendResult::WouldBlock
                    } else {
                        SendResult::Full
                    }
                } else {
                    let tail_idx = tail.load(Ordering::Acquire);
                    data[tail_idx % channel.capacity] = channel_message;
                    tail.store((tail_idx + 1) % channel.capacity, Ordering::Release);
                    len.fetch_add(1, Ordering::AcqRel);
                    SendResult::Sent
                }
            },
            ChannelBuffer::Vector { data } => {
                if data.len() >= self.global_config.max_unbounded_size {
                    SendResult::Full
                } else {
                    data.push(channel_message)
                        .map_err(|_| Error::resource_limit_exceeded("Channel buffer full"))?;
                    SendResult::Sent
                }
            },
            ChannelBuffer::Single { data, taken } => {
                if data.is_some() {
                    SendResult::Full
                } else {
                    *data = Some(channel_message);
                    SendResult::Sent
                }
            },
            ChannelBuffer::Priority { data } => {
                let priority_msg = PriorityMessage {
                    message: channel_message,
                    priority: priority.unwrap_or(0),
                };
                data.push(priority_msg)
                    .map_err(|_| Error::resource_limit_exceeded("Priority channel full"))?;
                SendResult::Sent
            },
        };

        if send_result == SendResult::Sent {
            // Update statistics
            {
                let channel = self
                    .channels
                    .get_mut(&channel_id)
                    .ok_or_else(|| Error::validation_invalid_input("Channel not found"))?;
                channel.total_sent.fetch_add(1, Ordering::Relaxed);
                channel.fuel_consumed.fetch_add(CHANNEL_SEND_FUEL, Ordering::Relaxed);

                // Wake receivers while we have the channel borrow
                // Extract wakers to avoid holding mutable borrow during wake
                let mut wakers_to_wake = BoundedVec::<Waker, 32>::new();
                while let Some(waker) = channel.receiver_wakers.pop() {
                    let _ = wakers_to_wake.push(waker);
                }
                // Drop channel borrow before waking
                let _ = channel;

                // Now wake all wakers without holding any borrows
                for waker in wakers_to_wake {
                    waker.wake();
                }
            }
            self.channel_stats.total_messages_sent.fetch_add(1, Ordering::Relaxed);

            if self.global_config.wake_coalescing {
                self.channel_stats.wake_coalescings.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(send_result)
    }

    /// Receive message from channel
    pub fn receive_message(
        &mut self,
        channel_id: ChannelId,
        receiver_id: ComponentInstanceId,
    ) -> Result<ReceiveResult> {
        let channel = self
            .channels
            .get_mut(&channel_id)
            .ok_or_else(|| Error::validation_invalid_input("Channel not found"))?;

        // Try to receive message
        let receive_result = match &mut channel.buffer {
            ChannelBuffer::Ring {
                data,
                head,
                tail,
                len,
            } => {
                let current_len = len.load(Ordering::Acquire);
                if current_len == 0 {
                    if channel.closed.load(Ordering::Acquire) {
                        ReceiveResult::Closed
                    } else {
                        ReceiveResult::WouldBlock
                    }
                } else {
                    let head_idx = head.load(Ordering::Acquire);
                    let message = data[head_idx % channel.capacity].clone();
                    head.store((head_idx + 1) % channel.capacity, Ordering::Release);
                    len.fetch_sub(1, Ordering::AcqRel);
                    ReceiveResult::Received(message)
                }
            },
            ChannelBuffer::Vector { data } => {
                if data.is_empty() {
                    if channel.closed.load(Ordering::Acquire) {
                        ReceiveResult::Closed
                    } else {
                        ReceiveResult::WouldBlock
                    }
                } else {
                    let message = data.remove(0);
                    ReceiveResult::Received(message)
                }
            },
            ChannelBuffer::Single { data, taken } => {
                if let Some(message) = data.take() {
                    taken.store(true, Ordering::Release);
                    ReceiveResult::Received(message)
                } else if channel.closed.load(Ordering::Acquire) {
                    ReceiveResult::Closed
                } else {
                    ReceiveResult::WouldBlock
                }
            },
            ChannelBuffer::Priority { data } => {
                if data.is_empty() {
                    if channel.closed.load(Ordering::Acquire) {
                        ReceiveResult::Closed
                    } else {
                        ReceiveResult::WouldBlock
                    }
                } else {
                    // Get highest priority message
                    data.sort_by(|a, b| b.priority.cmp(&a.priority));
                    let priority_msg = data.remove(0);
                    ReceiveResult::Received(priority_msg.message)
                }
            },
        };

        if let ReceiveResult::Received(_) = receive_result {
            // Update statistics
            channel.total_received.fetch_add(1, Ordering::Relaxed);
            channel.fuel_consumed.fetch_add(CHANNEL_RECV_FUEL, Ordering::Relaxed);
            self.channel_stats.total_messages_received.fetch_add(1, Ordering::Relaxed);

            // Wake senders if backpressure was active
            if self.global_config.enable_backpressure {
                // Wake all sender wakers (drain not available on StaticVec, so manually iterate and clear)
                while let Some(waker) = channel.sender_wakers.pop() {
                    waker.wake();
                }
            }
        }

        Ok(receive_result)
    }

    /// Close a channel
    pub fn close_channel(&mut self, channel_id: ChannelId) -> Result<()> {
        let channel = self
            .channels
            .get_mut(&channel_id)
            .ok_or_else(|| Error::validation_invalid_input("Channel not found"))?;

        channel.closed.store(true, Ordering::Release);
        channel.fuel_consumed.fetch_add(CHANNEL_CLOSE_FUEL, Ordering::Relaxed);

        // Wake all waiters
        while let Some(waker) = channel.sender_wakers.pop() {
            waker.wake();
        }
        while let Some(waker) = channel.receiver_wakers.pop() {
            waker.wake();
        }

        // Update statistics
        self.channel_stats.total_channel_closes.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get channel statistics
    pub fn get_channel_statistics(&self) -> ChannelStats {
        ChannelStats {
            total_channels_created: self
                .channel_stats
                .total_channels_created
                .load(Ordering::Relaxed),
            total_messages_sent: self.channel_stats.total_messages_sent.load(Ordering::Relaxed),
            total_messages_received: self
                .channel_stats
                .total_messages_received
                .load(Ordering::Relaxed),
            total_channel_closes: self.channel_stats.total_channel_closes.load(Ordering::Relaxed),
            backpressure_events: self.channel_stats.backpressure_events.load(Ordering::Relaxed),
            wake_coalescings: self.channel_stats.wake_coalescings.load(Ordering::Relaxed),
            active_channels: self.channels.len() as u64,
            total_fuel_consumed: self.channel_stats.total_fuel_consumed.load(Ordering::Relaxed),
        }
    }

    // Private helper methods

    fn create_channel_buffer(
        &self,
        channel_type: ChannelType,
        capacity: usize,
    ) -> Result<ChannelBuffer> {
        // Use standard buffer size for channel allocation
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;

        match channel_type {
            ChannelType::Bounded(_) => Ok(ChannelBuffer::Ring {
                data: BoundedVec::new(),
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                len: AtomicUsize::new(0),
            }),
            ChannelType::Unbounded => Ok(ChannelBuffer::Vector {
                data: BoundedVec::new(),
            }),
            ChannelType::Oneshot => Ok(ChannelBuffer::Single {
                data: None,
                taken: AtomicBool::new(false),
            }),
            ChannelType::Broadcast(_) => Ok(ChannelBuffer::Vector {
                data: BoundedVec::new(),
            }),
            ChannelType::Priority => Ok(ChannelBuffer::Priority {
                data: BoundedVec::new(),
            }),
        }
    }

    fn wake_receivers(&mut self, channel: &mut AsyncChannel) -> Result<()> {
        // Wake all receiver wakers (drain not available on StaticVec, so manually iterate and clear)
        while let Some(waker) = channel.receiver_wakers.pop() {
            waker.wake();
        }

        if self.global_config.wake_coalescing {
            self.channel_stats.wake_coalescings.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    fn wake_senders(&mut self, channel: &mut AsyncChannel) -> Result<()> {
        // Wake all sender wakers (drain not available on StaticVec, so manually iterate and clear)
        while let Some(waker) = channel.sender_wakers.pop() {
            waker.wake();
        }

        Ok(())
    }

    fn wake_all_waiters(&mut self, channel: &mut AsyncChannel) -> Result<()> {
        self.wake_senders(channel)?;
        self.wake_receivers(channel)?;
        Ok(())
    }

    fn get_timestamp(&self) -> u64 {
        // In real implementation, would use proper time source
        0
    }
}

/// Result of send operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendResult {
    /// Message sent successfully
    Sent,
    /// Channel is full, would block
    WouldBlock,
    /// Channel is full and cannot accept more
    Full,
    /// Channel is closed
    Closed,
}

impl Default for SendResult {
    fn default() -> Self {
        Self::Sent
    }
}

impl Checksummable for SendResult {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            SendResult::Sent => 0u8,
            SendResult::WouldBlock => 1u8,
            SendResult::Full => 2u8,
            SendResult::Closed => 3u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for SendResult {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            SendResult::Sent => 0u8,
            SendResult::WouldBlock => 1u8,
            SendResult::Full => 2u8,
            SendResult::Closed => 3u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for SendResult {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(SendResult::Sent),
            1 => Ok(SendResult::WouldBlock),
            2 => Ok(SendResult::Full),
            3 => Ok(SendResult::Closed),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid SendResult discriminant",
            )),
        }
    }
}

/// Result of receive operation
#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveResult {
    /// Message received successfully
    Received(ChannelMessage),
    /// No message available, would block
    WouldBlock,
    /// Channel is closed
    Closed,
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub total_channels_created: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_channel_closes: u64,
    pub backpressure_events: u64,
    pub wake_coalescings: u64,
    pub active_channels: u64,
    pub total_fuel_consumed: u64,
}

/// Send future for async channel operations
pub struct SendFuture {
    sender: ChannelSender,
    message: Option<ComponentValue<ComponentProvider>>,
    priority: Option<u8>,
}

impl CoreFuture for SendFuture {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(channels) = self.sender.channels_ref.upgrade() {
            let mut channels = channels.lock();
            if let Some(message) = self.message.take() {
                match channels.send_message(
                    self.sender.channel_id,
                    self.sender.component_id,
                    message,
                    self.priority,
                ) {
                    Ok(SendResult::Sent) => Poll::Ready(Ok(())),
                    Ok(SendResult::WouldBlock) => {
                        // Register waker for when space becomes available
                        if let Some(channel) = channels.channels.get_mut(&self.sender.channel_id) {
                            channel.sender_wakers.push(cx.waker().clone()).ok();
                        }
                        Poll::Pending
                    },
                    Ok(SendResult::Closed) => {
                        Poll::Ready(Err(Error::invalid_state_error("Channel closed")))
                    },
                    Ok(SendResult::Full) => {
                        Poll::Ready(Err(Error::resource_limit_exceeded("Channel full")))
                    },
                    Err(e) => Poll::Ready(Err(e)),
                }
            } else {
                Poll::Ready(Err(Error::invalid_state_error("Message already sent")))
            }
        } else {
            Poll::Ready(Err(Error::invalid_state_error("Channel manager dropped")))
        }
    }
}

/// Receive future for async channel operations
pub struct ReceiveFuture {
    receiver: ChannelReceiver,
}

impl CoreFuture for ReceiveFuture {
    type Output = Result<ComponentValue<ComponentProvider>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(channels) = self.receiver.channels_ref.upgrade() {
            let mut channels = channels.lock();
            match channels.receive_message(self.receiver.channel_id, self.receiver.component_id) {
                Ok(ReceiveResult::Received(message)) => Poll::Ready(Ok(message.value)),
                Ok(ReceiveResult::WouldBlock) => {
                    // Register waker for when message becomes available
                    if let Some(channel) = channels.channels.get_mut(&self.receiver.channel_id) {
                        channel.receiver_wakers.push(cx.waker().clone()).ok();
                    }
                    Poll::Pending
                },
                Ok(ReceiveResult::Closed) => {
                    Poll::Ready(Err(Error::invalid_state_error("Channel closed")))
                },
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            Poll::Ready(Err(Error::invalid_state_error("Channel manager dropped")))
        }
    }
}

impl ChannelSender {
    /// Send a message asynchronously
    pub fn send(&self, message: ComponentValue<ComponentProvider>) -> SendFuture {
        SendFuture {
            sender: self.clone(),
            message: Some(message),
            priority: None,
        }
    }

    /// Send a message with priority
    pub fn send_with_priority(
        &self,
        message: ComponentValue<ComponentProvider>,
        priority: u8,
    ) -> SendFuture {
        SendFuture {
            sender: self.clone(),
            message: Some(message),
            priority: Some(priority),
        }
    }
}

impl ChannelReceiver {
    /// Receive a message asynchronously
    pub fn receive(&self) -> ReceiveFuture {
        ReceiveFuture {
            receiver: self.clone(),
        }
    }
}
