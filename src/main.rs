mod rpc;

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
}

// #[tokio::main] turns `main` into an async function tokio can drive.
// Without this, `async`/`await` below wouldn't actually run anything.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parses real argv into our Cli struct. If the user runs something
    // invalid, clap prints a helpful error and exits — we never see it.
    let cli = Cli::parse();

    match cli.command{
        Command::Ping => cmd_ping(cli.json).await?,
        Command::Peers => cmd_peers(cli.json).await?,
        Command::Status => cmd_status(cli.json).await?,
        Command::Events => cmd_events().await?,
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
        println!("{} outbound slot(s): {:?}", info.outbound_slots.len(), info.outbound_slots);
    }
    Ok(())
}

async fn cmd_status(json: bool) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    // blockchain.last_confirmed_block returns a raw 2-element array [height, hash],
    // so it deserializes straight into a tuple — no wrapper struct needed.
    let lcb_reply =
        rpc::call(RPC_ENDPOINT, "blockchain.last_confirmed_block", Value::Array(vec![])).await?;
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
        println!("Peers: {}/{} connected", status.peers_connected, status.peers_slots);
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



















