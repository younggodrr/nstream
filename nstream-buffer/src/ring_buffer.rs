use std::collections::VecDeque;
use tracing::debug;

/// Fixed-capacity ring buffer backed by `VecDeque`.
/// Capacity is set to hold 5 minutes of video at 30 fps: 5 * 60 * 30 = 9000 frames.
pub const DEFAULT_RING_CAPACITY: usize = 5 * 60 * 30; // 9000 frames

/// In-memory ring buffer with automatic eviction of the oldest element when full.
pub struct RingBuffer<T> {
    inner: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Create a ring buffer with the default 9000-frame capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_RING_CAPACITY)
    }

    /// Create a ring buffer with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push an item into the buffer.
    /// If the buffer is at capacity, the oldest item is evicted first.
    pub fn push(&mut self, item: T) {
        if self.inner.len() >= self.capacity {
            self.inner.pop_front();
            debug!(
                capacity = self.capacity,
                "Ring buffer full — evicted oldest item"
            );
        }
        self.inner.push_back(item);
    }

    /// Remove and return the oldest item, or `None` if empty.
    pub fn pop_front(&mut self) -> Option<T> {
        self.inner.pop_front()
    }

    /// Peek at the oldest item without removing it.
    pub fn front(&self) -> Option<&T> {
        self.inner.front()
    }

    /// Returns `true` if the buffer contains no items.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of items currently in the buffer.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns `true` if the buffer is at full capacity.
    pub fn is_full(&self) -> bool {
        self.inner.len() >= self.capacity
    }

    /// Drain all items from the buffer into a `Vec`.
    pub fn drain_all(&mut self) -> Vec<T> {
        self.inner.drain(..).collect()
    }
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::new()
    }
}
