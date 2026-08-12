use darkfi::{blockchain::BlockInfo, util::encoding::base64, validator::consensus::Proposal};
use darkfi_serial::deserialize_async;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{Duration, sleep};

// Mirrors darkfi's JsonRequest struct (src/rpc/jsonrpc.rs) — same four
// fields, same JSON-RPC 2.0 shape. #[derive(Serialize)] auto-generates
// the code that turns this struct into a JSON string — we don't write
// that conversion by hand.
#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: i64,
    method: String,
    params: Value,
}

impl JsonRpcRequest {
    fn new(method: &str, params: Value) -> Self {
        // darkfi picks a random id for each request; a fixed id is fine
        // for now since we only ever send one request at a time.
        Self {
            jsonrpc: "2.0",
            id: 1,
            method: method.to_string(),
            params,
        }
    }
}

/// The actual single-attempt connect+send+read, unchanged from before.
/// Renamed to make room for `call`, which wraps this with retries.
async fn call_once(endpoint: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    let stream = TcpStream::connect(endpoint).await?;

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let req = JsonRpcRequest::new(method, params);
    let mut req_json = serde_json::to_string(&req)?;
    req_json.push('\n');
    write_half.write_all(req_json.as_bytes()).await?;

    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let parsed: Value = serde_json::from_str(&line)?;
    let result = parsed
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no 'result' field in reply: {line}"))?;

    Ok(result)
}

/// Public entry point — same signature as before, but now retries up to
/// 3 times total (1 initial attempt + 2 retries) with a short delay
/// between attempts, instead of failing on the very first hiccup.
pub async fn call(endpoint: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    const MAX_ATTEMPTS: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    let mut last_err = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match call_once(endpoint, method, params.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                eprintln!("attempt {attempt}/{MAX_ATTEMPTS} failed: {e}");
                last_err = Some(e);
                if attempt < MAX_ATTEMPTS {
                    sleep(RETRY_DELAY).await;
                }
            }
        }
    }
    // unwrap is safe here: the loop only exits without returning Ok
    // after at least one Err was recorded into last_err.
    Err(last_err.unwrap())
}

// Shared first half: pull the base64 payload out of a notification and
// decode it to raw bytes. Both BlockInfo and Proposal decoding start here.
fn extract_payload_bytes(line: &str) -> Result<Vec<u8>, String> {
    let parsed: Value = serde_json::from_str(line)
        .map_err(|e| format!("(failed to parse notification JSON: {e})"))?;
    let params = parsed
        .get("params")
        .and_then(|p| p.as_array())
        .ok_or_else(|| String::from("(no params array in notification)"))?;
    let b64 = params
        .first()
        .and_then(|p| p.as_str())
        .ok_or_else(|| String::from("(no base64 payload in notification)"))?;
    base64::decode(b64).ok_or_else(|| String::from("(failed to base64-decode payload)"))
}

// For subscribe_blocks: payload is a raw BlockInfo.
async fn decode_block_notification(line: &str) -> String {
    let bytes = match extract_payload_bytes(line) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let block: BlockInfo = match deserialize_async(&bytes).await {
        Ok(b) => b,
        Err(e) => return format!("(failed to deserialize BlockInfo: {e})"),
    };
    format!(
        "height={} hash={} txs={}",
        block.header.height,
        block.header.hash(),
        block.txs.len()
    )
}

// Fetches a single block by height via blockchain.get_block and decodes
// it into a real BlockInfo. Unlike the notification decoders, this comes
// from call() (request/reply), not a pushed subscription — so the
// payload is the raw base64 string directly in the result, not wrapped
// in a "params" array.
pub async fn get_block(endpoint: &str, height: u32) -> anyhow::Result<BlockInfo> {
    let result = call(
        endpoint,
        "blockchain.get_block",
        serde_json::json!([height]),
    )
    .await?;
    let b64 = result
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("expected string result, got: {result}"))?;
    let bytes = base64::decode(b64)
        .ok_or_else(|| anyhow::anyhow!("failed to base64-decode block payload"))?;
    let block: BlockInfo = deserialize_async(&bytes).await?;
    Ok(block)
}

// Fetches darkfid's currently configured block target time (seconds),
// used as the baseline for judging whether a real timestamp gap looks
// plausible or not.
pub async fn get_block_target(endpoint: &str) -> anyhow::Result<u64> {
    let result = call(endpoint, "blockchain.block_target", serde_json::json!([])).await?;
    let target = serde_json::from_value(result)?;
    Ok(target)
}

// For subscribe_proposals: payload is Proposal { hash, block: BlockInfo },
// NOT a raw BlockInfo — confirmed via src/validator/consensus.rs.
async fn decode_proposal_notification(line: &str) -> String {
    let bytes = match extract_payload_bytes(line) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let proposal: Proposal = match deserialize_async(&bytes).await {
        Ok(p) => p,
        Err(e) => return format!("(failed to deserialize Proposal: {e})"),
    };
    format!(
        "height={} hash={} txs={}",
        proposal.block.header.height,
        proposal.hash,
        proposal.block.txs.len()
    )
}

// subscribe_txs notifications are already a plain hex tx hash string —
// no binary decoding needed, just pull it out of the params array.
fn decode_tx_notification(line: &str) -> String {
    let parsed: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return format!("(failed to parse notification JSON: {e})"),
    };
    let Some(params) = parsed.get("params").and_then(|p| p.as_array()) else {
        return String::from("(no params array in notification)");
    };
    let Some(hash) = params.first().and_then(|p| p.as_str()) else {
        return String::from("(no tx hash in notification)");
    };
    format!("tx_hash={hash}")
}

// subscribe connects to `endpoint`, sends one subscribe request for `method`,
// then loops forever printing every line darkfid pushes back, until the
// connection drops or an error occurs. Unlike call(), this never returns
// under normal operation — it's meant to run as a long-lived task.
pub async fn subscribe(endpoint: &str, method: &str, label: &str) -> anyhow::Result<()> {
    let stream = TcpStream::connect(endpoint).await?;
    let (read_half, mut write_half) = stream.into_split();

    // Send the subscribe request once, same JSON-RPC 2.0 shape as call().
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": [],
        "id": 1
    });
    let mut line = req.to_string();
    line.push('\n');
    write_half.write_all(line.as_bytes()).await?;

    // Now just keep reading lines forever — each one is a pushed notification.
    let mut reader = BufReader::new(read_half);
    loop {
        let mut buf = String::new();
        let bytes_read = reader.read_line(&mut buf).await?;
        if bytes_read == 0 {
            anyhow::bail!("[{label}] connection closed by darkfid");
        }

        let summary = match method {
            "blockchain.subscribe_blocks" => decode_block_notification(buf.trim()).await,
            "blockchain.subscribe_proposals" => decode_proposal_notification(buf.trim()).await,
            "blockchain.subscribe_txs" => decode_tx_notification(buf.trim()),
            _ => buf.trim().to_string(),
        };

        println!("[{label}] {summary}");
    }
}
