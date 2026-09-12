use crate::{commands::WorldCommand, world::storage::World};
use orx_concurrent_bag::ConcurrentBag;
use std::mem::{self, ManuallyDrop, MaybeUninit};

struct CommandMeta {
    consume_and_advance: unsafe fn(
        chunks_iter: &mut dyn Iterator<Item = &mut BufferChunk>,
        world: Option<&mut World>,
    ),
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub(crate) struct BufferChunk {
    pub(crate) bytes: [MaybeUninit<u8>; 16],
}

pub(crate) struct CommandQueue {
    chunks: ConcurrentBag<BufferChunk>,
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self {
            chunks: ConcurrentBag::new(),
        }
    }
}

impl CommandQueue {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push<C: WorldCommand>(&self, command: C) {
        let meta = CommandMeta {
            consume_and_advance: |chunks_iter, world| unsafe {
                let mut local_buffer = MaybeUninit::<C>::uninit();
                let dst_ptr = local_buffer.as_mut_ptr().cast::<u8>();
                let mut bytes_written = 0;
                let total_bytes_to_read = mem::size_of::<C>();
                let payload_chunk_count = total_bytes_to_read.div_ceil(16);

                for _ in 0..payload_chunk_count {
                    if let Some(chunk) = chunks_iter.next() {
                        let bytes_left = total_bytes_to_read - bytes_written;
                        let bytes_to_copy = bytes_left.min(16);

                        std::ptr::copy_nonoverlapping(
                            chunk.bytes.as_ptr().cast::<u8>(),
                            dst_ptr.add(bytes_written),
                            bytes_to_copy,
                        );
                        bytes_written += bytes_to_copy;
                    }
                }

                let command: C = std::ptr::read(local_buffer.as_ptr());
                match world {
                    Some(w) => command.apply(w),
                    None => mem::drop(command),
                }
            },
        };

        let stream_iter = StackCommandIterator::new(meta, command);
        self.chunks.extend(stream_iter);
    }

    pub(crate) fn push_fn<F>(&self, f: F)
    where
        F: FnOnce(&mut World) + Send + 'static,
    {
        self.push(FunctionCommand { func: f });
    }

    pub(crate) fn apply(&mut self, world: &mut World) {
        if self.chunks.is_empty() {
            return;
        }

        {
            let mut chunks_iter = self.chunks.iter_mut();
            let mut dyn_iter = &mut chunks_iter as &mut dyn Iterator<Item = &mut BufferChunk>;

            while let Some(meta_chunk) = dyn_iter.next() {
                unsafe {
                    let meta: CommandMeta = std::ptr::read(meta_chunk.bytes.as_ptr().cast());
                    (meta.consume_and_advance)(&mut dyn_iter, Some(world));
                }
            }
        }
        self.chunks.clear();
    }
}

impl Drop for CommandQueue {
    fn drop(&mut self) {
        if self.chunks.is_empty() {
            return;
        }

        {
            let mut chunks_iter = self.chunks.iter_mut();
            let mut dyn_iter = &mut chunks_iter as &mut dyn Iterator<Item = &mut BufferChunk>;

            while let Some(meta_chunk) = dyn_iter.next() {
                unsafe {
                    let meta: CommandMeta = std::ptr::read(meta_chunk.bytes.as_ptr().cast());
                    (meta.consume_and_advance)(&mut dyn_iter, None);
                }
            }
        }
        self.chunks.clear();
    }
}

struct StackCommandIterator<C: WorldCommand> {
    meta: ManuallyDrop<CommandMeta>,
    command: ManuallyDrop<C>,
    current_chunk_idx: usize,
    total_chunks: usize,
}

impl<C: WorldCommand> StackCommandIterator<C> {
    fn new(meta: CommandMeta, command: C) -> Self {
        let payload_size = mem::size_of::<C>();
        let total_chunks = 1 + payload_size.div_ceil(16);
        Self {
            meta: ManuallyDrop::new(meta),
            command: ManuallyDrop::new(command),
            current_chunk_idx: 0,
            total_chunks,
        }
    }
}

impl<C: WorldCommand> Iterator for StackCommandIterator<C> {
    type Item = BufferChunk;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_chunk_idx >= self.total_chunks {
            return None;
        }

        let mut chunk = BufferChunk {
            bytes: [MaybeUninit::uninit(); 16],
        };

        unsafe {
            let chunk_ptr = chunk.bytes.as_mut_ptr().cast::<u8>();
            std::ptr::write_bytes(chunk_ptr, 0, 16);

            if self.current_chunk_idx == 0 {
                std::ptr::write(
                    chunk_ptr.cast::<unsafe fn(&mut dyn Iterator<Item = &mut BufferChunk>, Option<&mut World>)>(),
                    self.meta.consume_and_advance,
                );
            } else {
                let command_start_ptr = &*self.command as *const C as *const u8;
                let payload_offset = (self.current_chunk_idx - 1) * 16;
                let bytes_left = mem::size_of::<C>().saturating_sub(payload_offset);
                let bytes_to_copy = bytes_left.min(16);

                if bytes_to_copy > 0 {
                    std::ptr::copy_nonoverlapping(
                        command_start_ptr.add(payload_offset),
                        chunk_ptr,
                        bytes_to_copy,
                    );
                }
            }
        }

        self.current_chunk_idx += 1;
        Some(chunk)
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.total_chunks - self.current_chunk_idx;
        (remaining, Some(remaining))
    }
}

impl<C: WorldCommand> ExactSizeIterator for StackCommandIterator<C> {}

struct FunctionCommand<F>
where
    F: FnOnce(&mut World) + Send + 'static,
{
    func: F,
}

impl<F: FnOnce(&mut World) + Send + 'static> WorldCommand for FunctionCommand<F> {
    fn apply(self, world: &mut World) {
        (self.func)(world)
    }
}
