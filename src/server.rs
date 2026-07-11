//! Server handles: `JavaServer`, `BedrockServer`, `LegacyServer`. Each has
//! sync methods always, plus async counterparts (`_async` suffix) behind
//! the `async` feature.

use std::time::Duration;

use crate::address::Address;
use crate::error::RustmcError;
use crate::protocol::io::{TcpConnection, UdpConnection};
use crate::protocol::java::JavaClient;
use crate::protocol::query::QueryClient;
use crate::resolver::AddressResolver;
use crate::response::{JavaStatusResponse, QueryBasicResponse, QueryResponse};

/// Default timeout used by `lookup`/`lookup_async` when no explicit
/// timeout is given via `with_timeout`.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Fixed delay between retries. Not exponential — timeouts here are
/// already short (seconds) and `tries` is small (default 3), so
/// exponential backoff adds complexity without meaningful benefit at this
/// scale.
const RETRY_DELAY: Duration = Duration::from_millis(250);

pub struct JavaServer {
    pub address: Address,
    pub timeout: Duration,
    pub tries: usize,
    /// Client protocol version sent in the handshake. `-1` is used as a
    /// sentinel (matching `mcstatus`/`mc-query`): the server does not
    /// validate this during a status request, it only matters for a real
    /// login, so there's no reason to hardcode (and have to keep updating)
    /// a specific protocol number here.
    pub version: i32,
    pub ping_token: Option<i64>,
    /// Port for the GameSpy4 Query protocol (`query()`/`query_basic()`).
    /// Defaults to the same port as `address.port` (the common case), but
    /// `enable-query`'s `query.port` in `server.properties` can be set to
    /// something else — use `.with_query_port(..)` when it is.
    pub query_port: u16,
    resolver: AddressResolver,
}

impl JavaServer {
    /// Creates a server handle without resolving anything yet — resolution
    /// happens lazily on `status()`/`ping()`.
    pub fn new(address: Address, timeout: Duration) -> Self {
        let query_port = address.port;
        Self {
            address,
            timeout,
            tries: 3,
            version: -1,
            ping_token: None,
            query_port,
            resolver: AddressResolver::new(),
        }
    }

    /// Parses `host` (`host:port`, `host`, or bracketed IPv6) and builds a
    /// server handle with `DEFAULT_TIMEOUT`. To use a different timeout,
    /// chain `.with_timeout(..)` after this call.
    ///
    /// With the `srv` feature enabled: if `host` has no explicit port, this
    /// performs a `_minecraft._tcp` SRV lookup and uses its target/port
    /// when found (mirroring the real Minecraft client's own address-bar
    /// behavior), falling back to `25565` if there's no SRV record.
    /// Without `srv`, a missing port always falls back to `25565` directly.
    ///
    /// This SRV lookup is a one-shot resolution done once here, not cached
    /// even if `.with_cache(..)` is chained afterwards — caching mainly
    /// pays off for lookups that repeat (like the A/AAAA resolution done on
    /// every `status()`/`ping()` call), and this one doesn't repeat within
    /// a single `JavaServer`'s lifetime. Use `AddressResolver::resolve_srv`
    /// directly if you need a cached SRV lookup across multiple
    /// `JavaServer::lookup` calls for the same host.
    pub fn lookup(host: &str) -> Result<Self, RustmcError> {
        let address = resolve_lookup_address(host, 25565)?;
        Ok(Self::new(address, DEFAULT_TIMEOUT))
    }

    /// Async counterpart to `lookup`.
    #[cfg(feature = "async")]
    pub async fn lookup_async(host: &str) -> Result<Self, RustmcError> {
        let address = resolve_lookup_address_async(host, 25565).await?;
        Ok(Self::new(address, DEFAULT_TIMEOUT))
    }

    /// Enables DNS/SRV caching backed by the given `DnsCache` (requires the
    /// `dns_cache` feature). Pass a shared `Arc<DnsCache>` to have multiple
    /// server handles share one cache instead of each resolving
    /// independently.
    #[cfg(feature = "dns_cache")]
    #[must_use]
    pub fn with_cache(mut self, cache: std::sync::Arc<crate::cache::DnsCache>) -> Self {
        self.resolver = AddressResolver::with_cache(cache);
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Overrides the Query protocol port (see the `query_port` field docs).
    #[must_use]
    pub fn with_query_port(mut self, port: u16) -> Self {
        self.query_port = port;
        self
    }

    /// DNS/SRV cache stats, if a cache was configured via `with_cache`.
    /// `None` when the `dns_cache` feature is disabled or no cache is set.
    #[cfg(feature = "dns_cache")]
    pub fn cache_stats(&self) -> Option<crate::cache::CacheStats> {
        self.resolver.cache_stats()
    }

    /// Generates a pseudo-random token when none was set explicitly.
    ///
    /// No `rand` dependency: `RandomState`'s hasher is seeded randomly per
    /// process by `std`, so hashing a couple of high-resolution clock reads
    /// through it gives an adequately unpredictable `i64` for a value
    /// that's only ever compared against the server's echo of it — no
    /// cryptographic property is needed here.
    fn ping_token(&self) -> i64 {
        if let Some(t) = self.ping_token {
            return t;
        }
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};
        let mut hasher = RandomState::new().build_hasher();
        std::time::Instant::now().hash(&mut hasher);
        std::time::SystemTime::now().hash(&mut hasher);
        hasher.finish() as i64
    }

    fn connect(&self) -> Result<(JavaClient, Option<crate::response::DnsInfo>), RustmcError> {
        let (addr, dns_info) = self.resolver.resolve_with_info(&self.address)?;
        let conn = TcpConnection::connect(addr, self.timeout)?;
        let client = JavaClient::new(conn, self.address.clone(), self.version, self.ping_token());
        Ok((client, dns_info))
    }

    /// Requests the server's status (MOTD, players, version) and measures
    /// latency for the status round-trip.
    ///
    /// # Errors
    /// Returns the last `RustmcError` after retrying up to `self.tries`
    /// times on I/O failure.
    pub fn status(&self) -> Result<JavaStatusResponse, RustmcError> {
        retry(self.tries, || {
            let (mut client, dns_info) = self.connect()?;
            client.handshake()?;
            let (raw, latency) = client.read_status()?;
            let mut response = JavaStatusResponse::build(raw, latency)?;
            response.dns = dns_info.clone();
            Ok(response)
        })
    }

    /// Measures round-trip latency via a dedicated ping packet (separate
    /// from the status request's own latency measurement).
    pub fn ping(&self) -> Result<f64, RustmcError> {
        retry(self.tries, || {
            let (mut client, _dns_info) = self.connect()?;
            client.handshake()?;
            client.test_ping()
        })
    }

    fn connect_udp(&self) -> Result<UdpConnection, RustmcError> {
        let ip = self.resolver.resolve(&self.address)?;
        UdpConnection::connect(std::net::SocketAddr::new(ip, self.query_port), self.timeout)
    }

    /// GameSpy4 Query protocol, full stat (requires `enable-query=true` in
    /// `server.properties`; see `query_port` if `query.port` differs from
    /// the game port).
    pub fn query(&self) -> Result<QueryResponse, RustmcError> {
        retry(self.tries, || {
            let mut client = QueryClient::new(self.connect_udp()?);
            client.handshake()?;
            client.full_stat()
        })
    }

    /// GameSpy4 Query protocol, basic stat. Smaller response (no player
    /// list, no plugins) that some servers expose even without full query
    /// enabled.
    pub fn query_basic(&self) -> Result<QueryBasicResponse, RustmcError> {
        retry(self.tries, || {
            let mut client = QueryClient::new(self.connect_udp()?);
            client.handshake()?;
            client.basic_stat()
        })
    }

    #[cfg(feature = "async")]
    async fn connect_async(
        &self,
    ) -> Result<(crate::protocol::java::AsyncJavaClient, Option<crate::response::DnsInfo>), RustmcError> {
        let (addr, dns_info) = self.resolver.resolve_with_info(&self.address)?;
        let conn = crate::protocol::io::AsyncTcpConnection::connect(addr, self.timeout).await?;
        let client = crate::protocol::java::AsyncJavaClient::new(
            conn,
            self.address.clone(),
            self.version,
            self.ping_token(),
        );
        Ok((client, dns_info))
    }

    #[cfg(feature = "async")]
    async fn connect_udp_async(&self) -> Result<crate::protocol::io::AsyncUdpConnection, RustmcError> {
        let ip = self.resolver.resolve(&self.address)?;
        crate::protocol::io::AsyncUdpConnection::connect(
            std::net::SocketAddr::new(ip, self.query_port),
            self.timeout,
        )
        .await
    }

    /// Async counterpart to `status()`.
    #[cfg(feature = "async")]
    pub async fn status_async(&self) -> Result<JavaStatusResponse, RustmcError> {
        retry_async(self.tries, || async {
            let (mut client, dns_info) = self.connect_async().await?;
            client.handshake().await?;
            let (raw, latency) = client.read_status().await?;
            let mut response = JavaStatusResponse::build(raw, latency)?;
            response.dns = dns_info;
            Ok(response)
        })
        .await
    }

    /// Async counterpart to `ping()`.
    #[cfg(feature = "async")]
    pub async fn ping_async(&self) -> Result<f64, RustmcError> {
        retry_async(self.tries, || async {
            let (mut client, _dns) = self.connect_async().await?;
            client.handshake().await?;
            client.test_ping().await
        })
        .await
    }

    /// Async counterpart to `query()`.
    #[cfg(feature = "async")]
    pub async fn query_async(&self) -> Result<QueryResponse, RustmcError> {
        retry_async(self.tries, || async {
            let mut client = crate::protocol::query::AsyncQueryClient::new(self.connect_udp_async().await?);
            client.handshake().await?;
            client.full_stat().await
        })
        .await
    }

    /// Async counterpart to `query_basic()`.
    #[cfg(feature = "async")]
    pub async fn query_basic_async(&self) -> Result<QueryBasicResponse, RustmcError> {
        retry_async(self.tries, || async {
            let mut client = crate::protocol::query::AsyncQueryClient::new(self.connect_udp_async().await?);
            client.handshake().await?;
            client.basic_stat().await
        })
        .await
    }
}

/// Shared by `JavaServer::lookup`: resolves SRV (if the `srv` feature is
/// enabled and no port was given) or falls back to a plain
/// `Address::parse`.
fn resolve_lookup_address(host: &str, default_port: u16) -> Result<Address, RustmcError> {
    #[cfg(feature = "srv")]
    {
        crate::address::srv_lookup(host, default_port)
    }
    #[cfg(not(feature = "srv"))]
    {
        Address::parse(host, default_port)
    }
}

#[cfg(feature = "async")]
async fn resolve_lookup_address_async(host: &str, default_port: u16) -> Result<Address, RustmcError> {
    #[cfg(feature = "srv")]
    {
        crate::address::srv_lookup_async(host, default_port).await
    }
    #[cfg(not(feature = "srv"))]
    {
        Address::parse(host, default_port)
    }
}

fn retry<T, F: FnMut() -> Result<T, RustmcError>>(tries: usize, mut f: F) -> Result<T, RustmcError> {
    let mut last_err = None;
    for attempt in 0..tries.max(1) {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < tries {
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }
    Err(last_err.unwrap())
}

#[cfg(feature = "async")]
async fn retry_async<T, Fut, F>(tries: usize, mut f: F) -> Result<T, RustmcError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, RustmcError>>,
{
    let mut last_err = None;
    for attempt in 0..tries.max(1) {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < tries {
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

/// Bedrock Edition server handle (RakNet Unconnected Ping/Pong over UDP).
/// Default port is `19132`, not `25565`.
pub struct BedrockServer {
    pub address: Address,
    pub timeout: Duration,
    pub tries: usize,
    resolver: AddressResolver,
}

impl BedrockServer {
    pub fn new(address: Address, timeout: Duration) -> Self {
        Self { address, timeout, tries: 3, resolver: AddressResolver::new() }
    }

    /// Parses `host` with Bedrock's default port `19132`. Bedrock has no
    /// SRV convention (unlike Java's `_minecraft._tcp`), so this always
    /// uses the explicit port or `19132`, regardless of the `srv` feature.
    pub fn lookup(host: &str) -> Result<Self, RustmcError> {
        let address = Address::parse(host, 19132)?;
        Ok(Self::new(address, DEFAULT_TIMEOUT))
    }

    /// Async counterpart to `lookup`. No SRV involved (see `lookup` docs),
    /// so this is just `Address::parse` — provided for API symmetry with
    /// `JavaServer::lookup_async`.
    #[cfg(feature = "async")]
    pub async fn lookup_async(host: &str) -> Result<Self, RustmcError> {
        Self::lookup(host)
    }

    #[cfg(feature = "dns_cache")]
    #[must_use]
    pub fn with_cache(mut self, cache: std::sync::Arc<crate::cache::DnsCache>) -> Self {
        self.resolver = AddressResolver::with_cache(cache);
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(feature = "dns_cache")]
    pub fn cache_stats(&self) -> Option<crate::cache::CacheStats> {
        self.resolver.cache_stats()
    }

    pub fn status(&self) -> Result<crate::response::BedrockStatusResponse, RustmcError> {
        retry(self.tries, || {
            let addr = self.resolver.resolve_socket_addr(&self.address)?;
            let conn = crate::protocol::io::UdpConnection::connect(addr, self.timeout)?;
            crate::protocol::bedrock::BedrockClient::new(conn).read_status()
        })
    }

    #[cfg(feature = "async")]
    pub async fn status_async(&self) -> Result<crate::response::BedrockStatusResponse, RustmcError> {
        retry_async(self.tries, || async {
            let addr = self.resolver.resolve_socket_addr_async(&self.address).await?;
            let conn = crate::protocol::io::AsyncUdpConnection::connect(addr, self.timeout).await?;
            crate::protocol::bedrock::AsyncBedrockClient::new(conn).read_status().await
        })
        .await
    }
}

/// Legacy (pre-1.7) Java server handle. Uses the same default port as
/// modern Java Edition (`25565`) — legacy and modern SLP share a listening
/// port, they're distinguished by the request bytes, not the port.
pub struct LegacyServer {
    pub address: Address,
    pub timeout: Duration,
    pub tries: usize,
    resolver: AddressResolver,
}

impl LegacyServer {
    pub fn new(address: Address, timeout: Duration) -> Self {
        Self { address, timeout, tries: 3, resolver: AddressResolver::new() }
    }

    pub fn lookup(host: &str) -> Result<Self, RustmcError> {
        let address = Address::parse(host, 25565)?;
        Ok(Self::new(address, DEFAULT_TIMEOUT))
    }

    #[cfg(feature = "async")]
    pub async fn lookup_async(host: &str) -> Result<Self, RustmcError> {
        Self::lookup(host)
    }

    #[cfg(feature = "dns_cache")]
    #[must_use]
    pub fn with_cache(mut self, cache: std::sync::Arc<crate::cache::DnsCache>) -> Self {
        self.resolver = AddressResolver::with_cache(cache);
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(feature = "dns_cache")]
    pub fn cache_stats(&self) -> Option<crate::cache::CacheStats> {
        self.resolver.cache_stats()
    }

    pub fn status(&self) -> Result<crate::response::LegacyStatusResponse, RustmcError> {
        retry(self.tries, || {
            let addr = self.resolver.resolve_socket_addr(&self.address)?;
            let conn = TcpConnection::connect(addr, self.timeout)?;
            crate::protocol::legacy::LegacyClient::new(conn).read_status()
        })
    }

    #[cfg(feature = "async")]
    pub async fn status_async(&self) -> Result<crate::response::LegacyStatusResponse, RustmcError> {
        retry_async(self.tries, || async {
            let addr = self.resolver.resolve_socket_addr_async(&self.address).await?;
            let conn = crate::protocol::io::AsyncTcpConnection::connect(addr, self.timeout).await?;
            crate::protocol::legacy::AsyncLegacyClient::new(conn).read_status().await
        })
        .await
    }
}
