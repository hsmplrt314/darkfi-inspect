mod diagnose;
mod rpc;

use diagnose::{BlockInspection, CheckResult, CheckState, Confidence, DiagnosticSummary};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// The main RPC port and management RPC port, as fixed constants for now.
// Later these become configurable (network selection, custom endpoint).
const RPC_ENDPOINT: &str = "127.0.0.1:18345";
const MGMT_ENDPOINT: &str = "127.0.0.1:18346";

// This is the *specific* shape of p2p.get_info's result, matching
// exactly what we read in src/rpc/p2p_method.rs — a list of channels
// (each with url/session/id) and a list of outbound slot numbers.
#[derive(Deserialize, Serialize, Debug)]
struct P2pInfo {
    channels: Vec<Channel>,
    outbound_slots: Vec<u64>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Channel {
    url: String,
    session: String,
    id: u64,
}

// NodeStatus: synthesized health snapshot from 3 separate RPC calls
// (blockchain.last_confirmed_block, blockchain.best_fork_next_block_height,
// p2p.get_info) — this is the whole point of `status`: combine raw RPC
// replies into one useful picture instead of relaying them one at a time.
#[derive(Debug, Serialize)]
struct NodeStatus {
    last_confirmed_height: u32,
    last_confirmed_hash: String,
    best_fork_next_height: u32,
    // confirmation_depth: distance between best_fork_next_height and last_confirmed_height.
    // Confirmed via live observation: DarkFi consistently confirms blocks 5 deep —
    // this should read 5 on a healthy node. Anything else is the real anomaly to flag
    // (stuck confirmation logic, reorg in progress, or genuine sync lag).
    confirmation_depth: i64,
    peers_connected: usize,
    peers_slots: usize,
    // wall-clock time for the slowest of the 3 calls -is darkfid alive/responsive" signal
    rpc_latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct TxCallInspection {
    index: usize,
    contract_id: String,
    function_code: Option<u8>,
    data_length: usize,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Serialize)]
struct TxInspection {
    hash: String,
    calls: usize,
    proofs: usize,
    signatures: usize,
    call_details: Vec<TxCallInspection>,
    summary: DiagnosticSummary,
    checks: Vec<CheckResult>,
}

// #[derive(Parser)] turns this struct into the whole CLI definition.
// `about` becomes the text shown in `--help`.
#[derive(Parser)]
#[command(name = "darkfi-inspect", about = "Observability tool for DarkFi nodes")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Output raw JSON instead of a human-readable format
    #[arg(long, global = true)]
    json: bool,
}

// Each variant here becomes a subcommand, e.g. `darkfi-inspect ping`.
// This is where `status`, `peers`, `events` etc will all get added later.
#[derive(Subcommand)]
enum Command {
    /// Send a ping request to darkfid
    Ping,
    /// List connected peers
    Peers,
    /// Show node status (chain height, sync gap, peers, RPC latency)
    Status,
    /// Subscribe to live node events (blocks, txs, proposals)
    Events,
    /// Inspect a specific object by height or hash
    Inspect {
        #[command(subcommand)]
        target: InspectTarget,
    },
}

// Separate enum for what kind of thing we're inspecting — clap nests this
// as a subcommand of Inspect, e.g. `darkfi-inspect inspect block 43723`.
#[derive(Subcommand)]
enum InspectTarget {
    /// Inspect a block by height
    Block { height: u32 },
    /// Inspect a transaction by hash
    Tx { hash: String },
}

// #[tokio::main] turns `main` into an async function tokio can drive.
// Without this, `async`/`await` below wouldn't actually run anything.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parses real argv into our Cli struct. If the user runs something
    // invalid, clap prints a helpful error and exits — we never see it.
    let cli = Cli::parse();

    match cli.command {
        Command::Ping => cmd_ping(cli.json).await?,
        Command::Peers => cmd_peers(cli.json).await?,
        Command::Status => cmd_status(cli.json).await?,
        Command::Events => cmd_events().await?,
        Command::Inspect { target } => cmd_inspect(target, cli.json).await?,
    }

    Ok(())
}

async fn cmd_ping(json: bool) -> anyhow::Result<()> {
    let reply = rpc::call(RPC_ENDPOINT, "ping", Value::Array(vec![])).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&reply)?);
    } else {
        println!("ping -> {reply}");
    }
    Ok(())
}

async fn cmd_peers(json: bool) -> anyhow::Result<()> {
    let reply = rpc::call(MGMT_ENDPOINT, "p2p.get_info", Value::Array(vec![])).await?;
    let info: P2pInfo = serde_json::from_value(reply)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("{} peer(s) connected:", info.channels.len());
        for ch in &info.channels {
            println!(" [{}] {} ({})", ch.id, ch.url, ch.session);
        }
        println!(
            "{} outbound slot(s): {:?}",
            info.outbound_slots.len(),
            info.outbound_slots
        );
    }
    Ok(())
}

async fn cmd_status(json: bool) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    // blockchain.last_confirmed_block returns a raw 2-element array [height, hash],
    // so it deserializes straight into a tuple — no wrapper struct needed.
    let lcb_reply = rpc::call(
        RPC_ENDPOINT,
        "blockchain.last_confirmed_block",
        Value::Array(vec![]),
    )
    .await?;
    let (last_confirmed_height, last_confirmed_hash): (u32, String) =
        serde_json::from_value(lcb_reply)?;

    // best_fork_next_block_height returns a bare number, no wrapper either/
    let bfh_reply = rpc::call(
        RPC_ENDPOINT,
        "blockchain.best_fork_next_block_height",
        Value::Array(vec![]),
    )
    .await?;
    let best_fork_next_height: u32 = serde_json::from_value(bfh_reply)?;

    // Reuse the same p2p.get_info call/struct that `peers` already uses.
    let p2p_reply = rpc::call(MGMT_ENDPOINT, "p2p.get_info", Value::Array(vec![])).await?;
    let info: P2pInfo = serde_json::from_value(p2p_reply)?;

    let rpc_latency_ms = start.elapsed().as_millis();

    let status = NodeStatus {
        last_confirmed_height,
        last_confirmed_hash,
        best_fork_next_height,
        confirmation_depth: (best_fork_next_height as i64 - last_confirmed_height as i64) - 1,
        peers_connected: info.channels.len(),
        peers_slots: info.outbound_slots.len(),
        rpc_latency_ms,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!(
            "Last confirmed block: {} ({})",
            status.last_confirmed_height, status.last_confirmed_hash
        );
        println!("Best fork next height: {}", status.best_fork_next_height);
        println!("Confirmation depth: {}", status.confirmation_depth);
        println!(
            "Peers: {}/{} connected",
            status.peers_connected, status.peers_slots
        );
        println!("RPC latency: {} ms", status.rpc_latency_ms);
    }

    // Exit code groundwork for future scripting/monitoring use (cron, CI checks,
    // watch loops): 0 = healthy (confirmation_depth == 5), 1 = anomalous.
    // Not printed/decorated — a human reading the line above already sees the number;
    // this is purely for callers that don't have a human watching.
    if status.confirmation_depth != 5 {
        std::process::exit(1);
    }

    Ok(())
}

async fn cmd_events() -> anyhow::Result<()> {
    // Run all three subscriptions concurrently on independent connections.
    // tokio::join! polls all three futures together; since none of them
    // return under normal operation, this effectively runs forever until
    // one connection drops or errors — whichever happens first ends this
    // function, since we're propagating errors with `?` below.
    let (blocks, txs, proposals) = tokio::join!(
        rpc::subscribe(RPC_ENDPOINT, "blockchain.subscribe_blocks", "BLOCK"),
        rpc::subscribe(RPC_ENDPOINT, "blockchain.subscribe_txs", "TX"),
        rpc::subscribe(RPC_ENDPOINT, "blockchain.subscribe_proposals", "PROPOSAL"),
    );

    blocks?;
    txs?;
    proposals?;

    Ok(())
}

// Verify that the block returned by RPC actually reports the height we requested.
// This protects the inspection layer from trusting an unexpected or mismatched
// block response.
fn check_block_height(requested_height: u32, block: &darkfi::blockchain::BlockInfo) -> CheckResult {
    if block.header.height == requested_height {
        CheckResult::confirmed_pass(
            "block_height",
            &format!("block reports requested height {}", requested_height),
        )
    } else {
        CheckResult::confirmed_fail(
            "block_height",
            &format!(
                "block reports height {}, requested {}",
                block.header.height, requested_height
            ),
        )
    }
}

fn check_tx_hash(requested_hash: &str, tx: &darkfi::tx::Transaction) -> CheckResult {
    let actual_hash = tx.hash().to_string();

    if requested_hash.eq_ignore_ascii_case(&actual_hash) {
        CheckResult::confirmed_pass("tx_hash", "transaction hash matches the requested hash")
    } else {
        CheckResult::confirmed_fail(
            "tx_hash",
            &format!(
                "transaction hash {} does not match requested {}",
                actual_hash, requested_hash
            ),
        )
    }
}

fn check_tx_proof_alignment(tx: &darkfi::tx::Transaction) -> CheckResult {
    if tx.calls.len() == tx.proofs.len() {
        CheckResult::confirmed_pass(
            "calls_proofs",
            &format!("call count matches proof-group count ({})", tx.calls.len()),
        )
    } else {
        CheckResult::confirmed_fail(
            "calls_proofs",
            &format!(
                "call count {} does not match proof-group count {}",
                tx.calls.len(),
                tx.proofs.len()
            ),
        )
    }
}

fn check_tx_signature_alignment(tx: &darkfi::tx::Transaction) -> CheckResult {
    if tx.calls.len() == tx.signatures.len() {
        CheckResult::confirmed_pass(
            "calls_signatures",
            &format!(
                "call count matches signature-group count ({})",
                tx.calls.len()
            ),
        )
    } else {
        CheckResult::confirmed_fail(
            "calls_signatures",
            &format!(
                "call count {} does not match signature-group count {}",
                tx.calls.len(),
                tx.signatures.len()
            ),
        )
    }
}

async fn cmd_inspect(target: InspectTarget, json: bool) -> anyhow::Result<()> {
    match target {
        InspectTarget::Block { height } => {
            let block = rpc::get_block(RPC_ENDPOINT, height).await?;

            let height_check = check_block_height(height, &block);

            // Fetch the previous block once — both chain_linkage and
            // timestamp_sanity need it, no reason to fetch it twice.
            let prev_block = if height == 0 {
                None
            } else {
                rpc::get_block(RPC_ENDPOINT, height - 1).await.ok()
            };

            // Verify that this block correctly points to its predecessor.
            let linkage_check = check_chain_linkage(height, &block, prev_block.as_ref());

            // Check whether the timestamp gap from the predecessor is plausible.
            let timestamp_check = check_timestamp_sanity(height, &block, prev_block.as_ref()).await;

            // Keep the checks together so the same collection can feed both
            // the inspection report and its diagnostic summary.
            let checks = vec![height_check, linkage_check, timestamp_check];

            let inspection = BlockInspection {
                height: block.header.height,
                hash: block.header.hash().to_string(),
                previous: block.header.previous.to_string(),
                txs: block.txs.len(),
                summary: DiagnosticSummary::from_checks(&checks),
                checks,
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("Block {}", inspection.height);
                println!("  Hash:     {}", inspection.hash);
                println!("  Previous: {}", inspection.previous);
                println!("  Txs:      {}", inspection.txs);

                for check in &inspection.checks {
                    check.print_human();
                }
            }
        }

        InspectTarget::Tx { hash } => {
            let tx = rpc::get_tx(RPC_ENDPOINT, &hash).await?;

            let actual_hash = tx.hash().to_string();

            let checks = vec![
                check_tx_hash(&hash, &tx),
                check_tx_proof_alignment(&tx),
                check_tx_signature_alignment(&tx),
            ];

            let call_details = tx
                .calls
                .iter()
                .enumerate()
                .map(|(index, call)| TxCallInspection {
                    index,
                    contract_id: call.data.contract_id.to_string(),
                    function_code: call.data.data.first().copied(),
                    data_length: call.data.data.len(),
                    parent: call.parent_index,
                    children: call.children_indexes.clone(),
                })
                .collect();

            let inspection = TxInspection {
                hash: actual_hash,
                calls: tx.calls.len(),
                proofs: tx.proofs.len(),
                signatures: tx.signatures.len(),
                call_details,
                summary: DiagnosticSummary::from_checks(&checks),
                checks,
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("Transaction {}", inspection.hash);
                println!("  Calls:      {}", inspection.calls);
                println!("  Proof sets: {}", inspection.proofs);
                println!("  Sig sets:   {}", inspection.signatures);

                for call in &inspection.call_details {
                    println!("  Call {}", call.index);
                    println!("    Contract:  {}", call.contract_id);

                    if let Some(code) = call.function_code {
                        println!("    Function:  {}", code);
                    } else {
                        println!("    Function:  <none>");
                    }

                    println!("    Data:      {} byte(s)", call.data_length);
                    println!("    Parent:    {:?}", call.parent);
                    println!("    Children:  {:?}", call.children);
                }

                for check in &inspection.checks {
                    check.print_human();
                }
            }
        }
    }
    Ok(())
}

// Verify that a block points to the actual hash of its predecessor.
// This is a strict structural check: if both blocks are available,
// the result is either confirmed pass or confirmed failure.
fn check_chain_linkage(
    height: u32,
    block: &darkfi::blockchain::BlockInfo,
    prev_block: Option<&darkfi::blockchain::BlockInfo>,
) -> CheckResult {
    if height == 0 {
        return CheckResult::unknown("chain_linkage", "N/A - genesis block has no predecessor");
    }

    match prev_block {
        Some(prev) => {
            let actual_prev_hash = prev.header.hash();

            if actual_prev_hash == block.header.previous {
                CheckResult::confirmed_pass(
                    "chain_linkage",
                    &format!("previous hash matches block {}", height - 1),
                )
            } else {
                CheckResult::confirmed_fail(
                    "chain_linkage",
                    &format!(
                        "previous hash does NOT match block {} - possible reorg or consensus anomaly",
                        height - 1
                    ),
                )
            }
        }

        None => CheckResult::unknown(
            "chain_linkage",
            &format!("could not fetch block {} to verify", height - 1),
        ),
    }
}

// Check whether the time gap between this block and its predecessor is
// plausible relative to DarkFi's current block target.
async fn check_timestamp_sanity(
    height: u32,
    block: &darkfi::blockchain::BlockInfo,
    prev_block: Option<&darkfi::blockchain::BlockInfo>,
) -> CheckResult {
    if height == 0 {
        return CheckResult::unknown("timestamp_sanity", "N/A — genesis block has no predecessor");
    }

    let Some(prev) = prev_block else {
        return CheckResult::unknown(
            "timestamp_sanity",
            &format!("could not fetch block {} to verify", height - 1),
        );
    };

    let gap = block.header.timestamp.inner() as i64 - prev.header.timestamp.inner() as i64;

    let target = rpc::get_block_target(RPC_ENDPOINT).await.unwrap_or(120);

    if gap < 0 {
        CheckResult::new(
            "timestamp_sanity",
            CheckState::Fail,
            Confidence::High,
            &format!(
                "timestamp is BEFORE block {} — possible clock drift or reorg",
                height - 1
            ),
        )
    } else if gap as u64 > target * 20 {
        CheckResult::new(
            "timestamp_sanity",
            CheckState::Fail,
            Confidence::Medium,
            &format!(
                "{gap}s gap since block {} is unusually large (target: {target}s)",
                height - 1
            ),
        )
    } else {
        CheckResult::new(
            "timestamp_sanity",
            CheckState::Pass,
            Confidence::Medium,
            &format!(
                "{gap}s since block {} — within plausible range (target: {target}s)",
                height - 1
            ),
        )
    }
}
