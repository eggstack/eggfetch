//! Caller-owned fixed-target datagram routing for experimental HTTP/3.
//!
//! This module carries the whole public contract for the first experimental
//! routing seam plus the crate-private Quinn bridge that implements it.
//! Nothing here is reachable unless the `http3` feature is enabled, and the
//! seam only exists together with `advanced-routing`, which `http3` always
//! enables.
//!
//! The shape mirrors [`Dialer`](super::dialer::Dialer) on purpose: a
//! client-scoped dialer resolves a [`DialTarget`] to a transport, and the
//! transport here is a *fixed-target datagram route* rather than a TCP stream.
//! Reusing [`DialError`] / [`DialErrorKind`] keeps caller evidence typed end to
//! end instead of degrading it into unclassified text.
//!
//! # Ownership and lifetime
//!
//! A [`DatagramDialer`] is owned by the client for the client's lifetime. Each
//! successful [`DatagramDialer::connect`] call returns one [`DatagramRoute`],
//! and that route is owned by exactly one HTTP/3 connection generation. The
//! route, the abstract QUIC endpoint built over it, and the two bridge worker
//! tasks are released together when the generation is evicted, reaches a
//! terminal state, or is dropped — never before, and never in a detached leak.
//!
//! # Deliberate exclusions
//!
//! The contract is deliberately narrow and must stay narrow. It carries no
//! opening handshake, no certificate bytes, no QUIC frames, no retry policy,
//! and no second engine. Eggfetch still owns the whole QUIC/TLS stack: the
//! bridge below builds the QUIC client configuration, so a caller supplies only
//! a datagram carrier and can never take over authentication. Concrete carriers
//! (a MASQUE CONNECT-UDP datagram stream, a process-local UDP socket, an
//! inter-process channel) are follow-on work and are out of scope here.
//!
//! # Cancellation contract
//!
//! [`DatagramRoute::send`] and [`DatagramRoute::recv`] return boxed futures that
//! Eggfetch holds only while they are in flight, so a provider must be
//! cancellation-safe under ordinary future drop: dropping a `send` future may
//! lose that datagram but must not corrupt the route or poison later sends;
//! dropping a `recv` future must leave the route usable for another `recv`.
//! Eggfetch drives `recv` one datagram at a time, so a provider needs no
//! buffering of its own.

use std::fmt;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::task::{Context, Poll, Waker};

use bytes::Bytes;
use quinn::udp::{RecvMeta, Transmit};

use super::dialer::{DialError, DialErrorKind, DialTarget};

/// Boxed future returned by [`DatagramDialer::connect`].
pub type DatagramDialFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Arc<dyn DatagramRoute>, DialError>> + Send + 'a>>;

/// Boxed future for sending one datagram over an established route.
pub type DatagramSendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), DialError>> + Send + 'a>>;

/// Boxed future for receiving one datagram over an established route.
pub type DatagramRecvFuture<'a> =
    Pin<Box<dyn Future<Output = Result<usize, DialError>> + Send + 'a>>;

/// One established datagram route bound to a single fixed remote peer.
///
/// The peer is fixed by construction: Eggfetch asks the dialer for one route and
/// then never changes its destination, so an implementation cannot silently
/// redirect a QUIC connection elsewhere. Implementations must also reject
/// datagrams whose source is not the peer reported by
/// [`DatagramRoute::peer_addr`] — a route that forwarded unverified bytes would
/// let an off-path host inject packets into the QUIC connection.
///
/// `Debug` output must not expose implementation details or configuration that
/// could carry credentials. Eggfetch's own bridge never formats a route, so a
/// provider's `Debug` implementation is never rendered by the HTTP/3 stack.
pub trait DatagramRoute: Send + Sync + 'static {
    /// Sends one datagram to [`DatagramRoute::peer_addr`].
    ///
    /// The payload is borrowed only for the lifetime of the returned future.
    fn send<'a>(&'a self, payload: &'a [u8]) -> DatagramSendFuture<'a>;

    /// Receives one datagram into `buffer`, returning its length.
    ///
    /// A datagram larger than `buffer` may be truncated. QUIC treats a
    /// truncated datagram exactly as it treats a corrupted one, so the packet
    /// fails its AEAD check, is discarded, and the peer retransmits. `Ok(n)`
    /// with `n <= buffer.len()` is therefore the only success signal, and
    /// bytes beyond `n` must never be read.
    fn recv<'a>(&'a self, buffer: &'a mut [u8]) -> DatagramRecvFuture<'a>;

    /// Returns the local address of this route.
    fn local_addr(&self) -> SocketAddr;

    /// Returns the single remote peer address this route is bound to.
    fn peer_addr(&self) -> SocketAddr;
}

/// Client-scoped dialer that establishes [`DatagramRoute`]s for QUIC.
///
/// A dialer receives the same [`DialTarget`] a [`Dialer`](super::dialer::Dialer)
/// would receive, which carries the original logical host and port rather than
/// any physical path. Turning that target into a physical carrier — including
/// choosing a relay — is the provider's responsibility.
///
/// Implementations must not perform a DNS fallback of their own unless the
/// caller asked for it, must not retry, and must not cache a total deadline.
/// Eggfetch applies the request's own remaining connect budget to the returned
/// future, so a provider that blocks forever is bounded by the caller rather
/// than becoming a hang.
pub trait DatagramDialer: Send + Sync + 'static {
    /// Establishes one fixed-target datagram route for `target`.
    ///
    /// Returning a typed [`DialError`] is the only way to report failure, so
    /// the HTTP/3 stack can classify the attempt without parsing `Display`
    /// text. [`DialErrorKind::Timeout`] must only be returned when the
    /// provider itself observed a timeout; a caller-side deadline abort is
    /// reported by Eggfetch instead.
    fn connect(&self, target: DialTarget) -> DatagramDialFuture<'_>;
}

impl<T> DatagramDialer for Arc<T>
where
    T: DatagramDialer + ?Sized,
{
    fn connect(&self, target: DialTarget) -> DatagramDialFuture<'_> {
        (**self).connect(target)
    }
}

// ---------------------------------------------------------------------------
// Crate-private Quinn bridge
// ---------------------------------------------------------------------------

/// QUIC datagram payload ceiling for every caller-routed generation.
///
/// A route-owned endpoint advertises this as Quinn's `max_udp_payload_size`, so
/// the stack never emits a datagram the 1200-byte IPv6 minimum cannot carry and
/// never attempts path-MTU discovery over a carrier whose real MTU Eggfetch
/// cannot measure.
pub(crate) const ROUTE_MTU: usize = 1200;

/// Compile-time proof that the bridge can always carry a QUIC Initial packet.
///
/// A route whose carrier cannot carry 1200 bytes fails its first handshake
/// transmit; the bridge converts that into typed route evidence, so the
/// generation is refused during establishment — before any H3 stream is opened
/// and before any direct UDP socket could be created. There is no direct socket
/// to fall back to, so this cannot become a silent downgrade.
const _: () = assert!(ROUTE_MTU >= 1200);

/// Bounded datagram queue depth in each direction.
///
/// Both queues are hard-bounded, so memory per generation stays constant no
/// matter how much the peer sends. Saturated sends surface to Quinn as
/// [`io::ErrorKind::WouldBlock`]; saturated receives pause the bridge's receive
/// loop until capacity exists, so a slow carrier back-pressures QUIC instead of
/// silently dropping authenticated packets.
const ROUTE_QUEUE_DEPTH: usize = 64;

/// Hard cap on registered poller wakers per direction.
///
/// Quinn registers at most one poller per connection and a route-owned endpoint
/// serves exactly one connection generation, so this bound is unreachable in
/// practice. It exists so a misbehaving poller cannot grow the waker set
/// without limit.
const MAX_ROUTE_WAKERS: usize = 8;

/// QUIC transport error code used when a route's I/O fails.
///
/// Eggfetch closes the QUIC connection itself so a generation reaches a terminal
/// state promptly; a dead route must not leave a request hanging until an idle
/// timeout.
pub(crate) const ROUTE_IO_ERROR_CODE: u32 = 0x100;

/// Fixed text used for every route-originated I/O error.
///
/// Quinn renders I/O errors into its own logs, so the value must never carry
/// caller-controlled content.
const ROUTE_IO_TEXT: &str = "caller-owned datagram route terminated";

/// Terminal route failure shared between the bridge and its H3 generation.
///
/// The typed [`DialError`] is retained so the connector can classify the
/// failure with caller evidence. The [`io::ErrorKind`] is retained separately so
/// the caller's message never reaches a Quinn log line.
#[derive(Debug)]
struct RouteFailure {
    error: DialError,
    io_kind: io::ErrorKind,
}

impl RouteFailure {
    fn new(error: DialError) -> Self {
        let io_kind = match error.kind() {
            DialErrorKind::Timeout => io::ErrorKind::TimedOut,
            DialErrorKind::Authentication | DialErrorKind::Rejected => {
                io::ErrorKind::PermissionDenied
            }
            DialErrorKind::Connection | DialErrorKind::Other => io::ErrorKind::Other,
        };
        Self { error, io_kind }
    }

    fn io_error(&self) -> io::Error {
        io::Error::new(self.io_kind, ROUTE_IO_TEXT)
    }
}

/// Poller waker set with an explicit bound and `will_wake` de-duplication.
#[derive(Default)]
struct RouteWakers {
    wakers: Vec<Waker>,
}

impl RouteWakers {
    fn register(&mut self, cx: &Context<'_>) {
        if self.wakers.iter().any(|w| w.will_wake(cx.waker())) {
            return;
        }
        if self.wakers.len() < MAX_ROUTE_WAKERS {
            self.wakers.push(cx.waker().clone());
        }
    }

    fn wake_all(&mut self) {
        for waker in self.wakers.drain(..) {
            waker.wake();
        }
    }
}

impl fmt::Debug for RouteWakers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteWakers")
            .field("registered", &self.wakers.len())
            .finish()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock would mean a route worker panicked. The bridge keeps its
    // state valid regardless, so recovering is strictly better than propagating
    // a panic into unrelated request tasks.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Shared terminal-state signal observed by the H3 driver task.
///
/// A `watch` channel is used rather than a bare notification so a failure
/// recorded before the driver task starts polling is never lost.
#[derive(Debug)]
struct RouteFailureSignal {
    sender: tokio::sync::watch::Sender<bool>,
}

impl RouteFailureSignal {
    fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        (Self { sender }, receiver)
    }

    fn fail(&self) {
        // `send_replace` rather than `send`: `send` refuses to store the value
        // when no receiver is subscribed yet, which is exactly the case for a
        // failure recorded before the driver task starts polling. Storing the
        // value unconditionally is what makes the late watcher reliable.
        self.sender.send_replace(true);
    }
}

/// Bridge state shared with the two route worker tasks.
///
/// This is deliberately *not* the socket itself. The socket owns its workers'
/// `JoinHandle`s, so a worker holding an `Arc<RouteUdpSocket>` would keep that
/// socket alive through its own task handle and leak the whole generation.
/// Workers hold only this, and nothing here points back at the socket.
#[derive(Debug)]
struct RouteShared {
    failure: Mutex<Option<Arc<RouteFailure>>>,
    signal: Arc<RouteFailureSignal>,
    tx_wakers: Mutex<RouteWakers>,
    rx_wakers: Mutex<RouteWakers>,
}

impl RouteShared {
    fn new(signal: Arc<RouteFailureSignal>) -> Self {
        Self {
            failure: Mutex::new(None),
            signal,
            tx_wakers: Mutex::new(RouteWakers::default()),
            rx_wakers: Mutex::new(RouteWakers::default()),
        }
    }

    fn record_failure(&self, error: DialError) {
        let mut failure = lock(&self.failure);
        if failure.is_none() {
            *failure = Some(Arc::new(RouteFailure::new(error)));
            self.signal.fail();
        }
        drop(failure);
        // A terminated route must unblock both directions immediately, so a
        // parked QUIC driver never waits on a queue that can never drain.
        lock(&self.tx_wakers).wake_all();
        lock(&self.rx_wakers).wake_all();
    }

    fn failure(&self) -> Option<Arc<RouteFailure>> {
        lock(&self.failure).clone()
    }

    fn io_error(&self) -> Option<io::Error> {
        self.failure().map(|failure| failure.io_error())
    }

    /// Typed evidence for the connector's classification, rebuilt from the
    /// retained kind so the caller's message is never re-exposed.
    fn typed_failure(&self) -> Option<DialError> {
        self.failure()
            .map(|failure| DialError::new(failure.error.kind(), ROUTE_IO_TEXT))
    }
}

/// Quinn [`AsyncUdpSocket`](quinn::AsyncUdpSocket) over a caller-owned route.
///
/// One socket serves exactly one route-owned endpoint generation. It is built
/// only after a [`DatagramDialer`] has returned a route, so configuring a dialer
/// never instantiates a direct UDP socket: a failed or refused route cannot
/// silently fall back to opening one.
pub(crate) struct RouteUdpSocket {
    // Held for the whole generation so the route provably outlives every
    // handshake, stream, and eviction decision. Never rendered by `Debug`.
    _route: Arc<dyn DatagramRoute>,
    peer: SocketAddr,
    local: SocketAddr,
    shared: Arc<RouteShared>,
    // `mpsc` supplies the bounded queues for free: `try_send`/`try_recv` are the
    // synchronous halves Quinn needs and `recv`/`reserve` are the awaiting halves
    // the workers need, so queue depth bounds memory exactly.
    tx: tokio::sync::mpsc::Sender<Bytes>,
    rx: Mutex<tokio::sync::mpsc::Receiver<Bytes>>,
    workers: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl fmt::Debug for RouteUdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteUdpSocket")
            .field("peer", &self.peer)
            .field("local", &self.local)
            .field("send_capacity", &self.tx.capacity())
            .field("terminated", &self.shared.failure().is_some())
            .finish_non_exhaustive()
    }
}

impl Drop for RouteUdpSocket {
    fn drop(&mut self) {
        // Dropping a generation must not leak the two route workers. They only
        // ever await caller-owned futures, so abort is the correct teardown and
        // depends on the documented cancellation contract of the public trait.
        for worker in std::mem::take(&mut *lock(&self.workers)) {
            worker.abort();
        }
    }
}

impl RouteUdpSocket {
    /// Builds the socket and starts its two route worker tasks.
    pub(crate) fn from_route(route: Arc<dyn DatagramRoute>) -> Arc<Self> {
        let peer = route.peer_addr();
        let local = route.local_addr();
        let (tx, tx_rx) = tokio::sync::mpsc::channel::<Bytes>(ROUTE_QUEUE_DEPTH);
        let (rx_tx, rx) = tokio::sync::mpsc::channel::<Bytes>(ROUTE_QUEUE_DEPTH);
        let (signal, _initial) = RouteFailureSignal::new();
        let send_route = Arc::clone(&route);
        let recv_route = Arc::clone(&route);

        let shared = Arc::new(RouteShared::new(Arc::new(signal)));
        let socket = Arc::new(Self {
            _route: route,
            peer,
            local,
            shared: Arc::clone(&shared),
            tx,
            rx: Mutex::new(rx),
            workers: Mutex::new(Vec::with_capacity(2)),
        });

        let send_worker = tokio::spawn(Self::send_worker(send_route, tx_rx, Arc::clone(&shared)));
        let recv_worker = tokio::spawn(Self::recv_worker(recv_route, rx_tx, Arc::clone(&shared)));
        let mut workers = lock(&socket.workers);
        workers.push(send_worker);
        workers.push(recv_worker);
        drop(workers);

        socket
    }

    /// Returns the single remote peer this socket is bound to.
    pub(crate) fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// Returns a watcher that observes route termination without a lost wakeup.
    ///
    /// `watch` retains the last value, so a failure recorded before the driver
    /// task polls is still observed.
    pub(crate) fn failure_watcher(&self) -> tokio::sync::watch::Receiver<bool> {
        self.shared.signal.sender.subscribe()
    }

    /// Returns the typed route failure for this generation, if it terminated.
    ///
    /// The connector consults this once per dispatch so route evidence replaces
    /// a generic QUIC error exactly once for the generation it belongs to.
    pub(crate) fn route_failure(&self) -> Option<DialError> {
        self.shared.typed_failure()
    }

    async fn send_worker(
        route: Arc<dyn DatagramRoute>,
        mut rx: tokio::sync::mpsc::Receiver<Bytes>,
        shared: Arc<RouteShared>,
    ) {
        while let Some(datagram) = rx.recv().await {
            // Capacity just returned, so wake any poller Quinn parked on
            // `WouldBlock` before the possibly slow caller send.
            lock(&shared.tx_wakers).wake_all();
            if let Err(error) = route.send(&datagram).await {
                shared.record_failure(error);
                return;
            }
        }
    }

    async fn recv_worker(
        route: Arc<dyn DatagramRoute>,
        tx: tokio::sync::mpsc::Sender<Bytes>,
        shared: Arc<RouteShared>,
    ) {
        let mut buffer = vec![0u8; ROUTE_MTU];
        loop {
            // Reserving before the receive is what makes receive-queue
            // saturation back-pressured rather than lossy: the route is not asked
            // for another datagram until the bridge has room to keep it.
            let Ok(permit) = tx.reserve().await else {
                return;
            };
            let len = match route.recv(&mut buffer).await {
                Ok(len) => usize::min(len, buffer.len()),
                Err(error) => {
                    shared.record_failure(error);
                    return;
                }
            };
            permit.send(Bytes::copy_from_slice(&buffer[..len]));
            lock(&shared.rx_wakers).wake_all();
        }
    }
}

impl quinn::AsyncUdpSocket for RouteUdpSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn quinn::UdpPoller>> {
        // `Weak` keeps poller ownership acyclic: when a generation releases the
        // socket its workers stop with it, and a lingering poller reports a
        // closed transport instead of keeping the route alive.
        Box::pin(RouteUdpPoller {
            socket: Arc::downgrade(&self),
        })
    }

    fn try_send(&self, transmit: &Transmit<'_>) -> io::Result<()> {
        if transmit.destination != self.peer {
            // Fail closed: a route is single-peer by contract, so a different
            // destination means QUIC is being redirected off the agreed peer.
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "datagram route is bound to a single peer",
            ));
        }
        // One datagram per send. Batched segments and ECN codepoints are
        // refused rather than honoured: `max_transmit_segments` is 1, so
        // neither is ever produced for this socket.
        if transmit
            .segment_size
            .is_some_and(|segment_size| segment_size != transmit.contents.len())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "datagram route does not accept batched segments",
            ));
        }
        let _ = transmit.ecn;

        if let Some(failure) = self.shared.io_error() {
            return Err(failure);
        }

        // One datagram per send keeps ordering with the route's own send future.
        // This copy is the one allocation the experimental seam accepts; queue
        // depth bounds it to a constant per generation.
        match self.tx.try_send(Bytes::copy_from_slice(transmit.contents)) {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "caller-owned datagram route send queue is full",
            )),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err(self
                .shared
                .io_error()
                .unwrap_or_else(|| io::Error::other(ROUTE_IO_TEXT))),
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let datagram = lock(&self.rx).try_recv();
        match datagram {
            Ok(datagram) => {
                let Some(buf) = bufs.first_mut() else {
                    return Poll::Ready(Ok(0));
                };
                let len = datagram.len().min(buf.len());
                buf[..len].copy_from_slice(&datagram[..len]);
                let Some(slot) = meta.first_mut() else {
                    return Poll::Ready(Ok(0));
                };
                // No ECN and no destination IP: neither is observable through
                // this contract, so claiming them would be untrue.
                *slot = RecvMeta {
                    addr: self.peer,
                    len,
                    stride: len,
                    ecn: None,
                    dst_ip: None,
                };
                Poll::Ready(Ok(1))
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                if let Some(failure) = self.shared.io_error() {
                    // A terminal route failure must reach Quinn so the endpoint
                    // stops driving a dead generation. The H3 driver task closes
                    // the connection separately, carrying the typed evidence.
                    return Poll::Ready(Err(failure));
                }
                // Wake only for queued data or a terminal failure: the route's
                // own receive future is polled by the bridge worker, never here.
                lock(&self.shared.rx_wakers).register(cx);
                Poll::Pending
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "caller-owned datagram route receive worker stopped",
                )))
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }

    fn max_receive_segments(&self) -> usize {
        1
    }

    fn may_fragment(&self) -> bool {
        // `true` means "may fragment", which makes Quinn clear `allow_mtud` and
        // therefore never attempt path-MTU discovery over a carrier whose real
        // MTU cannot be measured from here.
        true
    }
}

fn released() -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        "caller-owned datagram route released",
    )
}

/// Quinn poller view over a [`RouteUdpSocket`].
struct RouteUdpPoller {
    socket: Weak<RouteUdpSocket>,
}

impl fmt::Debug for RouteUdpPoller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteUdpPoller").finish_non_exhaustive()
    }
}

impl quinn::UdpPoller for RouteUdpPoller {
    fn poll_writable(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Some(socket) = self.socket.upgrade() else {
            return Poll::Ready(Err(released()));
        };
        if let Some(failure) = socket.shared.io_error() {
            return Poll::Ready(Err(failure));
        }
        // Writable readiness is queue capacity, and the send worker wakes the
        // registered waker the moment capacity returns. That transition is what
        // makes the retried `try_send` observable.
        if socket.tx.capacity() > 0 {
            Poll::Ready(Ok(()))
        } else {
            lock(&socket.shared.tx_wakers).register(cx);
            Poll::Pending
        }
    }
}
// ---------------------------------------------------------------------------
// Bridge invariants
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    const PEER: &str = "203.0.113.7:4433";
    const LOCAL: &str = "203.0.113.9:51000";

    /// Minimal in-crate route whose receive side never completes and whose send
    /// side records what it was asked to carry.
    struct TestRoute {
        peer: SocketAddr,
        sent: Mutex<Vec<usize>>,
        /// Inbound datagram handed to the bridge on the first `recv`.
        inbound: Mutex<Option<Vec<u8>>>,
    }

    impl TestRoute {
        fn new() -> Arc<Self> {
            Self::with_inbound(None)
        }

        fn with_inbound(inbound: Option<&'static [u8]>) -> Arc<Self> {
            Arc::new(Self {
                peer: PEER.parse().expect("peer"),
                sent: Mutex::new(Vec::new()),
                inbound: Mutex::new(inbound.map(<[u8]>::to_vec)),
            })
        }
    }

    impl fmt::Debug for TestRoute {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestRoute")
                .field("private_detail", &"TEST-ROUTE-SECRET")
                .finish()
        }
    }

    impl DatagramRoute for TestRoute {
        fn send<'a>(&'a self, payload: &'a [u8]) -> DatagramSendFuture<'a> {
            self.sent
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(payload.len());
            Box::pin(async { Ok(()) })
        }

        fn recv<'a>(&'a self, buffer: &'a mut [u8]) -> DatagramRecvFuture<'a> {
            let pending = self
                .inbound
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            Box::pin(async move {
                match pending {
                    Some(datagram) => {
                        let len = datagram.len().min(buffer.len());
                        buffer[..len].copy_from_slice(&datagram[..len]);
                        Ok(len)
                    }
                    None => futures_util::future::pending().await,
                }
            })
        }

        fn local_addr(&self) -> SocketAddr {
            LOCAL.parse().expect("local")
        }

        fn peer_addr(&self) -> SocketAddr {
            self.peer
        }
    }

    fn transmit<'a>(destination: &str, contents: &'a [u8]) -> Transmit<'a> {
        Transmit {
            destination: destination.parse().expect("destination"),
            ecn: None,
            contents,
            segment_size: None,
            src_ip: None,
        }
    }

    #[tokio::test]
    async fn socket_debug_never_renders_the_route_implementation() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        let rendered = format!("{socket:?}");
        assert!(
            !rendered.contains("TEST-ROUTE-SECRET"),
            "bridge Debug must not expose the route: {rendered}"
        );
        assert!(
            !rendered.contains("TestRoute"),
            "bridge Debug must not name the provider type: {rendered}"
        );
        assert!(rendered.contains(PEER), "path addresses stay observable");
    }

    #[tokio::test]
    async fn a_foreign_destination_is_refused() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        let foreign = "198.51.100.4:443";
        let error = quinn::AsyncUdpSocket::try_send(&*socket, &transmit(foreign, b"payload"))
            .expect_err("a single-peer route must refuse redirection");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn batched_segments_are_refused() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        let mut batched = transmit(PEER, b"aaaabbbb");
        batched.segment_size = Some(4);
        let error = quinn::AsyncUdpSocket::try_send(&*socket, &batched)
            .expect_err("one datagram per send is the bridge contract");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn poll_recv_reports_the_fixed_peer_without_ecn() {
        let socket = RouteUdpSocket::from_route(TestRoute::with_inbound(Some(b"first")));

        let mut buffer = vec![0u8; ROUTE_MTU];
        let mut bufs = vec![io::IoSliceMut::new(&mut buffer)];
        let mut meta = vec![RecvMeta {
            addr: "0.0.0.0:0".parse().expect("zero"),
            len: 0,
            stride: 0,
            ecn: None,
            dst_ip: None,
        }];
        let mut cx = Context::from_waker(Waker::noop());
        // The bridge's receive worker must first pull the datagram off the
        // route, so readiness arrives as a wake-up rather than synchronously.
        let mut ready = Poll::Pending;
        for _ in 0..100 {
            ready = quinn::AsyncUdpSocket::poll_recv(&*socket, &mut cx, &mut bufs, &mut meta);
            if ready.is_ready() {
                break;
            }
            tokio::task::yield_now().await;
        }

        assert!(
            matches!(ready, Poll::Ready(Ok(1))),
            "one datagram per call, got {ready:?}"
        );
        assert_eq!(meta[0].addr, PEER.parse().expect("peer"));
        assert_eq!(meta[0].len, 5);
        assert_eq!(meta[0].stride, 5);
        assert!(
            meta[0].ecn.is_none(),
            "ECN is not observable through this contract, so it must not be claimed"
        );
        assert!(meta[0].dst_ip.is_none());
    }

    #[tokio::test]
    async fn an_empty_receive_queue_never_polls_the_route() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        let mut buffer = vec![0u8; ROUTE_MTU];
        let mut bufs = vec![io::IoSliceMut::new(&mut buffer)];
        let mut meta = vec![RecvMeta {
            addr: "0.0.0.0:0".parse().expect("zero"),
            len: 0,
            stride: 0,
            ecn: None,
            dst_ip: None,
        }];
        let mut cx = Context::from_waker(Waker::noop());
        let ready = quinn::AsyncUdpSocket::poll_recv(&*socket, &mut cx, &mut bufs, &mut meta);
        assert!(
            matches!(ready, Poll::Pending),
            "receive readiness comes only from queued data"
        );
    }

    #[tokio::test]
    async fn a_saturated_send_queue_reports_backpressure() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        let payload = vec![0u8; ROUTE_MTU];
        let mut blocked = 0;
        for _ in 0..(ROUTE_QUEUE_DEPTH * 2) {
            match quinn::AsyncUdpSocket::try_send(&*socket, &transmit(PEER, &payload)) {
                Ok(()) => {}
                Err(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
                    blocked += 1;
                    break;
                }
            }
        }
        assert!(blocked > 0, "a bounded queue must not grow without limit");
    }

    #[tokio::test]
    async fn a_terminal_failure_stays_typed_and_is_reported_once() {
        let socket = RouteUdpSocket::from_route(TestRoute::new());
        assert!(socket.route_failure().is_none());

        socket.shared.record_failure(DialError::new(
            DialErrorKind::Authentication,
            "provider said no",
        ));
        socket
            .shared
            .record_failure(DialError::new(DialErrorKind::Connection, "later cause"));

        // First evidence wins: the generation reports one cause, not a race
        // between whatever the provider happened to return last.
        let failure = socket.route_failure().expect("terminal failure");
        assert_eq!(failure.kind(), DialErrorKind::Authentication);
        assert_eq!(
            failure.message(),
            ROUTE_IO_TEXT,
            "the provider's message must not be re-exposed by the bridge"
        );

        // Once terminated, both directions report I/O rather than hanging.
        let error = quinn::AsyncUdpSocket::try_send(&*socket, &transmit(PEER, b"payload"))
            .expect_err("a terminated route cannot send");
        assert!(error.to_string().contains(ROUTE_IO_TEXT));
        assert!(
            !error.to_string().contains("provider said no"),
            "caller text must never reach a Quinn-facing error"
        );

        let watcher = socket.failure_watcher();
        assert!(
            *watcher.borrow(),
            "the terminal signal is retained so a late watcher still observes it"
        );
    }

    #[tokio::test]
    async fn the_route_is_released_with_the_socket() {
        struct CountedRoute {
            dropped: Arc<AtomicBool>,
        }

        impl fmt::Debug for CountedRoute {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("CountedRoute")
            }
        }

        impl DatagramRoute for CountedRoute {
            fn send<'a>(&'a self, _payload: &'a [u8]) -> DatagramSendFuture<'a> {
                Box::pin(futures_util::future::pending())
            }

            fn recv<'a>(&'a self, _buffer: &'a mut [u8]) -> DatagramRecvFuture<'a> {
                Box::pin(futures_util::future::pending())
            }

            fn local_addr(&self) -> SocketAddr {
                LOCAL.parse().expect("local")
            }

            fn peer_addr(&self) -> SocketAddr {
                PEER.parse().expect("peer")
            }
        }

        impl Drop for CountedRoute {
            fn drop(&mut self) {
                self.dropped
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        {
            let socket = RouteUdpSocket::from_route(Arc::new(CountedRoute {
                dropped: Arc::clone(&dropped),
            }));
            assert!(!dropped.load(std::sync::atomic::Ordering::SeqCst));
            // Give both workers a chance to park on the caller's futures so the
            // drop path has something real to tear down.
            tokio::task::yield_now().await;
            drop(socket);
        }
        // The workers hold their own route clones; aborting them on drop
        // releases the final reference.
        for _ in 0..50 {
            if dropped.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            dropped.load(std::sync::atomic::Ordering::SeqCst),
            "dropping the bridge must release the caller's route"
        );
    }
}
