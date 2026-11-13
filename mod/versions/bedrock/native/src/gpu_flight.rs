//! Bounded ownership of submitted GPU work, shared by avatar uploads and draws.
use hudhook::windows::{Win32::Graphics::Direct3D12::*, core::Interface};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

struct Pending<T> {
    value: T,
    _allocator: ID3D12CommandAllocator,
    _list: ID3D12GraphicsCommandList,
    _queue: ID3D12CommandQueue,
    device: ID3D12Device,
    fence: ID3D12Fence,
    signaled: bool,
}
enum Completion {
    Pending,
    Ready,
    Removed,
}
impl<T> Pending<T> {
    fn completion(&self) -> Completion {
        unsafe {
            if self.device.GetDeviceRemovedReason().is_err() {
                return Completion::Removed;
            }
            // The private fence is signaled once. UINT64_MAX is removal, not retirement.
            if self.signaled && self.fence.GetCompletedValue() == 1 {
                Completion::Ready
            } else {
                Completion::Pending
            }
        }
    }
}
struct Slot<T> {
    claimed: AtomicBool,
    abandoned: Mutex<Option<Pending<T>>>,
}
impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            claimed: AtomicBool::new(false),
            abandoned: Mutex::new(None),
        }
    }
}
pub(crate) struct Pool<T, const N: usize> {
    slots: [Slot<T>; N],
}
impl<T: Send + 'static, const N: usize> Pool<T, N> {
    pub(crate) const fn new() -> Self {
        Self {
            slots: [const { Slot::new() }; N],
        }
    }
    pub(crate) fn collect(&self) {
        for slot in &self.slots {
            let retired = {
                let mut abandoned = slot
                    .abandoned
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if abandoned
                    .as_ref()
                    .is_some_and(|pending| !matches!(pending.completion(), Completion::Pending))
                {
                    abandoned.take()
                } else {
                    None
                }
            };
            if let Some(retired) = retired {
                // Native owner destructors can reenter adapter code. Never run
                // their cleanup while holding the retirement-slot mutex.
                drop(retired);
                slot.claimed.store(false, Ordering::Release);
            }
        }
    }
    pub(crate) fn claim(&'static self) -> Result<Permit<T, N>, String> {
        self.collect();
        for (index, slot) in self.slots.iter().enumerate() {
            if slot
                .claimed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(Permit {
                    pool: self,
                    index,
                    armed: true,
                });
            }
        }
        Err("GPU submission slots are occupied or awaiting retirement".into())
    }
}
pub(crate) struct Permit<T: 'static, const N: usize> {
    pool: &'static Pool<T, N>,
    index: usize,
    armed: bool,
}
impl<T: 'static, const N: usize> Drop for Permit<T, N> {
    fn drop(&mut self) {
        if self.armed {
            self.pool.slots[self.index]
                .claimed
                .store(false, Ordering::Release);
        }
    }
}
impl<T: Send + 'static, const N: usize> Permit<T, N> {
    /// Caller has closed this exclusively owned list; value retains every
    /// resource/descriptor/native owner referenced by its recorded commands.
    pub(crate) fn submit(
        mut self,
        device: &ID3D12Device,
        queue: &ID3D12CommandQueue,
        allocator: ID3D12CommandAllocator,
        list: ID3D12GraphicsCommandList,
        value: T,
    ) -> Result<Ticket<T, N>, String> {
        let _private = crate::depth_tracker::private_commands();
        let mut queue_device: Option<ID3D12Device> = None;
        let mut list_device: Option<ID3D12Device> = None;
        let mut allocator_device: Option<ID3D12Device> = None;
        unsafe {
            queue
                .GetDevice(&mut queue_device)
                .map_err(|error| error.to_string())?;
            list.GetDevice(&mut list_device)
                .map_err(|error| error.to_string())?;
            allocator
                .GetDevice(&mut allocator_device)
                .map_err(|error| error.to_string())?;
            if queue_device.as_ref() != Some(device)
                || list_device.as_ref() != Some(device)
                || allocator_device.as_ref() != Some(device)
                || queue.GetDesc().Type != D3D12_COMMAND_LIST_TYPE_DIRECT
                || list.GetType() != D3D12_COMMAND_LIST_TYPE_DIRECT
            {
                return Err("GPU work requires an owned matching direct device/list/queue".into());
            }
        }
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }
            .map_err(|error| error.to_string())?;
        let command: ID3D12CommandList = list.cast().map_err(|error| error.to_string())?;
        // From before Execute onwards, every return/unwind runs Ticket::drop.
        let mut ticket = Ticket {
            pool: self.pool,
            index: self.index,
            pending: Some(Pending {
                value,
                _allocator: allocator,
                _list: list,
                _queue: queue.clone(),
                device: device.clone(),
                fence,
                signaled: false,
            }),
        };
        self.armed = false;
        unsafe {
            queue.ExecuteCommandLists(&[Some(command)]);
        }
        let pending = ticket.pending.as_mut().expect("prepared GPU submission");
        match unsafe { queue.Signal(&pending.fence, 1) } {
            Ok(()) => pending.signaled = true,
            Err(error) => {
                return Err(format!(
                    "GPU work submitted but completion signal failed: {error}"
                ));
            }
        }
        Ok(ticket)
    }
}
pub(crate) struct Ticket<T: 'static, const N: usize> {
    pool: &'static Pool<T, N>,
    index: usize,
    pending: Option<Pending<T>>,
}
impl<T: Send + 'static, const N: usize> Ticket<T, N> {
    pub(crate) fn take_completed(&mut self) -> Result<Option<T>, String> {
        match self
            .pending
            .as_ref()
            .ok_or("GPU ticket already retired")?
            .completion()
        {
            Completion::Pending => Ok(None),
            Completion::Ready => {
                let pending = self.pending.take().expect("pending GPU submission");
                self.pool.slots[self.index]
                    .claimed
                    .store(false, Ordering::Release);
                Ok(Some(pending.value))
            }
            Completion::Removed => {
                self.pending.take();
                self.pool.slots[self.index]
                    .claimed
                    .store(false, Ordering::Release);
                Err("GPU submission device was removed".into())
            }
        }
    }
}
impl<T: 'static, const N: usize> Drop for Ticket<T, N> {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            let slot = &self.pool.slots[self.index];
            match pending.completion() {
                Completion::Ready | Completion::Removed => {
                    drop(pending);
                    slot.claimed.store(false, Ordering::Release);
                }
                Completion::Pending => {
                    // There is exactly one live ticket per claimed slot.
                    // No timeout, failed signal or missing caller frees GPU owners.
                    *slot
                        .abandoned
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = Some(pending);
                }
            }
        }
    }
}
