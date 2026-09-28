//! This backend, as the `zaino-chain-store` ports.
//!
//! # This module implements ports; it does not define them
//!
//! The driven ports live in the `zaino-chain-store` domain crate, which is
//! where a consumer should read their contracts. An adapter has none of its
//! own — it satisfies someone else's — so what is here is `impl <port> for
//! DbReader<T>` and `impl <port> for FinalisedState<T>`, plus the conversions
//! those impls need. The module was called `ports` until it was pointed out
//! that the name reads as "the port contracts are defined here", which inverts
//! the layering it exists to express.
//!
//! # Where things are
//!
//! - `error_map` — this backend's failures, as the domain names them.
//! - `to_domain` — reading: what is on disk, as the domain names it.
//! - `from_domain` — writing: what the domain hands over, as this stores it.
//! - `history` — the feature-gated transparent-history port.
//!
//! Named in code spans rather than doc links because the modules are private:
//! an intra-doc link to one resolves only under `--document-private-items`, so
//! linking them breaks a plain `cargo doc`.
//!
//! The two conversion directions are separate modules rather than one, because
//! they are separate concerns that happen to be inverse: one is what a read
//! must survive, the other what a write must preserve. They are *not* named for
//! the types they produce — `StoredBlock` is a **domain** type, so a module
//! called `into_stored` would hold the conversions *out of* this backend, which
//! is the opposite of what the name suggests. `to_domain` and `from_domain`
//! name the direction and cannot be read backwards.
//!
//! # What this module does *not* do
//!
//! It does not re-implement any read. Every method delegates to the existing
//! reader and converts the answer. Where a domain method has no single backend
//! counterpart — `unspent_output`, which is an existence check and a spend
//! check — the composition is here, because it is a question about the domain
//! rather than about LMDB.

mod error_map;
mod from_domain;
#[cfg(feature = "transparent_address_history_experimental")]
mod history;
mod to_domain;

pub use from_domain::indexed_block_from_stored;
pub use to_domain::{domain_outpoint, stored_tx_out};

pub(crate) use to_domain::domain_block_ref;

use error_map::{chain_store_error, chain_store_source_error};
use from_domain::{stored_hash, stored_height, stored_outpoint, tx_location};
use to_domain::{
    block_tx_position, domain_hash, domain_height, domain_txid, store_capabilities, store_schema,
    stored_block, stored_tx_outs,
};

use core::future::Future;

use zaino_chain_store::{
    ChainStoreError, ChainStoreFreezeSink, ChainStoreIngest, ChainStoreReader, ChainStoreService,
    ChainStoreSource, ChainStoreSourceError, CompactBlockRead, PoolFilter, SpenderRef,
    SpentOutputIndex, StoreCapabilities, StoreSchema, StoreWatermark, StoredBlock, StoredBlockRead,
    StoredTxOut, TransactionIndex, TxOutSetAccumulator, TxOutSetIndex,
};
use zaino_primitives::types::{
    BlockHash as DomainBlockHash, BlockTxPosition, CompactBlock, Height as DomainHeight,
    Outpoint as DomainOutpoint, TransactionId,
};
use zaino_status::StatusType;

use crate::error::StoreError;
use crate::store::reader::DbReader;
use crate::store::FinalisedState;
use crate::types::{Height, Outpoint, TransactionHash};

/// How long a read took, recorded against `DB_READ_SECONDS` under its `op`.
///
/// A type rather than a bare `Instant` so the `prometheus` feature is handled
/// once: without it this compiles to nothing and no call site needs a `cfg`.
///
/// The whole read surface shares one histogram, split by an `op` label naming
/// the read. The label is the port method's own name, so a new read is
/// instrumented by starting a timer with its name rather than by minting a
/// metric.
///
/// Recorded on drop, so a read that returns early through `?` still records: a
/// read that fails slowly is the symptom worth seeing, and dropping those
/// samples would make a degrading store look faster as it got worse.
struct ReadTimer {
    #[cfg(feature = "prometheus")]
    op: &'static str,
    #[cfg(feature = "prometheus")]
    started: std::time::Instant,
}

impl ReadTimer {
    /// Starts timing a read labelled `op` — the port method's own name.
    fn start(op: &'static str) -> Self {
        #[cfg(not(feature = "prometheus"))]
        let _ = op;
        Self {
            #[cfg(feature = "prometheus")]
            op,
            #[cfg(feature = "prometheus")]
            started: std::time::Instant::now(),
        }
    }
}

#[cfg(feature = "prometheus")]
impl Drop for ReadTimer {
    fn drop(&mut self) {
        metrics::histogram!(crate::metric_names::DB_READ_SECONDS, "op" => self.op)
            .record(self.started.elapsed().as_secs_f64());
    }
}

impl<T: ChainStoreSource> ChainStoreReader for DbReader<T> {
    fn watermark(&self) -> StoreWatermark {
        self.inner.watermark()
    }

    fn capabilities(&self) -> StoreCapabilities {
        store_capabilities::<Self>(self.inner.capability())
    }

    async fn schema(&self) -> Result<StoreSchema, ChainStoreError> {
        let metadata = self.get_metadata().await.map_err(chain_store_error)?;
        Ok(store_schema(&metadata))
    }

    #[tracing::instrument(skip(self), fields(height = %height))]
    async fn block_hash(
        &self,
        height: DomainHeight,
    ) -> Result<Option<DomainBlockHash>, ChainStoreError> {
        self.bounded(height)?;
        let _timer = ReadTimer::start("block_hash");
        Ok(DbReader::get_block_hash(self, stored_height(height))
            .await
            .map_err(chain_store_error)?
            .map(domain_hash))
    }

    #[tracing::instrument(skip(self), fields(hash = %hash))]
    async fn block_height(
        &self,
        hash: DomainBlockHash,
    ) -> Result<Option<DomainHeight>, ChainStoreError> {
        let _timer = ReadTimer::start("block_height");
        match DbReader::get_block_height(self, stored_hash(hash))
            .await
            .map_err(chain_store_error)?
        {
            Some(height) => Ok(Some(domain_height(height)?)),
            None => Ok(None),
        }
    }

    fn status(&self) -> StatusType {
        DbReader::status(self)
    }
}

impl<T: ChainStoreSource> DbReader<T> {
    /// Rejects a height the store cannot answer for yet.
    ///
    /// Above the watermark is not a miss: the block probably exists and simply
    /// is not finalised, so a caller must route the question elsewhere rather
    /// than conclude the chain has no such block.
    fn bounded(&self, height: DomainHeight) -> Result<(), ChainStoreError> {
        let watermark = self.inner.watermark();
        if watermark.covers(height) {
            return Ok(());
        }
        // A passthrough store is not bounded by what it holds, because it is
        // not answering from what it holds: the read goes to the validator, and
        // the validator has the block. The watermark still describes the
        // durable rows — that is what it is for — but using it as a *limit*
        // here would refuse a question this store can answer perfectly well,
        // which is exactly the case a store that is still building is in.
        if watermark.provenance == zaino_chain_store::Provenance::Passthrough {
            return Ok(());
        }
        // No tip at all is not "above the watermark" — there is no watermark to
        // be above. A store still opening or still empty is transiently unable
        // to answer, which is what the caller needs to know.
        match watermark.tip {
            Some(tip) => Err(ChainStoreError::AboveWatermark {
                requested: height,
                watermark: tip.height,
            }),
            None => Err(ChainStoreError::NotReady),
        }
    }
}

/// How many blocks one read transaction covers.
///
/// The existing compact-block walk uses the same figure, and for the same
/// reason: it bounds how long a reader slot is held without making the
/// per-transaction cost dominate. Chunk boundaries carry no meaning — a
/// consumer must not read anything into where one ends.
const BLOCKS_PER_READ_TRANSACTION: u32 = 1024;

/// How many blocks one read covers while reads are passed through to the
/// validator: one.
///
/// A passthrough read holds no transaction, so a larger chunk has no
/// per-transaction cost to amortise: the passthrough fetches a range with one
/// validator round trip per block, and a chunk reaches the consumer only once
/// every block in it has arrived. At `BLOCKS_PER_READ_TRANSACTION` that is a
/// thousand round trips of silence before the first block of any range that
/// starts below the finalised seam. On the SWARM production chain that was
/// twenty seconds and more, and a light wallet abandons a stream after fifteen
/// seconds without a message, so a wallet syncing from its birthday never
/// received a block. One block per chunk sends each block as soon as it has
/// been fetched, at the same total cost.
const BLOCKS_PER_PASSTHROUGH_READ: u32 = 1;

/// Splits `start..=end` into chunks of `blocks_per_chunk` blocks and reads each
/// one.
///
/// Sequential rather than concurrent: the chunks are contiguous and the
/// consumer wants them in order, so overlapping reads would buy nothing and
/// would hold several reader slots at once. The stream stops at the first
/// error — a range with a hole in the middle is not a range.
///
/// Lazy: a chunk is read when the consumer asks for it and not before, so the
/// first chunk is in the consumer's hands after one read however long the
/// range is. The cost of one read is therefore how long a consumer waits for
/// its first block, which is why the chunk size is the caller's to choose.
fn chunked<B, F, Fut>(
    range: Option<(Height, Height)>,
    blocks_per_chunk: u32,
    read: F,
) -> impl futures::Stream<Item = Result<Vec<B>, ChainStoreError>> + Send
where
    B: Send + 'static,
    F: FnMut(Height, Height) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Vec<B>, ChainStoreError>> + Send + 'static,
{
    // `range` is `None` when the whole request sits above the watermark. The
    // empty case is folded in here rather than returned as
    // `stream::empty()` from the caller, because the port hands back one
    // opaque stream type and two arms returning different types could not
    // both be it.
    let (cursor, end) = match range {
        Some((start, end)) => (Some(start), end),
        None => (None, Height(0)),
    };
    // How far past its first height a chunk reaches. A chunk size of zero is
    // read as one block: a chunk of no blocks would never move the cursor.
    let reach = blocks_per_chunk.saturating_sub(1);
    // The reader travels in the unfold's state rather than being borrowed by
    // the closure: the future the closure returns outlives the call, so a
    // borrow would tie the stream's lifetime to this frame.
    futures::stream::try_unfold((cursor, read), move |(cursor, mut read)| async move {
        let Some(from) = cursor else {
            return Ok(None);
        };
        let to = Height(from.0.saturating_add(reach).min(end.0));
        let chunk = read(from, to).await?;
        let next = (to.0 < end.0).then(|| Height(to.0 + 1));
        Ok(Some((chunk, (next, read))))
    })
}

impl<T: ChainStoreSource> DbReader<T> {
    /// Narrows `start..=end` to what this store can answer for.
    ///
    /// A range extending above the watermark is truncated rather than refused,
    /// so a consumer merging with the recent window asks both halves the same
    /// question and lets each answer what it holds. `None` means the whole range
    /// is above the watermark, which is an empty answer rather than an error —
    /// the other half has all of it.
    ///
    /// A range whose start is above its end is rejected: ranges are ascending,
    /// and a descending one is a caller mistake rather than an empty set.
    fn clamped_range(
        &self,
        start: DomainHeight,
        end: DomainHeight,
    ) -> Result<Option<(Height, Height)>, ChainStoreError> {
        if start > end {
            return Err(ChainStoreError::InvalidRange { start, end });
        }

        let watermark = self.inner.watermark();

        // Passthrough answers from the validator, so there is nothing to clamp
        // to — see [`Self::bounded`]. The ascending check above still applies,
        // because that one is about the request rather than about coverage.
        if watermark.provenance == zaino_chain_store::Provenance::Passthrough {
            return Ok(Some((stored_height(start), stored_height(end))));
        }

        let Some(tip) = watermark.tip else {
            return Err(ChainStoreError::NotReady);
        };
        if start > tip.height {
            return Ok(None);
        }
        Ok(Some((
            stored_height(start),
            stored_height(end.min(tip.height)),
        )))
    }

    /// How many blocks each chunk of a chunked read covers.
    ///
    /// Decided by where the answers come from, because that is what a chunk
    /// costs: committed rows are read under one transaction per chunk, while a
    /// passthrough pays one validator round trip per block whatever the chunk
    /// size. See `BLOCKS_PER_PASSTHROUGH_READ`.
    ///
    /// Read once, when a stream is built. A store that comes online part-way
    /// through a long stream keeps the chunk size the stream started with,
    /// which costs speed and nothing else: chunk boundaries carry no meaning.
    fn blocks_per_chunk(&self) -> u32 {
        match self.inner.watermark().provenance {
            zaino_chain_store::Provenance::Durable => BLOCKS_PER_READ_TRANSACTION,
            zaino_chain_store::Provenance::Passthrough => BLOCKS_PER_PASSTHROUGH_READ,
        }
    }
}

impl<T: ChainStoreSource> TransactionIndex for DbReader<T> {
    #[tracing::instrument(skip(self), fields(txid = %txid))]
    async fn tx_position(
        &self,
        txid: &TransactionId,
    ) -> Result<Option<BlockTxPosition>, ChainStoreError> {
        let _timer = ReadTimer::start("tx_position");
        match self
            .get_tx_location(&TransactionHash((*txid).into()))
            .await
            .map_err(chain_store_error)?
        {
            Some(location) => Ok(Some(block_tx_position(location)?)),
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip(self), fields(position = ?position))]
    async fn txid_at(
        &self,
        position: BlockTxPosition,
    ) -> Result<Option<TransactionId>, ChainStoreError> {
        self.bounded(position.height)?;
        let Some(location) = tx_location(position) else {
            return Ok(None);
        };
        let _timer = ReadTimer::start("txid_at");
        // The backend errors on a miss where the domain answers `None`: asking
        // about a position past the end of a block is a reasonable question.
        match self.get_txid(location).await {
            Ok(txid) => Ok(Some(domain_txid(txid))),
            Err(StoreError::DataUnavailable(_)) => Ok(None),
            Err(error) => Err(chain_store_error(error)),
        }
    }
}

impl<T: ChainStoreSource> SpentOutputIndex for DbReader<T> {
    #[tracing::instrument(skip(self, outpoints), fields(outpoints = outpoints.len()))]
    async fn outpoint_spenders(
        &self,
        outpoints: &[DomainOutpoint],
    ) -> Result<Vec<Option<SpenderRef>>, ChainStoreError> {
        let _timer = ReadTimer::start("outpoint_spenders");
        let stored: Vec<Outpoint> = outpoints.iter().map(stored_outpoint).collect();
        let locations = DbReader::get_outpoint_spenders(self, stored)
            .await
            .map_err(chain_store_error)?;

        // The domain answer carries the spender's txid as well as its position,
        // because every caller resolves one to the other immediately. Doing it
        // here costs the same reads and halves the traffic across the seam.
        let mut spenders = Vec::with_capacity(locations.len());
        for location in locations {
            let Some(location) = location else {
                spenders.push(None);
                continue;
            };
            let txid = self.get_txid(location).await.map_err(chain_store_error)?;
            spenders.push(Some(SpenderRef {
                position: block_tx_position(location)?,
                txid: domain_txid(txid),
            }));
        }
        Ok(spenders)
    }

    #[tracing::instrument(skip(self, outpoints), fields(outpoints = outpoints.len()))]
    async fn previous_outputs(
        &self,
        outpoints: &[DomainOutpoint],
    ) -> Result<Vec<Option<StoredTxOut>>, ChainStoreError> {
        let _timer = ReadTimer::start("previous_outputs");
        let mut outputs = Vec::with_capacity(outpoints.len());
        for outpoint in outpoints {
            outputs.push(self.previous_output(outpoint).await?);
        }
        Ok(outputs)
    }

    #[tracing::instrument(skip(self), fields(outpoint = ?outpoint))]
    async fn unspent_output(
        &self,
        outpoint: DomainOutpoint,
    ) -> Result<Option<StoredTxOut>, ChainStoreError> {
        let _timer = ReadTimer::start("unspent_output");
        let Some(output) = self.previous_output(&outpoint).await? else {
            return Ok(None);
        };
        let spent = DbReader::get_outpoint_spender(self, stored_outpoint(&outpoint))
            .await
            .map_err(chain_store_error)?
            .is_some();

        Ok((!spent).then_some(output))
    }

    #[tracing::instrument(skip(self), fields(position = ?position))]
    async fn transparent_outputs(
        &self,
        position: BlockTxPosition,
    ) -> Result<Option<Vec<StoredTxOut>>, ChainStoreError> {
        self.bounded(position.height)?;
        let Some(location) = tx_location(position) else {
            return Ok(None);
        };
        let _timer = ReadTimer::start("transparent_outputs");
        match DbReader::get_transparent(self, location)
            .await
            .map_err(chain_store_error)?
        {
            Some(transparent) => Ok(Some(stored_tx_outs(&transparent)?)),
            None => Ok(None),
        }
    }
}

impl<T: ChainStoreSource> DbReader<T> {
    /// One outpoint's output, with a miss as `None`.
    ///
    /// The backend errors when the creating transaction is absent; the domain
    /// treats that as an answer, because an outpoint naming a transaction this
    /// store does not hold is exactly what a caller merging across the seam
    /// expects to see.
    async fn previous_output(
        &self,
        outpoint: &DomainOutpoint,
    ) -> Result<Option<StoredTxOut>, ChainStoreError> {
        match DbReader::get_previous_output(self, stored_outpoint(outpoint)).await {
            Ok(output) => Ok(Some(stored_tx_out(&output)?)),
            Err(StoreError::DataUnavailable(_)) => Ok(None),
            Err(error) => Err(chain_store_error(error)),
        }
    }
}

impl<T: ChainStoreSource> TxOutSetIndex for DbReader<T> {
    #[tracing::instrument(skip(self))]
    async fn txout_set(&self) -> Result<TxOutSetAccumulator, ChainStoreError> {
        let _timer = ReadTimer::start("txout_set");
        let accumulator = self
            .get_tx_out_set_info_accumulator()
            .await
            .map_err(chain_store_error)?;

        // The stored row *is* the domain value; only its encoding is this
        // backend's. The commitment it carries is defined by
        // `zaino_chain_store::txout_set`, which is also what maintains it — so
        // this hands the value over rather than restating it.
        Ok(accumulator.into_business())
    }
}

impl<T: ChainStoreSource> StoredBlockRead for DbReader<T> {
    #[tracing::instrument(skip(self), fields(start = %start, end = %end))]
    async fn blocks_chunk(
        &self,
        start: DomainHeight,
        end: DomainHeight,
    ) -> Result<Vec<StoredBlock>, ChainStoreError> {
        let Some((start, end)) = self.clamped_range(start, end)? else {
            return Ok(Vec::new());
        };

        // Timed around the chunk rather than the block: one read transaction
        // covers the range, so a per-block figure would divide one duration by
        // a count rather than measure anything.
        let _timer = ReadTimer::start("blocks_chunk");

        self.get_chain_block_range(start, end)
            .await
            .map_err(chain_store_error)?
            .into_iter()
            .map(stored_block)
            .collect()
    }

    async fn blocks_stream(
        &self,
        start: DomainHeight,
        end: DomainHeight,
    ) -> Result<
        impl futures::Stream<Item = Result<Vec<StoredBlock>, ChainStoreError>> + Send + use<T>,
        ChainStoreError,
    > {
        let reader = self.clone();
        let range = self.clamped_range(start, end)?;
        Ok(chunked(range, self.blocks_per_chunk(), move |from, to| {
            let reader = reader.clone();
            async move {
                reader
                    .get_chain_block_range(from, to)
                    .await
                    .map_err(chain_store_error)?
                    .into_iter()
                    .map(stored_block)
                    .collect()
            }
        }))
    }
}

impl<T: ChainStoreSource> CompactBlockRead for DbReader<T> {
    #[tracing::instrument(skip(self, pools), fields(start = %start, end = %end))]
    async fn compact_chunk(
        &self,
        start: DomainHeight,
        end: DomainHeight,
        pools: PoolFilter,
    ) -> Result<Vec<CompactBlock>, ChainStoreError> {
        let Some((start, end)) = self.clamped_range(start, end)? else {
            return Ok(Vec::new());
        };

        // The wallet-sync hot path: a syncing wallet spends almost all of its
        // time here, so this is the read whose latency a dashboard needs.
        let _timer = ReadTimer::start("compact_chunk");

        DbReader::get_compact_block_range(self, start, end, pools)
            .await
            .map_err(chain_store_error)
    }

    async fn compact_stream(
        &self,
        start: DomainHeight,
        end: DomainHeight,
        pools: PoolFilter,
    ) -> Result<
        impl futures::Stream<Item = Result<Vec<CompactBlock>, ChainStoreError>> + Send + use<T>,
        ChainStoreError,
    > {
        let reader = self.clone();
        let range = self.clamped_range(start, end)?;
        Ok(chunked(range, self.blocks_per_chunk(), move |from, to| {
            let reader = reader.clone();
            async move {
                DbReader::get_compact_block_range(&reader, from, to, pools)
                    .await
                    .map_err(chain_store_error)
            }
        }))
    }
}

impl<T: ChainStoreSource> ChainStoreService for FinalisedState<T> {
    type Reader = DbReader<T>;

    fn reader(&self) -> Self::Reader {
        FinalisedState::reader(self)
    }

    fn status(&self) -> StatusType {
        FinalisedState::status(self)
    }

    fn subscribe_watermark(&self) -> tokio::sync::watch::Receiver<StoreWatermark> {
        self.subscribe_watermark()
    }
}

impl<T: ChainStoreSource> ChainStoreIngest for FinalisedState<T> {
    async fn build_to(&self, target: DomainHeight) -> Result<(), ChainStoreSourceError> {
        FinalisedState::build_to(self, stored_height(target))
            .await
            .map_err(chain_store_source_error)
    }

    async fn rewind_to(&self, height: DomainHeight) -> Result<(), ChainStoreError> {
        FinalisedState::rewind_to(self, stored_height(height))
            .await
            .map_err(chain_store_error)
    }

    fn wait_until_built(&self) -> impl Future<Output = ()> + Send {
        self.wait_until_synced()
    }

    async fn shutdown(&self) -> Result<(), ChainStoreError> {
        FinalisedState::shutdown(self)
            .await
            .map_err(chain_store_error)
    }
}

impl<T: ChainStoreSource> ChainStoreFreezeSink for FinalisedState<T> {
    /// Writes blocks the composer has already seen fall beyond reorg.
    ///
    /// Idempotent on `(height, hash)` by delegation: the writer's put is a
    /// byte-compare on conflict, so re-seeing a block it already holds is a
    /// no-op and re-seeing a *different* block at the same height is an error
    /// rather than a silent overwrite. That is the property the freeze stream
    /// needs, because it can deliver the same heights twice across a reorg.
    ///
    /// Blocks below the store's tip are skipped rather than rejected. The
    /// stream has a retention window in which a block is both emitted and still
    /// held by the chain head, so a store that built past it through its own
    /// source will legitimately be handed blocks it already has.
    ///
    /// A gap is not repaired here. The writer is append-only and contiguous, so
    /// a block above `tip + 1` cannot be written; it is left for the
    /// source-driven build path, which is why that path cannot be removed.
    async fn freeze(&self, blocks: &[StoredBlock]) -> Result<(), ChainStoreError> {
        for block in blocks {
            let expected = match self.db_height().await.map_err(chain_store_error)? {
                Some(tip) => tip.0.saturating_add(1),
                None => crate::types::GENESIS_HEIGHT.0,
            };

            let height = u32::from(block.header.height);
            if height < expected {
                continue;
            }
            if height > expected {
                break;
            }

            self.write_block(indexed_block_from_stored(block)?)
                .await
                .map_err(chain_store_error)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every port this backend claims to implement is actually implemented.
    ///
    /// A bound check, not a behaviour test, and it earns its place: nothing else
    /// notices when a port is declared in the domain crate and never satisfied
    /// here. The failure it catches is a consumer being unable to name a
    /// capability the store advertises — which shows up at wiring time in
    /// another crate, a long way from the cause.
    #[test]
    fn the_backend_satisfies_every_port_it_advertises() {
        fn reader<R: ChainStoreReader + StoredBlockRead + CompactBlockRead>() {}
        fn indexes<R: TransactionIndex + SpentOutputIndex + TxOutSetIndex>() {}
        fn service<S: ChainStoreService + ChainStoreIngest + ChainStoreFreezeSink>() {}
        #[cfg(feature = "transparent_address_history_experimental")]
        fn history<R: zaino_chain_store::TransparentHistoryIndex>() {}

        type Validator = zaino_source_zebra::ZebraValidator;

        reader::<DbReader<Validator>>();
        indexes::<DbReader<Validator>>();
        service::<FinalisedState<Validator>>();
        #[cfg(feature = "transparent_address_history_experimental")]
        history::<DbReader<Validator>>();
    }

    /// What a `chunked` test's reader was asked for: one `(from, to)` per read,
    /// in the order the reads happened.
    type ReadLog = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>;

    /// Records one read.
    fn record(log: &ReadLog, from: Height, to: Height) {
        let mut reads = log.lock().expect("the read log is not poisoned");
        reads.push((from.0, to.0));
    }

    /// Every read recorded so far.
    fn recorded(log: &ReadLog) -> Vec<(u32, u32)> {
        log.lock().expect("the read log is not poisoned").clone()
    }

    /// The heights `from..=to`, which is what a read over them answers here.
    fn heights_in(from: Height, to: Height) -> Vec<u32> {
        (from.0..=to.0).collect()
    }

    /// A `chunked` stream over `range` whose reads are recorded in `log`.
    fn recording_stream(
        range: Option<(Height, Height)>,
        blocks_per_chunk: u32,
        log: &ReadLog,
    ) -> impl futures::Stream<Item = Result<Vec<u32>, ChainStoreError>> + Send {
        let log = std::sync::Arc::clone(log);
        chunked(range, blocks_per_chunk, move |from, to| {
            record(&log, from, to);
            std::future::ready(Ok::<_, ChainStoreError>(heights_in(from, to)))
        })
    }

    /// A chunked read covers its range in chunks of the size it was given, the
    /// last one short, and hands over every height once, in order.
    #[tokio::test]
    async fn a_chunked_read_covers_its_range_in_chunks_of_the_given_size() {
        let log = ReadLog::default();
        let stream = recording_stream(
            Some((Height(0), Height(2500))),
            BLOCKS_PER_READ_TRANSACTION,
            &log,
        );

        let chunks: Vec<_> = futures::StreamExt::collect(stream).await;
        let heights: Vec<u32> = chunks
            .into_iter()
            .flat_map(|chunk| chunk.expect("no read fails"))
            .collect();

        assert_eq!(heights, (0..=2500).collect::<Vec<u32>>());
        let expected_reads = vec![(0, 1023), (1024, 2047), (2048, 2500)];
        assert_eq!(recorded(&log), expected_reads);
    }

    /// A chunk is read when the consumer asks for it and not before, so a
    /// one-block chunk is in the consumer's hands after one read, however long
    /// the range behind it. This is the property a passthrough store's first
    /// block depends on.
    #[tokio::test]
    async fn a_one_block_chunk_is_handed_over_before_the_next_is_read() {
        let log = ReadLog::default();
        let stream = recording_stream(
            Some((Height(5), Height(9))),
            BLOCKS_PER_PASSTHROUGH_READ,
            &log,
        );
        let mut stream = std::pin::pin!(stream);

        let first = futures::StreamExt::next(&mut stream).await;
        let first = first.map(|chunk| chunk.expect("no read fails"));
        assert_eq!(first, Some(vec![5]));
        assert_eq!(recorded(&log), vec![(5, 5)]);

        while futures::StreamExt::next(&mut stream).await.is_some() {}
        let expected_reads = vec![(5, 5), (6, 6), (7, 7), (8, 8), (9, 9)];
        assert_eq!(recorded(&log), expected_reads);
    }

    /// A chunk size of zero is read as one block, not as a chunk that never
    /// moves the cursor.
    #[tokio::test]
    async fn a_zero_block_chunk_is_read_as_one_block() {
        let log = ReadLog::default();
        let stream = recording_stream(Some((Height(0), Height(2))), 0, &log);

        let chunks: Vec<_> = futures::StreamExt::collect(stream).await;

        assert_eq!(chunks.len(), 3);
        assert_eq!(recorded(&log), vec![(0, 0), (1, 1), (2, 2)]);
    }
}
