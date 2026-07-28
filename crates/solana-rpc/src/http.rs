use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::{Value, json};
use soldisco_domain::Commitment;

use crate::{
    AccountRecord, ReadContext, RpcError, RpcHealth, SignaturePage, SignaturePageRequest,
    SignatureRecord, SolanaReader, TransactionInstructionRecord, TransactionRecord,
};

const MAX_RPC_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_RPC_ERROR_BODY_CHARS: usize = 2_048;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Concrete standard Solana JSON-RPC HTTP client.
///
/// Request timeouts, connection pooling, proxy policy, and TLS configuration
/// are inherited from the supplied [`reqwest::Client`]. Use [`Self::new`] for
/// safe defaults or [`Self::with_client`] when the server owns those policies.
#[derive(Clone)]
pub struct SolanaHttpClient {
    endpoint: Url,
    client: Client,
    next_request_id: Arc<AtomicU64>,
}

impl SolanaHttpClient {
    pub fn new(endpoint: &str) -> Result<Self, RpcError> {
        Self::with_timeout(endpoint, DEFAULT_REQUEST_TIMEOUT)
    }

    pub fn with_timeout(endpoint: &str, timeout: Duration) -> Result<Self, RpcError> {
        if timeout.is_zero() {
            return Err(RpcError::InvalidRequest(
                "HTTP RPC timeout must be greater than zero".to_owned(),
            ));
        }
        let client = Client::builder()
            .connect_timeout(timeout)
            .timeout(timeout)
            .build()
            .map_err(|error| RpcError::Transport(error.to_string()))?;
        Self::with_client(endpoint, client)
    }

    pub fn with_client(endpoint: &str, client: Client) -> Result<Self, RpcError> {
        let endpoint = Url::parse(endpoint)
            .map_err(|error| RpcError::InvalidRequest(format!("invalid HTTP RPC URL: {error}")))?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(RpcError::InvalidRequest(
                "HTTP RPC URL must use http or https".to_owned(),
            ));
        }
        Ok(Self {
            endpoint,
            client,
            next_request_id: Arc::new(AtomicU64::new(1)),
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    /// Returns the immutable genesis hash identifying the connected cluster.
    pub async fn genesis_hash(&self) -> Result<String, RpcError> {
        let hash = self.rpc::<String>("getGenesisHash", json!([])).await?;
        if hash.is_empty()
            || hash.len() > 128
            || !hash.bytes().all(|byte| {
                b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&byte)
            })
        {
            return Err(RpcError::InvalidResponse(
                "getGenesisHash returned an invalid base58 value".to_owned(),
            ));
        }
        Ok(hash)
    }

    async fn rpc<T>(&self, method: &'static str, params: Value) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let bytes = read_bounded_body(response, MAX_RPC_RESPONSE_BYTES).await?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(RpcError::RateLimited);
        }
        if !status.is_success() {
            return Err(RpcError::HttpStatus {
                status: status.as_u16(),
                body: compact_error_body(&bytes),
            });
        }

        let envelope: JsonRpcEnvelope = serde_json::from_slice(&bytes)
            .map_err(|error| RpcError::InvalidResponse(error.to_string()))?;
        if envelope.jsonrpc != "2.0" {
            return Err(RpcError::InvalidResponse(
                "response is not JSON-RPC 2.0".to_owned(),
            ));
        }
        if envelope.id != Some(id) {
            return Err(RpcError::InvalidResponse(format!(
                "response id {:?} did not match request id {id}",
                envelope.id
            )));
        }
        if let Some(error) = envelope.error {
            if error.code == 429 || error.code == -32005 {
                return Err(RpcError::RateLimited);
            }
            return Err(RpcError::Rpc {
                code: error.code,
                message: error.message,
            });
        }
        let result = envelope
            .result
            .ok_or_else(|| RpcError::InvalidResponse("response omitted result".to_owned()))?;
        serde_json::from_value(result).map_err(|error| RpcError::InvalidResponse(error.to_string()))
    }
}

#[async_trait]
impl SolanaReader for SolanaHttpClient {
    async fn health(&self) -> RpcHealth {
        match self.rpc::<String>("getHealth", json!([])).await {
            Ok(value) if value == "ok" => RpcHealth::Up,
            Ok(_) | Err(RpcError::Timeout | RpcError::RateLimited | RpcError::Rpc { .. }) => {
                RpcHealth::Degraded
            }
            Err(_) => RpcHealth::Down,
        }
    }

    async fn transaction(
        &self,
        signature: &str,
        context: ReadContext,
    ) -> Result<Option<TransactionRecord>, RpcError> {
        if signature.is_empty() {
            return Err(RpcError::InvalidRequest(
                "transaction signature cannot be empty".to_owned(),
            ));
        }
        let result: Option<RpcTransaction> = self
            .rpc(
                "getTransaction",
                json!([
                    signature,
                    {
                        "commitment": commitment_name(context.commitment),
                        "encoding": "json",
                        "maxSupportedTransactionVersion": 0,
                    }
                ]),
            )
            .await?;

        result
            .map(|transaction| {
                validate_primary_signature(signature, &transaction.transaction)?;
                if context
                    .minimum_slot
                    .is_some_and(|minimum_slot| transaction.slot < minimum_slot)
                {
                    return Err(RpcError::Unavailable(format!(
                        "transaction slot {} is below required minimum {}",
                        transaction.slot,
                        context.minimum_slot.expect("checked as some")
                    )));
                }
                let meta = transaction.meta.ok_or_else(|| {
                    RpcError::InvalidResponse("getTransaction omitted transaction meta".to_owned())
                })?;
                let instructions = decode_instructions(&transaction.transaction, &meta)?;
                let log_messages = meta.log_messages.ok_or_else(|| {
                    RpcError::InvalidResponse("getTransaction omitted log messages".to_owned())
                })?;
                Ok(TransactionRecord {
                    slot: transaction.slot,
                    transaction_index: transaction.transaction_index,
                    signature: signature.to_owned(),
                    block_time_unix_seconds: transaction.block_time,
                    instructions,
                    log_messages,
                    transaction_error: compact_error(meta.error)?,
                })
            })
            .transpose()
    }

    async fn account(
        &self,
        address: &str,
        context: ReadContext,
    ) -> Result<Option<AccountRecord>, RpcError> {
        if address.is_empty() {
            return Err(RpcError::InvalidRequest(
                "account address cannot be empty".to_owned(),
            ));
        }
        let result: RpcContext<Option<RpcAccount>> = self
            .rpc(
                "getAccountInfo",
                json!([
                    address,
                    {
                        "commitment": commitment_name(context.commitment),
                        "encoding": "base64",
                        "minContextSlot": context.minimum_slot,
                    }
                ]),
            )
            .await?;

        result
            .value
            .map(|account| {
                if account.data.1 != "base64" {
                    return Err(RpcError::InvalidResponse(format!(
                        "getAccountInfo returned unsupported encoding {}",
                        account.data.1
                    )));
                }
                Ok(AccountRecord {
                    address: address.to_owned(),
                    owner: account.owner,
                    slot: result.context.slot,
                    encoded_data: account.data.0,
                    lamports: account.lamports,
                    executable: account.executable,
                })
            })
            .transpose()
    }

    async fn signature_page(
        &self,
        request: SignaturePageRequest,
    ) -> Result<SignaturePage, RpcError> {
        SignaturePageRequest::new(
            request.program_id.clone(),
            request.before.clone(),
            request.limit,
            request.context,
        )?;
        let records: Vec<RpcSignatureRecord> = self
            .rpc(
                "getSignaturesForAddress",
                json!([
                    request.program_id,
                    {
                        "before": request.before,
                        "commitment": commitment_name(request.context.commitment),
                        "limit": request.limit,
                        "minContextSlot": request.context.minimum_slot,
                    }
                ]),
            )
            .await?;
        if records.len() > request.limit {
            return Err(RpcError::InvalidResponse(format!(
                "signature page returned {} records for limit {}",
                records.len(),
                request.limit
            )));
        }

        let mut previous_slot = u64::MAX;
        let mut signatures = HashSet::with_capacity(records.len());
        let records_newest_first = records
            .into_iter()
            .map(|record| {
                if record.signature.is_empty() {
                    return Err(RpcError::InvalidResponse(
                        "signature page contained an empty signature".to_owned(),
                    ));
                }
                if !signatures.insert(record.signature.clone()) {
                    return Err(RpcError::InvalidResponse(format!(
                        "signature page repeated {}",
                        record.signature
                    )));
                }
                if record.slot > previous_slot {
                    return Err(RpcError::InvalidResponse(
                        "signature page was not newest-to-oldest".to_owned(),
                    ));
                }
                previous_slot = record.slot;
                Ok(SignatureRecord {
                    slot: record.slot,
                    transaction_index: record.transaction_index,
                    signature: record.signature,
                    block_time_unix_seconds: record.block_time,
                    confirmation_status: record.confirmation_status,
                    transaction_error: compact_error(record.error)?,
                })
            })
            .collect::<Result<Vec<_>, RpcError>>()?;

        let exhausted = records_newest_first.len() < request.limit;
        let next_before = (!exhausted)
            .then(|| {
                records_newest_first
                    .last()
                    .map(|record| record.signature.clone())
            })
            .flatten();

        Ok(SignaturePage {
            request,
            records_newest_first,
            next_before,
            exhausted,
        })
    }
}

fn commitment_name(commitment: Commitment) -> &'static str {
    match commitment {
        Commitment::Processed => "processed",
        Commitment::Confirmed => "confirmed",
        Commitment::Finalized => "finalized",
    }
}

fn compact_error(error: Option<Value>) -> Result<Option<String>, RpcError> {
    error
        .map(|value| {
            serde_json::to_string(&value)
                .map_err(|error| RpcError::InvalidResponse(error.to_string()))
        })
        .transpose()
}

fn validate_primary_signature(
    requested_signature: &str,
    transaction: &RpcJsonTransaction,
) -> Result<(), RpcError> {
    let returned_signature = transaction.signatures.first().ok_or_else(|| {
        RpcError::InvalidResponse(
            "getTransaction returned a transaction without a primary signature".to_owned(),
        )
    })?;
    if returned_signature != requested_signature {
        return Err(RpcError::InvalidResponse(
            "getTransaction primary signature did not match the request".to_owned(),
        ));
    }
    Ok(())
}

fn decode_instructions(
    transaction: &RpcJsonTransaction,
    meta: &RpcTransactionMeta,
) -> Result<Vec<TransactionInstructionRecord>, RpcError> {
    let mut account_keys = transaction.message.account_keys.clone();
    account_keys.extend(meta.loaded_addresses.writable.iter().cloned());
    account_keys.extend(meta.loaded_addresses.readonly.iter().cloned());

    let mut inner_groups: Vec<Option<&RpcInnerInstructionGroup>> =
        vec![None; transaction.message.instructions.len()];
    for group in &meta.inner_instructions {
        let index = usize::from(group.index);
        let target = inner_groups.get_mut(index).ok_or_else(|| {
            RpcError::InvalidResponse(format!(
                "inner instruction group {} has no top-level instruction",
                group.index
            ))
        })?;
        if target.replace(group).is_some() {
            return Err(RpcError::InvalidResponse(format!(
                "duplicate inner instruction group {}",
                group.index
            )));
        }
    }

    let instruction_count = transaction.message.instructions.len().saturating_add(
        meta.inner_instructions
            .iter()
            .map(|group| group.instructions.len())
            .sum(),
    );
    let mut decoded = Vec::with_capacity(instruction_count);
    for (outer_index, instruction) in transaction.message.instructions.iter().enumerate() {
        let outer_instruction_index = u16::try_from(outer_index)
            .map_err(|_| RpcError::InvalidResponse("too many top-level instructions".to_owned()))?;
        decoded.push(decode_instruction(
            instruction,
            &account_keys,
            outer_instruction_index,
            None,
        )?);
        if let Some(group) = inner_groups[outer_index] {
            for (inner_index, instruction) in group.instructions.iter().enumerate() {
                let inner_instruction_index = u16::try_from(inner_index).map_err(|_| {
                    RpcError::InvalidResponse("too many inner instructions".to_owned())
                })?;
                decoded.push(decode_instruction(
                    instruction,
                    &account_keys,
                    outer_instruction_index,
                    Some(inner_instruction_index),
                )?);
            }
        }
    }
    Ok(decoded)
}

fn decode_instruction(
    instruction: &RpcCompiledInstruction,
    account_keys: &[String],
    outer_instruction_index: u16,
    inner_instruction_index: Option<u16>,
) -> Result<TransactionInstructionRecord, RpcError> {
    let program_id = account_keys
        .get(usize::from(instruction.program_id_index))
        .ok_or_else(|| {
            RpcError::InvalidResponse(format!(
                "program id index {} is outside {} account keys",
                instruction.program_id_index,
                account_keys.len()
            ))
        })?
        .clone();
    Ok(TransactionInstructionRecord {
        outer_instruction_index,
        inner_instruction_index,
        stack_height: instruction.stack_height,
        program_id,
        data: decode_base58(&instruction.data)?,
    })
}

fn decode_base58(encoded: &str) -> Result<Vec<u8>, RpcError> {
    const MAX_INSTRUCTION_DATA_BYTES: usize = 16 * 1024;
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    let leading_zeroes = encoded.bytes().take_while(|byte| *byte == b'1').count();
    let mut bytes_little_endian = vec![0_u8];
    for character in encoded.bytes() {
        let digit = ALPHABET
            .iter()
            .position(|candidate| *candidate == character)
            .ok_or_else(|| {
                RpcError::InvalidResponse(format!(
                    "instruction data contains invalid base58 byte {character}"
                ))
            })?;
        let mut carry = u32::try_from(digit).expect("base58 digit fits in u32");
        for byte in &mut bytes_little_endian {
            let value = u32::from(*byte) * 58 + carry;
            *byte = u8::try_from(value % 256).expect("base256 digit fits in u8");
            carry = value / 256;
        }
        while carry > 0 {
            bytes_little_endian.push(u8::try_from(carry % 256).expect("base256 digit fits in u8"));
            carry /= 256;
        }
        if bytes_little_endian.len().saturating_add(leading_zeroes) > MAX_INSTRUCTION_DATA_BYTES {
            return Err(RpcError::InvalidResponse(format!(
                "instruction data exceeded {MAX_INSTRUCTION_DATA_BYTES} decoded bytes"
            )));
        }
    }
    while bytes_little_endian.len() > 1 && bytes_little_endian.last() == Some(&0) {
        bytes_little_endian.pop();
    }

    let mut decoded = vec![0_u8; leading_zeroes];
    if leading_zeroes != encoded.len() {
        decoded.extend(bytes_little_endian.into_iter().rev());
    }
    Ok(decoded)
}

fn map_reqwest_error(error: reqwest::Error) -> RpcError {
    if error.is_timeout() {
        RpcError::Timeout
    } else {
        RpcError::Transport(error.without_url().to_string())
    }
}

async fn read_bounded_body(
    mut response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>, RpcError> {
    if response
        .content_length()
        .is_some_and(|length| length > u64::try_from(maximum).unwrap_or(u64::MAX))
    {
        return Err(response_too_large(maximum));
    }

    let initial_capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(maximum);
    let mut body = Vec::with_capacity(initial_capacity);
    while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
        append_bounded(&mut body, &chunk, maximum)?;
    }
    Ok(body)
}

fn append_bounded(body: &mut Vec<u8>, chunk: &[u8], maximum: usize) -> Result<(), RpcError> {
    if body.len().saturating_add(chunk.len()) > maximum {
        return Err(response_too_large(maximum));
    }
    body.extend_from_slice(chunk);
    Ok(())
}

fn response_too_large(maximum: usize) -> RpcError {
    RpcError::InvalidResponse(format!("response exceeded {maximum} bytes"))
}

fn compact_error_body(bytes: &[u8]) -> String {
    let body = String::from_utf8_lossy(bytes);
    let mut compact = body
        .chars()
        .take(MAX_RPC_ERROR_BODY_CHARS)
        .collect::<String>();
    if body.chars().count() > MAX_RPC_ERROR_BODY_CHARS {
        compact.push_str("…<truncated>");
    }
    compact
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcTransaction {
    slot: u64,
    #[serde(rename = "transactionIndex", default)]
    transaction_index: Option<u64>,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    transaction: RpcJsonTransaction,
    meta: Option<RpcTransactionMeta>,
}

#[derive(Debug, Deserialize)]
struct RpcTransactionMeta {
    #[serde(rename = "err")]
    error: Option<Value>,
    #[serde(rename = "logMessages")]
    log_messages: Option<Vec<String>>,
    #[serde(rename = "innerInstructions", default)]
    #[serde(deserialize_with = "deserialize_null_default")]
    inner_instructions: Vec<RpcInnerInstructionGroup>,
    #[serde(rename = "loadedAddresses", default)]
    #[serde(deserialize_with = "deserialize_null_default")]
    loaded_addresses: RpcLoadedAddresses,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct RpcJsonTransaction {
    signatures: Vec<String>,
    message: RpcJsonMessage,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct RpcJsonMessage {
    #[serde(rename = "accountKeys")]
    account_keys: Vec<String>,
    instructions: Vec<RpcCompiledInstruction>,
    #[serde(flatten)]
    other: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
struct RpcCompiledInstruction {
    #[serde(rename = "programIdIndex")]
    program_id_index: u16,
    data: String,
    #[serde(rename = "stackHeight", default)]
    stack_height: Option<u32>,
    #[serde(flatten)]
    other: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct RpcInnerInstructionGroup {
    index: u16,
    instructions: Vec<RpcCompiledInstruction>,
}

#[derive(Debug, Default, Deserialize)]
struct RpcLoadedAddresses {
    #[serde(default)]
    writable: Vec<String>,
    #[serde(default)]
    readonly: Vec<String>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct RpcContext<T> {
    context: RpcSlotContext,
    value: T,
}

#[derive(Debug, Deserialize)]
struct RpcSlotContext {
    slot: u64,
}

#[derive(Debug, Deserialize)]
struct RpcAccount {
    data: (String, String),
    executable: bool,
    lamports: u64,
    owner: String,
}

#[derive(Debug, Deserialize)]
struct RpcSignatureRecord {
    slot: u64,
    #[serde(rename = "transactionIndex", default)]
    transaction_index: Option<u64>,
    signature: String,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    #[serde(rename = "confirmationStatus")]
    confirmation_status: Option<String>,
    #[serde(rename = "err")]
    error: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RPC_ERROR_BODY_CHARS, RpcSignatureRecord, RpcTransaction, append_bounded,
        commitment_name, compact_error, compact_error_body, decode_base58, decode_instructions,
        validate_primary_signature,
    };
    use serde_json::{Value, json};
    use soldisco_domain::Commitment;

    #[test]
    fn commitment_names_match_solana_rpc() {
        assert_eq!(commitment_name(Commitment::Processed), "processed");
        assert_eq!(commitment_name(Commitment::Confirmed), "confirmed");
        assert_eq!(commitment_name(Commitment::Finalized), "finalized");
    }

    #[test]
    fn parses_json_transaction_and_resolves_outer_and_inner_programs() {
        let value = json!({
            "slot": 99,
            "transactionIndex": 7,
            "blockTime": 1_720_000_000,
            "transaction": {
                "signatures": ["sig"],
                "message": {
                    "accountKeys": ["payer", "pump"],
                    "recentBlockhash": "hash",
                    "header": {},
                    "instructions": [{
                        "programIdIndex": 1,
                        "accounts": [],
                        "data": "2"
                    }]
                }
            },
            "meta": {
                "err": null,
                "logMessages": ["Program data: ZmFjdA=="],
                "loadedAddresses": {"writable": ["loaded"], "readonly": []},
                "innerInstructions": [{
                    "index": 0,
                    "instructions": [{
                        "programIdIndex": 2,
                        "accounts": [],
                        "data": "3",
                        "stackHeight": 2
                    }]
                }]
            }
        });
        let transaction: RpcTransaction = serde_json::from_value(value).unwrap();

        assert_eq!(transaction.slot, 99);
        assert_eq!(transaction.transaction_index, Some(7));
        assert_eq!(
            transaction
                .meta
                .as_ref()
                .unwrap()
                .log_messages
                .as_ref()
                .unwrap()[0],
            "Program data: ZmFjdA=="
        );
        let instructions =
            decode_instructions(&transaction.transaction, transaction.meta.as_ref().unwrap())
                .unwrap();
        assert_eq!(instructions[0].program_id, "pump");
        assert_eq!(instructions[0].data, [1]);
        assert!(!instructions[0].is_inner());
        assert_eq!(instructions[1].program_id, "loaded");
        assert_eq!(instructions[1].data, [2]);
        assert_eq!(instructions[1].inner_instruction_index, Some(0));
        assert_eq!(instructions[1].stack_height, Some(2));
    }

    #[test]
    fn transaction_errors_remain_machine_readable_json() {
        let error = compact_error(Some(json!({"InstructionError": [2, "Custom"]}))).unwrap();
        let parsed: Value = serde_json::from_str(&error.unwrap()).unwrap();

        assert_eq!(parsed["InstructionError"][0], 2);
    }

    #[test]
    fn rejects_missing_or_mismatched_primary_transaction_signatures() {
        let transaction: RpcTransaction = serde_json::from_value(json!({
            "slot": 99,
            "blockTime": null,
            "transaction": {
                "signatures": ["returned"],
                "message": {
                    "accountKeys": [],
                    "instructions": []
                }
            },
            "meta": {
                "err": null,
                "logMessages": [],
                "innerInstructions": [],
                "loadedAddresses": {"writable": [], "readonly": []}
            }
        }))
        .unwrap();

        assert!(
            validate_primary_signature("requested", &transaction.transaction).is_err(),
            "a response for a different transaction must be rejected"
        );

        let mut no_signature = transaction.transaction;
        no_signature.signatures.clear();
        assert!(validate_primary_signature("requested", &no_signature).is_err());
    }

    #[test]
    fn transaction_index_extension_is_optional_on_signature_records() {
        let indexed: RpcSignatureRecord = serde_json::from_value(json!({
            "slot": 99,
            "transactionIndex": 3,
            "signature": "sig",
            "blockTime": null,
            "confirmationStatus": "confirmed",
            "err": null
        }))
        .unwrap();
        let standard: RpcSignatureRecord = serde_json::from_value(json!({
            "slot": 99,
            "signature": "sig",
            "blockTime": null,
            "confirmationStatus": "confirmed",
            "err": null
        }))
        .unwrap();

        assert_eq!(indexed.transaction_index, Some(3));
        assert_eq!(standard.transaction_index, None);
    }

    #[test]
    fn accepts_null_optional_instruction_metadata_as_empty() {
        let meta: super::RpcTransactionMeta = serde_json::from_value(json!({
            "err": null,
            "logMessages": [],
            "innerInstructions": null,
            "loadedAddresses": null
        }))
        .unwrap();

        assert!(meta.inner_instructions.is_empty());
        assert!(meta.loaded_addresses.writable.is_empty());
    }

    #[test]
    fn base58_decoder_is_strict_and_preserves_leading_zeroes() {
        assert_eq!(decode_base58("1").unwrap(), [0]);
        assert_eq!(decode_base58("2").unwrap(), [1]);
        assert_eq!(decode_base58("12").unwrap(), [0, 1]);
        assert!(decode_base58("0").is_err());
    }

    #[test]
    fn response_body_limit_is_enforced_before_growth() {
        let mut body = vec![1, 2];
        append_bounded(&mut body, &[3, 4], 4).expect("body at the limit is valid");
        assert_eq!(body, [1, 2, 3, 4]);

        let error = append_bounded(&mut body, &[5], 4).expect_err("body must remain bounded");
        assert!(matches!(error, crate::RpcError::InvalidResponse(_)));
        assert_eq!(body, [1, 2, 3, 4]);
    }

    #[test]
    fn upstream_error_body_is_safe_for_bounded_logging() {
        let body = vec![b'x'; MAX_RPC_ERROR_BODY_CHARS + 10];
        let compact = compact_error_body(&body);

        assert!(compact.ends_with("…<truncated>"));
        assert!(compact.chars().count() < body.len() + 20);
    }
}
