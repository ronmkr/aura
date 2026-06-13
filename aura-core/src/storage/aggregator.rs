//! Sequential Write Aggregator to prevent random I/O fragmentation,
//! following the design intent of [ADR-0033](aura-docs/adr/0033-generation-writes-and-aggregation.md).

use crate::TaskId;
use bytes::BytesMut;
use std::collections::{BTreeMap, HashMap};

pub struct ContiguousBlock {
    pub offset: u64,
    pub data: Vec<BytesMut>,
}

/// Aggregates and sequences disk write requests to minimize random disk I/O,
/// implementing the strategy defined in [ADR-0033](aura-docs/adr/0033-generation-writes-and-aggregation.md).
pub struct SequentialAggregator {
    pending_writes: HashMap<TaskId, BTreeMap<u64, BytesMut>>,
    dirty_buffers: HashMap<TaskId, Vec<(u64, BytesMut)>>,
    dirty_sizes: HashMap<TaskId, usize>,
    next_offsets: HashMap<TaskId, u64>,
}

impl Default for SequentialAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialAggregator {
    pub fn new() -> Self {
        Self {
            pending_writes: HashMap::new(),
            dirty_buffers: HashMap::new(),
            dirty_sizes: HashMap::new(),
            next_offsets: HashMap::new(),
        }
    }

    pub fn register_task(&mut self, id: TaskId) {
        self.next_offsets.entry(id).or_insert(0);
        self.pending_writes.entry(id).or_default();
        self.dirty_buffers.entry(id).or_default();
        self.dirty_sizes.entry(id).or_insert(0);
    }

    pub fn unregister_task(&mut self, id: &TaskId) {
        self.next_offsets.remove(id);
        self.pending_writes.remove(id);
        self.dirty_buffers.remove(id);
        self.dirty_sizes.remove(id);
    }

    pub fn add_write(
        &mut self,
        id: TaskId,
        offset: u64,
        data: BytesMut,
        padding_ranges: &[crate::task::Range],
        threshold: usize,
    ) -> Vec<ContiguousBlock> {
        let next_offset = *self.next_offsets.get(&id).unwrap_or(&0);
        let mut ready_blocks = Vec::new();

        if offset == next_offset {
            let len = data.len() as u64;

            // Filter padding before pushing to dirty buffer
            let subranges =
                super::engine::get_non_padding_subranges_impl(padding_ranges, offset, len);
            for sub in subranges {
                let sub_offset = sub.start;
                let sub_len = sub.length();

                // Check for discontinuity in dirty buffer (caused by padding skip)
                let mut needs_flush = false;
                if let Some(dirty) = self.dirty_buffers.get(&id) {
                    if let Some(last) = dirty.last() {
                        if sub_offset != (last.0 + last.1.len() as u64) {
                            needs_flush = true;
                        }
                    }
                }

                if needs_flush {
                    if let Some(block) = self.take_dirty_block(id) {
                        ready_blocks.push(block);
                    }
                }

                if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
                    let start_in_data = (sub_offset - offset) as usize;
                    let end_in_data = start_in_data + sub_len as usize;
                    let sub_data = BytesMut::from(&data[start_in_data..end_in_data]);

                    dirty.push((sub_offset, sub_data));
                    if let Some(size) = self.dirty_sizes.get_mut(&id) {
                        *size += sub_len as usize;
                    }
                }
            }

            let mut current_offset = next_offset + len;

            let mut to_flush = Vec::new();
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                while let Some(p_data) = pending.remove(&current_offset) {
                    let p_len = p_data.len() as u64;

                    // Filter padding for pending writes as well
                    let p_subranges = super::engine::get_non_padding_subranges_impl(
                        padding_ranges,
                        current_offset,
                        p_len,
                    );
                    for sub in p_subranges {
                        let start_in_p = (sub.start - current_offset) as usize;
                        let end_in_p = start_in_p + sub.length() as usize;
                        to_flush.push((sub.start, BytesMut::from(&p_data[start_in_p..end_in_p])));
                    }

                    current_offset += p_len;
                }
            }

            for (off, p_data) in to_flush {
                let p_len = p_data.len();

                // Check for discontinuity again
                let mut needs_flush = false;
                if let Some(dirty) = self.dirty_buffers.get(&id) {
                    if let Some(last) = dirty.last() {
                        if off != (last.0 + last.1.len() as u64) {
                            needs_flush = true;
                        }
                    }
                }

                if needs_flush {
                    if let Some(block) = self.take_dirty_block(id) {
                        ready_blocks.push(block);
                    }
                }

                if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
                    dirty.push((off, p_data));
                    if let Some(size) = self.dirty_sizes.get_mut(&id) {
                        *size += p_len;
                    }
                }
            }

            self.next_offsets.insert(id, current_offset);

            if let Some(&size) = self.dirty_sizes.get(&id) {
                if size >= threshold {
                    if let Some(block) = self.take_dirty_block(id) {
                        ready_blocks.push(block);
                    }
                }
            }
        } else {
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                pending.insert(offset, data);
            }
        }

        ready_blocks
    }

    pub fn take_dirty_block(&mut self, id: TaskId) -> Option<ContiguousBlock> {
        let buffers = self.dirty_buffers.get_mut(&id).map(std::mem::take)?;
        if buffers.is_empty() {
            return None;
        }

        let offset = buffers.first().unwrap().0;
        let data = buffers.into_iter().map(|(_, d)| d).collect::<Vec<_>>();

        if let Some(size) = self.dirty_sizes.get_mut(&id) {
            *size = 0;
        }

        Some(ContiguousBlock { offset, data })
    }

    pub fn take_all_pending(&mut self, id: TaskId) -> Vec<ContiguousBlock> {
        let mut blocks = Vec::new();
        if let Some(block) = self.take_dirty_block(id) {
            blocks.push(block);
        }

        if let Some(pending) = self.pending_writes.remove(&id) {
            for (offset, data) in pending {
                blocks.push(ContiguousBlock {
                    offset,
                    data: vec![data],
                });
            }
        }
        blocks
    }

    pub fn get_dirty_task_ids(&self) -> Vec<TaskId> {
        self.dirty_sizes
            .iter()
            .filter_map(|(&id, &size)| if size > 0 { Some(id) } else { None })
            .collect()
    }
}
