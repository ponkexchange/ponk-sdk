//! Chains, venues, ids and links for ponk agents on EVM chains.
//!
//! Ponk agents run on Solana and on five EVM chains. This module holds the
//! constants and the pure helpers the client needs for the EVM side; nothing
//! in it sends a request.
//!
//! ```
//! let id = ponk::evm::pool_id("bsc", "0x92b7807bF19b7DDdf89b706143896d05228f3121").unwrap();
//! assert_eq!(id, "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121");
//! ```
//!
//! Ids are CAIP strings, lowercase hex, exactly as the API writes them:
//!
//! * pool or token: `eip155:<chainId>:0x<40 hex>` (the gas coin is the zero
//!   address);
//! * Uniswap v4 or PancakeSwap Infinity CL pool:
//!   `eip155:<chainId>:0x<pool manager>:0x<64 hex pool id>`;
//! * position NFT: `eip155:<chainId>:0x<position manager>:<decimal token id>`;
//! * v2 position: `eip155:<chainId>:0x<pair>:0x<owner>`.
//!
//! Units, everywhere on the EVM side: wei amounts are decimal strings (they
//! do not fit a float), USD amounts are decimal strings, and a figure the
//! server could not read is `None`, never 0.

use std::fmt;

/// Every chain word the API uses, Solana first.
pub const CHAINS: [&str; 6] = ["solana", "ethereum", "arbitrum", "base", "bsc", "robinhood"];

/// The five EVM chains and their EIP-155 chain ids.
pub const EVM_CHAINS: [(&str, u64); 5] = [
    ("ethereum", 1),
    ("arbitrum", 42161),
    ("base", 8453),
    ("bsc", 56),
    ("robinhood", 4663),
];

/// Each EVM venue id (the `dex` of an agent, a pool or a position) and the
/// chains it runs on.
pub const EVM_VENUES: [(&str, &[&str]); 8] = [
    ("uniswap_v2", &["arbitrum", "robinhood", "ethereum"]),
    ("uniswap_v3", &["arbitrum", "robinhood", "ethereum"]),
    ("uniswap_v4", &["arbitrum", "robinhood", "ethereum"]),
    ("pancakeswap_v2", &["bsc"]),
    ("pancakeswap_v3", &["bsc"]),
    ("pancakeswap_infinity_cl", &["bsc"]),
    ("aerodrome_classic", &["base"]),
    ("aerodrome_slipstream", &["base"]),
];

/// Venues where a position is a share of the whole pool: no range, and the
/// agent compounds and holds (`fee_collector`, or `monitor_only` to watch).
pub const FULL_RANGE_VENUES: [&str; 3] = ["uniswap_v2", "pancakeswap_v2", "aerodrome_classic"];

/// Venues where a position can be staked for farm rewards by choice
/// (`config.evm.stake`), and the token the farm pays.
pub const STAKE_VENUES: [(&str, &str); 3] = [
    ("pancakeswap_v3", "CAKE"),
    ("aerodrome_slipstream", "AERO"),
    ("aerodrome_classic", "AERO"),
];

/// The strategy ids a range venue takes. Degen Mode (`entry_agent`) is not
/// offered on an EVM venue.
pub const RANGE_STRATEGIES: [&str; 5] = [
    "adaptive_range_recovery",
    "bin_rebalancer",
    "conservative_bin_rebalancer",
    "volatility_aware_bin_rebalancer",
    "custom",
];

/// The liquidity shapes an EVM agent takes (`knobs.liquidity_shape`, or the
/// top-level `liquidity_shape` on `adaptive_range_recovery`): `spot` is one
/// even range (the default), `curve` is heaviest at the price, `bid_ask`
/// heaviest at the edges and `spot_bid_ask` half Spot and half Bid-Ask.
pub const SHAPES: [&str; 4] = ["spot", "curve", "bid_ask", "spot_bid_ask"];

/// The shapes the EVM copilot's sentence grammar names.
pub const PLAN_SHAPES: [&str; 3] = ["spot", "curve", "bid_ask"];

/// The two sides a one-sided range sits on.
pub const ONE_SIDED: [&str; 2] = ["below", "above"];

/// Where a shape other than Spot opens as several ranges managed as one
/// position, venue to chains. On every other range venue a shape is kept on
/// the agent and the position opens as one range.
pub const SHAPE_VENUES: [(&str, &[&str]); 5] = [
    ("uniswap_v3", &["arbitrum", "ethereum", "robinhood"]),
    ("uniswap_v4", &["arbitrum", "ethereum"]),
    ("pancakeswap_v3", &["bsc"]),
    ("pancakeswap_infinity_cl", &["bsc"]),
    ("aerodrome_slipstream", &["base"]),
];

/// How many ranges a shape opens as on a venue in [`SHAPE_VENUES`]
/// (`config.evm.shape_ranges`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeRanges {
    /// Chosen from live gas at the open, so that opening and later closing
    /// stays within 3% of the deposit. The default with a shape.
    Auto,
    /// A fixed count, 1 to 7.
    Fixed(u8),
}

impl ShapeRanges {
    /// The value the API reads: `"auto"` or the count.
    pub fn to_json(self) -> Result<serde_json::Value, EvmError> {
        match self {
            ShapeRanges::Auto => Ok(serde_json::json!("auto")),
            ShapeRanges::Fixed(n) if (1..=7).contains(&n) => Ok(serde_json::json!(n)),
            ShapeRanges::Fixed(n) => Err(EvmError(format!(
                "shape_ranges is \"auto\" or a whole number from 1 to 7, got {n}"
            ))),
        }
    }
}

/// The site the fund-and-sign pages live on.
pub const SITE: &str = "https://ponk.exchange";

/// Agents per fund-and-sign link, the API's own grouping.
pub const IDS_PER_FUND_LINK: usize = 40;

const ZERO: &str = "0x0000000000000000000000000000000000000000";

/// An argument this module refused before anything was built or sent. The
/// message is a plain sentence naming the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmError(pub String);

impl fmt::Display for EvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EvmError {}

/// True for one of the five EVM chain words.
pub fn is_evm_chain(chain: &str) -> bool {
    EVM_CHAINS.iter().any(|(c, _)| *c == chain)
}

/// The EIP-155 chain id of an EVM chain word.
pub fn chain_id(chain: &str) -> Option<u64> {
    EVM_CHAINS.iter().find(|(c, _)| *c == chain).map(|(_, id)| *id)
}

/// The chain word for an EIP-155 chain id ponk runs on.
pub fn chain_for_id(id: u64) -> Option<&'static str> {
    EVM_CHAINS.iter().find(|(_, i)| *i == id).map(|(c, _)| *c)
}

/// The gas coin an agent on a chain is funded in: `SOL`, `BNB` on BNB Chain,
/// `ETH` on the other four.
pub fn native_symbol(chain: &str) -> Option<&'static str> {
    match chain {
        "solana" => Some("SOL"),
        "bsc" => Some("BNB"),
        c if is_evm_chain(c) => Some("ETH"),
        _ => None,
    }
}

/// The chains an EVM venue runs on. Empty for a venue this crate does not know.
pub fn venue_chains(dex: &str) -> &'static [&'static str] {
    EVM_VENUES.iter().find(|(v, _)| *v == dex).map(|(_, c)| *c).unwrap_or(&[])
}

/// True where a position has no range (v2 style pools).
pub fn is_full_range_venue(dex: &str) -> bool {
    FULL_RANGE_VENUES.contains(&dex)
}

/// `CAKE` or `AERO` where an agent can stake its position, else `None`.
pub fn stake_reward_token(dex: &str) -> Option<&'static str> {
    STAKE_VENUES.iter().find(|(v, _)| *v == dex).map(|(_, t)| *t)
}

/// True when a shape other than Spot opens as several ranges on this venue
/// and chain ([`SHAPE_VENUES`]).
pub fn is_shape_venue(dex: &str, chain: &str) -> bool {
    SHAPE_VENUES.iter().any(|(v, chains)| *v == dex && chains.contains(&chain))
}

fn is_hex(s: &str, digits: usize) -> bool {
    s.len() == digits + 2 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn addr(value: &str, what: &str) -> Result<String, EvmError> {
    if is_hex(value, 40) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(EvmError(format!("{what} must be 0x followed by 40 hex characters, got '{value}'")))
    }
}

fn cid(chain: &str) -> Result<u64, EvmError> {
    chain_id(chain).ok_or_else(|| {
        EvmError(format!(
            "'{chain}' is not an EVM chain; the chains are ethereum, arbitrum, base, bsc, robinhood"
        ))
    })
}

/// `eip155:<chainId>:0x<address>`: a v2 or v3 style pool, lowercase.
pub fn pool_id(chain: &str, address: &str) -> Result<String, EvmError> {
    Ok(format!("eip155:{}:{}", cid(chain)?, addr(address, "the pool address")?))
}

/// `eip155:<chainId>:0x<address>`: a token. The gas coin is the zero address.
pub fn token_id(chain: &str, address: &str) -> Result<String, EvmError> {
    Ok(format!("eip155:{}:{}", cid(chain)?, addr(address, "the token address")?))
}

/// The id of a chain's gas coin (ETH, or BNB on BNB Chain).
pub fn native_token_id(chain: &str) -> Result<String, EvmError> {
    Ok(format!("eip155:{}:{ZERO}", cid(chain)?))
}

/// `eip155:<chainId>:0x<pool manager>:0x<64 hex>`: the form of a Uniswap v4
/// or PancakeSwap Infinity CL pool, which lives inside its venue's pool
/// manager and is named by a 32 byte pool id.
pub fn v4_pool_id(chain: &str, pool_manager: &str, pool_key_hash: &str) -> Result<String, EvmError> {
    if !is_hex(pool_key_hash, 64) {
        return Err(EvmError(format!(
            "the pool id must be 0x followed by 64 hex characters, got '{pool_key_hash}'"
        )));
    }
    Ok(format!(
        "eip155:{}:{}:{}",
        cid(chain)?,
        addr(pool_manager, "the pool manager")?,
        pool_key_hash.to_ascii_lowercase()
    ))
}

/// `eip155:<chainId>:0x<position manager>:<decimal token id>`.
pub fn position_nft_id(chain: &str, position_manager: &str, nft_token_id: u128) -> Result<String, EvmError> {
    Ok(format!(
        "eip155:{}:{}:{nft_token_id}",
        cid(chain)?,
        addr(position_manager, "the position manager")?
    ))
}

/// What the part after the first address of a CAIP id names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmIdKind {
    /// A pool or a token: `eip155:<chainId>:0x<address>`.
    Address,
    /// A Uniswap v4 or PancakeSwap Infinity CL pool, with its 32 byte pool id.
    V4Pool { pool_key_hash: String },
    /// A position NFT, with its token id in decimal.
    PositionNft { nft_token_id: String },
    /// A v2 position: the pair, and the owner's address.
    V2Position { owner: String },
}

/// One parsed CAIP id. `address` is the first address of the id: the pool or
/// token, the pool manager, the position manager or the pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmId {
    pub chain: &'static str,
    pub chain_id: u64,
    pub address: String,
    pub kind: EvmIdKind,
}

impl EvmId {
    /// True for the gas coin's token id (the zero address).
    pub fn is_native(&self) -> bool {
        self.kind == EvmIdKind::Address && self.address == ZERO
    }
}

/// Read a CAIP id the API sent.
///
/// A chain id ponk does not run on is refused, so a Solana address or an id
/// for another network can never be mistaken for one of these.
pub fn parse_evm_id(value: &str) -> Result<EvmId, EvmError> {
    let not_an_id = || EvmError(format!("'{value}' is not an eip155 id"));
    let parts: Vec<&str> = value.split(':').collect();
    if !(parts.len() == 3 || parts.len() == 4) || parts[0] != "eip155" {
        return Err(not_an_id());
    }
    if parts[1].is_empty() || !parts[1].bytes().all(|b| b.is_ascii_digit()) {
        return Err(not_an_id());
    }
    let id: u64 = parts[1].parse().map_err(|_| not_an_id())?;
    let chain = chain_for_id(id)
        .ok_or_else(|| EvmError(format!("'{value}' names chain id {id}, which ponk does not run on")))?;
    let address = addr(parts[2], "the address in the id")?;
    let kind = match parts.get(3) {
        None => EvmIdKind::Address,
        Some(tail) if is_hex(tail, 64) => EvmIdKind::V4Pool { pool_key_hash: tail.to_ascii_lowercase() },
        Some(tail) if is_hex(tail, 40) => EvmIdKind::V2Position { owner: tail.to_ascii_lowercase() },
        Some(tail) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
            EvmIdKind::PositionNft { nft_token_id: (*tail).to_string() }
        }
        Some(_) => return Err(not_an_id()),
    };
    Ok(EvmId { chain, chain_id: id, address, kind })
}

/// The chain an id lives on: the EVM chain word for a CAIP id, else `solana`.
pub fn chain_of(value: &str) -> &'static str {
    parse_evm_id(value).map(|id| id.chain).unwrap_or("solana")
}

/// A wei string as whole coins (ETH or BNB), exactly, as a decimal string:
/// `"452200000000000"` is `"0.0004522"`. `None` when `wei` is not a whole
/// number.
pub fn wei_to_native(wei: &str) -> Option<String> {
    if wei.is_empty() || !wei.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = wei.trim_start_matches('0');
    let padded = format!("{digits:0>19}");
    let (whole, frac) = padded.split_at(padded.len() - 18);
    let frac = frac.trim_end_matches('0');
    Some(if frac.is_empty() { whole.to_string() } else { format!("{whole}.{frac}") })
}

// -- configs ------------------------------------------------------------------

fn ticks(fraction: f64) -> u32 {
    // One tick is a price step of 1.0001, on every concentrated EVM venue.
    ((1.0 + fraction).ln() / 1.0001f64.ln()).round().max(1.0) as u32
}

/// The config of a range agent on a concentrated EVM venue.
///
/// `down_pct` and `up_pct` are how far the range reaches below and above the
/// price, as FRACTIONS: 0.05 is 5%. The keys are the ones the API reads,
/// `range_pct: {down_pct, up_pct}`; `down` and `up` are refused. A ONE-SIDED
/// range is one side at 0: `up_pct` 0 puts the whole band below the price,
/// `down_pct` 0 above it. Both at 0 is refused.
///
/// Built for the bin rebalancer family (`bin_rebalancer`, and
/// `conservative_bin_rebalancer`, which takes `min_out_of_range_secs` too:
/// add it to the result). The range goes in as the edge percentages
/// (`knobs.range_pct`), which is what the agent opens on, and as the same
/// span in ticks (`target_bin_width`), which the engine reads only when the
/// percentages are absent. For any other strategy write the config yourself;
/// on `adaptive_range_recovery` the range is a top-level `range_pct`.
///
/// `stake` of `Some(true)` stakes the position for CAKE or AERO
/// (`config.evm.stake`), on the venues in [`STAKE_VENUES`] only.
pub fn range_config(
    down_pct: f64,
    up_pct: f64,
    strategy: &str,
    stake: Option<bool>,
) -> Result<serde_json::Value, EvmError> {
    for (name, value) in [("down_pct", down_pct), ("up_pct", up_pct)] {
        if !(value.is_finite() && (0.0..1.0).contains(&value)) {
            return Err(EvmError(format!(
                "{name} is a fraction from 0 to below 1 (0.05 is 5%), got {value}"
            )));
        }
    }
    if down_pct == 0.0 && up_pct == 0.0 {
        return Err(EvmError(
            "down_pct and up_pct are both 0: give the range a width (one side at 0 is a one-sided range)".into(),
        ));
    }
    let side = |f: f64| if f > 0.0 { ticks(f) } else { 0 };
    let mut config = serde_json::json!({
        "kind": strategy,
        "target_bin_width": side(down_pct) + side(up_pct) + 1,
        "auto_compound": true,
        "knobs": { "range_pct": { "down_pct": down_pct, "up_pct": up_pct } },
    });
    if let Some(s) = stake {
        config["evm"] = serde_json::json!({ "stake": s });
    }
    Ok(config)
}

/// The config of a compound and hold agent on a full-range venue
/// (`uniswap_v2`, `pancakeswap_v2`, `aerodrome_classic`), with strategy
/// `fee_collector`. `stake` is for `aerodrome_classic` only.
pub fn full_range_config(stake: Option<bool>) -> serde_json::Value {
    let mut config = serde_json::json!({
        "kind": "fee_collector",
        "min_total_pending_base_units": 0,
        "auto_compound": true,
    });
    if let Some(s) = stake {
        config["evm"] = serde_json::json!({ "stake": s });
    }
    config
}

/// `config` with its liquidity shape set, as the API reads it.
///
/// `shape` is one of [`SHAPES`]. It goes where the strategy keeps it: top
/// level `liquidity_shape` on `adaptive_range_recovery`,
/// `knobs.liquidity_shape` on every other strategy (`strategy`, else the
/// config's `kind`). `ranges` is written to `config.evm.shape_ranges`: on a
/// venue in [`SHAPE_VENUES`] a shape other than Spot opens as that many
/// ranges managed as one position; the server lays them out. A full-range
/// venue takes no shape. Per-side shapes are the config keys `shape_below`,
/// `shape_above` (`spot`, `curve` or `bid_ask`) and `ratio_below_pct` (1 to
/// 99): set them in your own config.
pub fn with_shape(
    mut config: serde_json::Value,
    shape: Option<&str>,
    ranges: Option<ShapeRanges>,
    strategy: Option<&str>,
) -> Result<serde_json::Value, EvmError> {
    if !config.is_object() {
        config = serde_json::json!({});
    }
    if let Some(shape) = shape {
        if !SHAPES.contains(&shape) {
            return Err(EvmError(format!(
                "shape is one of {}, got '{shape}'",
                SHAPES.join(", ")
            )));
        }
        let kind = strategy
            .map(str::to_string)
            .or_else(|| config.get("kind").and_then(|k| k.as_str()).map(str::to_string));
        if kind.as_deref() == Some("adaptive_range_recovery") {
            config["liquidity_shape"] = shape.into();
        } else {
            if !config.get("knobs").is_some_and(|k| k.is_object()) {
                config["knobs"] = serde_json::json!({});
            }
            config["knobs"]["liquidity_shape"] = shape.into();
        }
    }
    if let Some(r) = ranges {
        let v = r.to_json()?;
        if !config.get("evm").is_some_and(|e| e.is_object()) {
            config["evm"] = serde_json::json!({});
        }
        config["evm"]["shape_ranges"] = v;
    }
    Ok(config)
}

/// `{"evm": {"stake": bool}}`, the patch `update_config` takes on a farm venue.
pub fn stake_config(stake: bool) -> serde_json::Value {
    serde_json::json!({ "evm": { "stake": stake } })
}

// -- the fund-and-sign link ---------------------------------------------------

/// One agent of a fund-and-sign link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FundLinkAgent {
    pub id: String,
    /// SOL to send to a Solana agent, in lamports. `None` or 0: sign the
    /// approval only. Ignored on an EVM chain.
    pub lamports: Option<u64>,
    /// The agent's chain. `None` is Solana.
    pub chain: Option<String>,
}

impl FundLinkAgent {
    /// An agent to sign for, with nothing to send.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), lamports: None, chain: None }
    }

    pub fn lamports(mut self, lamports: u64) -> Self {
        self.lamports = Some(lamports);
        self
    }

    pub fn chain(mut self, chain: impl Into<String>) -> Self {
        self.chain = Some(chain.into());
        self
    }
}

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

fn amount(agent: &FundLinkAgent) -> String {
    // The amount field is lamports. An EVM agent is funded in its chain's own
    // coin, sent by the owner to the agent's address there, so it is listed
    // with 0, which means "sign the approval only".
    if agent.chain.as_deref().is_some_and(is_evm_chain) {
        return "0".into();
    }
    match agent.lamports {
        Some(l) if l > 0 && l.to_string().len() <= 16 => l.to_string(),
        _ => "0".into(),
    }
}

/// The ponk.exchange pages where the owner funds agents and signs for them.
///
/// ```text
/// https://ponk.exchange/agents/fund?a=<agent id>:<lamports>,<agent id>:<lamports>
/// ```
///
/// `lamports` is the SOL to send to a Solana agent; absent or 0 means sign
/// the approval only. An agent on an EVM chain is always listed with 0: it is
/// funded in its chain's own coin (BNB on BNB Chain, ETH on the others),
/// which the owner sends to the agent's address on that chain, and its
/// approval is two signatures on the page (the Solana login wallet, then the
/// EVM wallet that will receive withdrawals).
///
/// At most 40 agents go in one link, so a longer list returns several. An id
/// that is not a UUID is left out and a repeated one is listed once; with
/// nothing usable the result is empty. `top_up` adds `&mode=top_up`, so the
/// page sends only what each wallet is still short of and opening the link
/// twice never sends the same SOL twice.
///
/// NOTHING IS SIGNED HERE. A link names agent ids only; the page resolves
/// every destination for the signed-in account, so a link cannot name an
/// address. This crate holds no key and offers no signer.
pub fn fund_links(agents: &[FundLinkAgent], top_up: bool) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut parts: Vec<(String, String)> = Vec::new();
    for agent in agents {
        let id = agent.id.trim().to_ascii_lowercase();
        if !is_uuid(&id) || seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        parts.push((id, amount(agent)));
    }
    parts
        .chunks(IDS_PER_FUND_LINK)
        .map(|chunk| {
            let funded = chunk.iter().any(|(_, a)| a != "0");
            let list: Vec<String> = chunk.iter().map(|(i, a)| format!("{i}:{a}")).collect();
            format!(
                "{SITE}/agents/fund?a={}{}",
                list.join(","),
                if top_up && funded { "&mode=top_up" } else { "" }
            )
        })
        .collect()
}

/// One agent's fund-and-sign link, to sign its approval. See [`fund_links`].
/// `None` when `agent_id` is not an agent id, rather than a link that opens
/// on nothing.
pub fn fund_link(agent_id: &str, chain: Option<&str>) -> Option<String> {
    let agent = FundLinkAgent { id: agent_id.to_string(), lamports: None, chain: chain.map(str::to_string) };
    fund_links(&[agent], false).into_iter().next()
}

/// An agent's own page. For an EVM agent it shows the deposit address and
/// takes the two approval signatures.
pub fn agent_page_url(agent_id: &str) -> String {
    format!("{SITE}/agents/{}", agent_id.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POOL: &str = "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121";
    const AGENT: &str = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
    const AGENT2: &str = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2c";

    #[test]
    fn a_pool_id_is_lowercase_caip() {
        assert_eq!(pool_id("bsc", "0x92b7807bF19b7DDdf89b706143896d05228f3121").unwrap(), POOL);
        assert_eq!(
            token_id("base", "0x4200000000000000000000000000000000000006").unwrap(),
            "eip155:8453:0x4200000000000000000000000000000000000006"
        );
        assert_eq!(
            native_token_id("robinhood").unwrap(),
            "eip155:4663:0x0000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn the_chain_ids_are_the_apis() {
        let ids: Vec<u64> = ["ethereum", "arbitrum", "base", "bsc", "robinhood"]
            .iter()
            .map(|c| chain_id(c).unwrap())
            .collect();
        assert_eq!(ids, vec![1, 42161, 8453, 56, 4663]);
        assert_eq!(chain_id("solana"), None);
        assert_eq!(CHAINS[0], "solana");
        assert_eq!(native_symbol("bsc"), Some("BNB"));
        assert_eq!(native_symbol("robinhood"), Some("ETH"));
        assert_eq!(native_symbol("solana"), Some("SOL"));
        assert_eq!(native_symbol("polygon"), None);
    }

    #[test]
    fn the_v4_form_carries_the_manager_and_the_pool_id() {
        let manager = "0x000000000004444c5dc75cB358380D2e3dE08A90";
        let key = format!("0x{}", "E6".repeat(32));
        let id = v4_pool_id("ethereum", manager, &key).unwrap();
        assert_eq!(id, format!("eip155:1:{}:{}", manager.to_lowercase(), key.to_lowercase()));
        let parsed = parse_evm_id(&id).unwrap();
        assert_eq!((parsed.chain, parsed.chain_id), ("ethereum", 1));
        assert_eq!(parsed.address, manager.to_lowercase());
        assert_eq!(parsed.kind, EvmIdKind::V4Pool { pool_key_hash: key.to_lowercase() });
    }

    #[test]
    fn every_id_form_parses() {
        let p = parse_evm_id(POOL).unwrap();
        assert_eq!((p.chain, &p.kind), ("bsc", &EvmIdKind::Address));
        assert!(!p.is_native());
        assert!(parse_evm_id(&native_token_id("bsc").unwrap()).unwrap().is_native());
        let nft = position_nft_id("bsc", "0x46A15B0b27311cedF172AB29E4f4766fbE7F4364", 812_345).unwrap();
        assert_eq!(
            parse_evm_id(&nft).unwrap().kind,
            EvmIdKind::PositionNft { nft_token_id: "812345".into() }
        );
        let v2 = parse_evm_id(
            "eip155:8453:0xcdac0d6c6c59727a65f871236188350531885c43:0x52908400098527886e0f7030069857d2e4169ee7",
        )
        .unwrap();
        assert_eq!(v2.chain, "base");
        assert_eq!(
            v2.kind,
            EvmIdKind::V2Position { owner: "0x52908400098527886e0f7030069857d2e4169ee7".into() }
        );
    }

    #[test]
    fn a_solana_address_or_a_foreign_chain_is_never_an_evm_id() {
        for bad in [
            "So11111111111111111111111111111111111111112",
            "eip155:137:0x92b7807bf19b7dddf89b706143896d05228f3121",
            "eip155:56:0x1234",
            "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121:nope",
            "eip155::0x92b7807bf19b7dddf89b706143896d05228f3121",
            "",
        ] {
            assert!(parse_evm_id(bad).is_err(), "{bad}");
        }
        assert_eq!(chain_of("So11111111111111111111111111111111111111112"), "solana");
        assert_eq!(chain_of(POOL), "bsc");
    }

    #[test]
    fn bad_inputs_are_refused_before_an_id_is_built() {
        assert!(pool_id("polygon", "0x92b7807bf19b7dddf89b706143896d05228f3121").is_err());
        assert!(pool_id("bsc", "92b7807bf19b7dddf89b706143896d05228f3121").is_err());
        assert!(v4_pool_id("ethereum", "0x000000000004444c5dc75cb358380d2e3de08a90", "0x12").is_err());
        let e = pool_id("polygon", "0x92b7807bf19b7dddf89b706143896d05228f3121").unwrap_err();
        assert!(e.to_string().contains("is not an EVM chain"), "{e}");
    }

    #[test]
    fn venues_know_their_chains_ranges_and_farms() {
        assert_eq!(venue_chains("pancakeswap_infinity_cl"), ["bsc"]);
        assert_eq!(venue_chains("uniswap_v4").len(), 3);
        assert_eq!(venue_chains("aerodrome_slipstream"), ["base"]);
        assert!(venue_chains("meteora_dlmm").is_empty());
        assert!(is_full_range_venue("aerodrome_classic"));
        assert!(!is_full_range_venue("uniswap_v3"));
        assert_eq!(stake_reward_token("pancakeswap_v3"), Some("CAKE"));
        assert_eq!(stake_reward_token("aerodrome_classic"), Some("AERO"));
        assert_eq!(stake_reward_token("pancakeswap_infinity_cl"), None);
        for (venue, chains) in EVM_VENUES {
            assert!(chains.iter().all(|c| is_evm_chain(c)), "{venue}");
        }
    }

    #[test]
    fn wei_reads_exactly() {
        assert_eq!(wei_to_native("452200000000000").as_deref(), Some("0.0004522"));
        assert_eq!(wei_to_native("150000000000000000").as_deref(), Some("0.15"));
        assert_eq!(wei_to_native("1000000000000000000").as_deref(), Some("1"));
        assert_eq!(wei_to_native("0").as_deref(), Some("0"));
        assert_eq!(
            wei_to_native("340282366920938463463374607431768211455").as_deref(),
            Some("340282366920938463463.374607431768211455")
        );
        assert_eq!(wei_to_native("1.5"), None);
        assert_eq!(wei_to_native(""), None);
    }

    #[test]
    fn the_tick_width_matches_the_servers_planner() {
        // The server's planner: round(ln(1 + f) / ln(1.0001)) * 2 + 1.
        let c = range_config(0.05, 0.05, "bin_rebalancer", Some(true)).unwrap();
        assert_eq!(
            c,
            serde_json::json!({
                "kind": "bin_rebalancer",
                "target_bin_width": 977,
                "auto_compound": true,
                "knobs": { "range_pct": { "down_pct": 0.05, "up_pct": 0.05 } },
                "evm": { "stake": true },
            })
        );
        assert_eq!(range_config(0.06, 0.06, "bin_rebalancer", None).unwrap()["target_bin_width"], 1167);
        let asym = range_config(0.03, 0.12, "conservative_bin_rebalancer", None).unwrap();
        assert_eq!(asym["knobs"]["range_pct"]["up_pct"], 0.12);
        assert!(asym.get("evm").is_none());
    }

    #[test]
    fn a_range_is_a_fraction_not_a_percent() {
        for bad in [5.0, -0.05, 1.0, f64::NAN, f64::INFINITY] {
            assert!(range_config(bad, 0.05, "bin_rebalancer", None).is_err(), "{bad}");
            assert!(range_config(0.05, bad, "bin_rebalancer", None).is_err(), "{bad}");
        }
        // One side at 0 is a one-sided range (0.16.0); both at 0 is no range.
        assert!(range_config(0.0, 0.0, "bin_rebalancer", None).is_err());
        let below = range_config(0.05, 0.0, "bin_rebalancer", None).unwrap();
        assert_eq!(below["knobs"]["range_pct"], serde_json::json!({ "down_pct": 0.05, "up_pct": 0.0 }));
        assert!(below["target_bin_width"].as_u64() < range_config(0.05, 0.05, "bin_rebalancer", None).unwrap()["target_bin_width"].as_u64());
    }

    #[test]
    fn a_shape_goes_where_the_strategy_keeps_it() {
        let c = with_shape(range_config(0.05, 0.05, "bin_rebalancer", Some(false)).unwrap(), Some("curve"), Some(ShapeRanges::Auto), None).unwrap();
        assert_eq!(c["knobs"]["liquidity_shape"], "curve");
        assert_eq!(c["evm"], serde_json::json!({ "stake": false, "shape_ranges": "auto" }));
        let a = with_shape(serde_json::json!({ "kind": "adaptive_range_recovery" }), Some("bid_ask"), Some(ShapeRanges::Fixed(3)), None).unwrap();
        assert_eq!((a["liquidity_shape"].as_str(), a.get("knobs")), (Some("bid_ask"), None));
        assert_eq!(a["evm"]["shape_ranges"], 3);
        let named = with_shape(serde_json::json!({}), Some("spot_bid_ask"), None, Some("adaptive_range_recovery")).unwrap();
        assert_eq!(named["liquidity_shape"], "spot_bid_ask");
        assert!(with_shape(serde_json::json!({}), Some("hybrid"), None, None).is_err());
        for n in [0u8, 8] {
            assert!(with_shape(serde_json::json!({}), None, Some(ShapeRanges::Fixed(n)), None).is_err());
        }
        assert!(is_shape_venue("uniswap_v3", "robinhood"));
        assert!(!is_shape_venue("uniswap_v4", "robinhood"));
        assert!(is_shape_venue("pancakeswap_infinity_cl", "bsc"));
        assert!(!is_shape_venue("aerodrome_classic", "base"));
        for (venue, chains) in SHAPE_VENUES {
            for chain in chains {
                assert!(venue_chains(venue).contains(chain), "{venue} {chain}");
            }
        }
    }

    #[test]
    fn the_full_range_and_stake_configs() {
        assert_eq!(
            full_range_config(Some(true)),
            serde_json::json!({
                "kind": "fee_collector", "min_total_pending_base_units": 0,
                "auto_compound": true, "evm": { "stake": true }
            })
        );
        assert!(full_range_config(None).get("evm").is_none());
        assert_eq!(stake_config(false), serde_json::json!({ "evm": { "stake": false } }));
    }

    #[test]
    fn an_evm_agent_is_listed_with_zero() {
        let want = format!("https://ponk.exchange/agents/fund?a={AGENT}:0");
        assert_eq!(fund_link(AGENT, Some("bsc")).as_deref(), Some(want.as_str()));
        // The amount field is lamports, so it never carries to an EVM agent.
        let agent = FundLinkAgent::new(AGENT).lamports(5_000_000).chain("base");
        assert_eq!(fund_links(&[agent], true), vec![want]);
    }

    #[test]
    fn a_solana_agent_carries_its_lamports() {
        let links = fund_links(&[FundLinkAgent::new(AGENT).lamports(150_000_000)], false);
        assert_eq!(links, vec![format!("https://ponk.exchange/agents/fund?a={AGENT}:150000000")]);
        for bad in [0u64, 10_000_000_000_000_000] {
            let links = fund_links(&[FundLinkAgent::new(AGENT).lamports(bad)], false);
            assert!(links[0].ends_with(":0"), "{bad}");
        }
    }

    #[test]
    fn ids_are_cleaned_and_listed_once() {
        let links = fund_links(
            &[
                FundLinkAgent::new(AGENT.to_uppercase()),
                FundLinkAgent::new(format!(" {AGENT}")),
                FundLinkAgent::new("not-an-id"),
                FundLinkAgent::new(AGENT2).chain("bsc"),
            ],
            false,
        );
        assert_eq!(links, vec![format!("https://ponk.exchange/agents/fund?a={AGENT}:0,{AGENT2}:0")]);
        assert!(fund_links(&[FundLinkAgent::new("nope")], false).is_empty());
        assert_eq!(fund_link("nope", None), None);
    }

    #[test]
    fn forty_agents_to_a_link() {
        let agents: Vec<FundLinkAgent> = (0..85)
            .map(|i| FundLinkAgent::new(format!("00000000-0000-7000-8000-{i:012x}")))
            .collect();
        let links = fund_links(&agents, false);
        let counts: Vec<usize> = links.iter().map(|l| l.matches(":0").count()).collect();
        assert_eq!(counts, vec![40, 40, 5]);
        for a in &agents {
            assert_eq!(links.iter().filter(|l| l.contains(&a.id)).count(), 1);
        }
    }

    #[test]
    fn top_up_is_added_only_when_something_is_funded() {
        let funded = fund_links(&[FundLinkAgent::new(AGENT).lamports(1000)], true);
        assert!(funded[0].ends_with(&format!("{AGENT}:1000&mode=top_up")));
        let unfunded = fund_links(&[FundLinkAgent::new(AGENT).lamports(1000).chain("bsc")], true);
        assert!(unfunded[0].ends_with(&format!("{AGENT}:0")));
    }

    #[test]
    fn the_agent_page() {
        assert_eq!(agent_page_url(&AGENT.to_uppercase()), format!("https://ponk.exchange/agents/{AGENT}"));
    }
}
