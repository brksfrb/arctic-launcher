//! Bound a download's idle socket reads independently of its total body timeout.

use std::cell::Cell;
use std::time::Duration;

use ureq::unversioned::transport::{Buffers, ConnectionDetails, Connector, NextTimeout, Transport};

thread_local! {
    static BODY_IDLE: Cell<Option<Duration>> = const { Cell::new(None) };
}

/// The synchronous request and body reader run on the same thread. Keeping the
/// limit on the read lets pooled connections use the policy of their current request.
pub(super) fn with_body_idle<T>(limit: Duration, read: impl FnOnce() -> T) -> T {
    BODY_IDLE.with(|idle| {
        struct Restore<'a>(&'a Cell<Option<Duration>>, Option<Duration>);

        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }

        let _restore = Restore(idle, idle.replace(Some(limit)));
        read()
    })
}

#[derive(Debug)]
pub(super) struct DownloadIdleConnector;

impl Connector<Box<dyn Transport>> for DownloadIdleConnector {
    type Out = DownloadIdleTransport;

    fn connect(
        &self,
        _details: &ConnectionDetails,
        chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(chained.map(DownloadIdleTransport))
    }
}

#[derive(Debug)]
pub(super) struct DownloadIdleTransport(Box<dyn Transport>);

impl Transport for DownloadIdleTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.0.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.0.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, mut timeout: NextTimeout) -> Result<bool, ureq::Error> {
        if timeout.reason == ureq::Timeout::RecvBody
            && let Some(idle) = BODY_IDLE.get()
        {
            timeout.after = timeout.after.min(idle.into());
        }
        self.0.await_input(timeout)
    }

    fn is_open(&mut self) -> bool {
        self.0.is_open()
    }

    fn is_tls(&self) -> bool {
        self.0.is_tls()
    }
}
