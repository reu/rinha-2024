use std::collections::{vec_deque, VecDeque};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingBuffer<T, const SIZE: usize>(VecDeque<T>);

impl<T> Default for RingBuffer<T, 10> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, T> RingBuffer<T, SIZE> {
    pub fn new() -> Self {
        Self(VecDeque::with_capacity(SIZE))
    }

    pub fn push_front(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_back();
            self.0.push_front(item);
        } else {
            self.0.push_front(item);
        }
    }

    pub fn push_back(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_front();
            self.0.push_back(item);
        } else {
            self.0.push_back(item);
        }
    }
}

impl<const SIZE: usize, T> IntoIterator for RingBuffer<T, SIZE> {
    type Item = T;
    type IntoIter = vec_deque::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<const SIZE: usize, A> FromIterator<A> for RingBuffer<A, SIZE> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let mut ring_buffer = Self::new();
        for item in iter.into_iter() {
            ring_buffer.push_back(item);
        }
        ring_buffer
    }
}
