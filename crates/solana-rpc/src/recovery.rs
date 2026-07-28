use crate::{
    ReadContext, RecoveryCheckpoint, RpcError, SignaturePage, SignaturePageRequest, SignatureRecord,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryBatch {
    pub program_id: String,
    /// Safe processing order: oldest recovered transaction to newest.
    pub records_oldest_first: Vec<SignatureRecord>,
    /// Durable checkpoint to persist only after the complete batch succeeds.
    pub head_checkpoint: Option<RecoveryCheckpoint>,
    pub pages_fetched: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryProgress {
    More,
    Complete(RecoveryBatch),
}

/// Deterministic `getSignaturesForAddress` pagination.
///
/// Solana returns newest-first pages. This pager buffers those pages until it
/// reaches the exact durable checkpoint, then releases one oldest-first batch.
/// It never releases a partial gap. The server can therefore process the batch
/// idempotently and advance to `head_checkpoint` only after durable success.
#[derive(Clone, Debug)]
pub struct RecoveryPager {
    program_id: String,
    context: ReadContext,
    page_size: usize,
    max_records: usize,
    checkpoint: Option<RecoveryCheckpoint>,
    before: Option<String>,
    records_newest_first: Vec<SignatureRecord>,
    pages_fetched: usize,
    complete: bool,
}

impl RecoveryPager {
    /// Read one newest page to establish an initial head. This deliberately
    /// does not imply full historical ingestion.
    pub fn initial_head(
        program_id: impl Into<String>,
        page_size: usize,
        context: ReadContext,
    ) -> Result<Self, RpcError> {
        let program_id = program_id.into();
        SignaturePageRequest::new(program_id.clone(), None, page_size, context)?;
        Ok(Self {
            program_id,
            context,
            page_size,
            max_records: page_size,
            checkpoint: None,
            before: None,
            records_newest_first: Vec::with_capacity(page_size),
            pages_fetched: 0,
            complete: false,
        })
    }

    /// Recover every signature newer than an exact durable checkpoint.
    ///
    /// If the checkpoint is not found or the safety limit is reached, recovery
    /// fails without releasing any records.
    pub fn after_checkpoint(
        checkpoint: RecoveryCheckpoint,
        page_size: usize,
        max_records: usize,
        context: ReadContext,
    ) -> Result<Self, RpcError> {
        if checkpoint.program_id.is_empty() || checkpoint.last_signature.is_empty() {
            return Err(RpcError::InvalidRequest(
                "recovery checkpoint program and signature cannot be empty".to_owned(),
            ));
        }
        SignaturePageRequest::new(checkpoint.program_id.clone(), None, page_size, context)?;
        if max_records < page_size {
            return Err(RpcError::InvalidRequest(
                "recovery max_records cannot be smaller than page_size".to_owned(),
            ));
        }
        Ok(Self {
            program_id: checkpoint.program_id.clone(),
            context,
            page_size,
            max_records,
            checkpoint: Some(checkpoint),
            before: None,
            records_newest_first: Vec::with_capacity(max_records.min(page_size * 2)),
            pages_fetched: 0,
            complete: false,
        })
    }

    pub fn next_request(&self) -> Result<SignaturePageRequest, RpcError> {
        if self.complete {
            return Err(RpcError::RecoveryProtocol(
                "pager is already complete".to_owned(),
            ));
        }
        SignaturePageRequest::new(
            self.program_id.clone(),
            self.before.clone(),
            self.page_size,
            self.context,
        )
    }

    pub fn accept_page(&mut self, page: SignaturePage) -> Result<RecoveryProgress, RpcError> {
        let expected = self.next_request()?;
        if page.request != expected {
            return Err(RpcError::RecoveryProtocol(
                "page does not match the outstanding request".to_owned(),
            ));
        }
        validate_page_shape(&page)?;
        self.pages_fetched += 1;

        if self.checkpoint.is_none() {
            self.records_newest_first.extend(page.records_newest_first);
            return Ok(RecoveryProgress::Complete(self.finish_batch(None)));
        }

        let checkpoint = self
            .checkpoint
            .as_ref()
            .expect("checkpoint branch is checked");
        if let Some(index) = page
            .records_newest_first
            .iter()
            .position(|record| record.signature == checkpoint.last_signature)
        {
            let boundary = &page.records_newest_first[index];
            if boundary.slot != checkpoint.last_slot {
                return Err(RpcError::RecoveryProtocol(format!(
                    "checkpoint signature slot {} did not match durable slot {}",
                    boundary.slot, checkpoint.last_slot
                )));
            }
            self.ensure_capacity(index)?;
            self.records_newest_first
                .extend(page.records_newest_first.into_iter().take(index));
            let head = self.head_checkpoint();
            return Ok(RecoveryProgress::Complete(self.finish_batch(head)));
        }

        self.ensure_capacity(page.records_newest_first.len())?;
        self.records_newest_first.extend(page.records_newest_first);
        if page.exhausted {
            return Err(RpcError::RecoveryCheckpointNotFound {
                program_id: checkpoint.program_id.clone(),
                slot: checkpoint.last_slot,
                signature: checkpoint.last_signature.clone(),
            });
        }
        let next_before = page.next_before.ok_or_else(|| {
            RpcError::RecoveryProtocol("non-exhausted page omitted next_before".to_owned())
        })?;
        if self.before.as_ref() == Some(&next_before) {
            return Err(RpcError::RecoveryProtocol(
                "signature cursor did not advance".to_owned(),
            ));
        }
        self.before = Some(next_before);
        Ok(RecoveryProgress::More)
    }

    fn ensure_capacity(&self, additional: usize) -> Result<(), RpcError> {
        if self.records_newest_first.len().saturating_add(additional) > self.max_records {
            Err(RpcError::RecoveryLimitExceeded {
                max_records: self.max_records,
            })
        } else {
            Ok(())
        }
    }

    fn head_checkpoint(&self) -> Option<RecoveryCheckpoint> {
        self.records_newest_first
            .first()
            .map(|record| RecoveryCheckpoint {
                program_id: self.program_id.clone(),
                last_slot: record.slot,
                last_transaction_index: record.transaction_index,
                last_signature: record.signature.clone(),
            })
            .or_else(|| self.checkpoint.clone())
    }

    fn finish_batch(&mut self, head_checkpoint: Option<RecoveryCheckpoint>) -> RecoveryBatch {
        self.complete = true;
        let mut records_oldest_first = std::mem::take(&mut self.records_newest_first);
        let head_checkpoint = head_checkpoint.or_else(|| {
            records_oldest_first
                .first()
                .map(|record| RecoveryCheckpoint {
                    program_id: self.program_id.clone(),
                    last_slot: record.slot,
                    last_transaction_index: record.transaction_index,
                    last_signature: record.signature.clone(),
                })
        });
        records_oldest_first.reverse();
        RecoveryBatch {
            program_id: self.program_id.clone(),
            records_oldest_first,
            head_checkpoint,
            pages_fetched: self.pages_fetched,
        }
    }
}

fn validate_page_shape(page: &SignaturePage) -> Result<(), RpcError> {
    if page.records_newest_first.len() > page.request.limit {
        return Err(RpcError::RecoveryProtocol(
            "page exceeded requested limit".to_owned(),
        ));
    }
    if page.exhausted && page.next_before.is_some() {
        return Err(RpcError::RecoveryProtocol(
            "exhausted page included next_before".to_owned(),
        ));
    }
    if !page.exhausted {
        let expected_cursor = page
            .records_newest_first
            .last()
            .map(|record| record.signature.as_str());
        if page.next_before.as_deref() != expected_cursor {
            return Err(RpcError::RecoveryProtocol(
                "next_before did not identify the oldest page record".to_owned(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use soldisco_domain::Commitment;

    use super::{RecoveryPager, RecoveryProgress};
    use crate::{
        ReadContext, RecoveryCheckpoint, RpcError, SignaturePage, SignaturePageRequest,
        SignatureRecord,
    };

    fn context() -> ReadContext {
        ReadContext {
            commitment: Commitment::Confirmed,
            minimum_slot: None,
        }
    }

    fn record(slot: u64, signature: &str) -> SignatureRecord {
        SignatureRecord {
            slot,
            transaction_index: None,
            signature: signature.to_owned(),
            block_time_unix_seconds: None,
            confirmation_status: Some("confirmed".to_owned()),
            transaction_error: None,
        }
    }

    fn page(
        request: SignaturePageRequest,
        records: Vec<SignatureRecord>,
        exhausted: bool,
    ) -> SignaturePage {
        let next_before = (!exhausted)
            .then(|| records.last().map(|record| record.signature.clone()))
            .flatten();
        SignaturePage {
            request,
            records_newest_first: records,
            next_before,
            exhausted,
        }
    }

    #[test]
    fn recovery_buffers_pages_and_releases_oldest_first_at_exact_checkpoint() {
        let checkpoint = RecoveryCheckpoint {
            program_id: "pump".to_owned(),
            last_slot: 10,
            last_transaction_index: None,
            last_signature: "s10".to_owned(),
        };
        let mut pager = RecoveryPager::after_checkpoint(checkpoint, 2, 8, context()).unwrap();

        let request = pager.next_request().unwrap();
        assert_eq!(
            pager
                .accept_page(page(
                    request,
                    vec![record(14, "s14"), record(13, "s13")],
                    false
                ))
                .unwrap(),
            RecoveryProgress::More
        );
        let request = pager.next_request().unwrap();
        let result = pager
            .accept_page(page(
                request,
                vec![record(12, "s12"), record(10, "s10")],
                false,
            ))
            .unwrap();

        let RecoveryProgress::Complete(batch) = result else {
            panic!("expected complete recovery");
        };
        assert_eq!(
            batch
                .records_oldest_first
                .iter()
                .map(|record| record.signature.as_str())
                .collect::<Vec<_>>(),
            ["s12", "s13", "s14"]
        );
        assert_eq!(batch.head_checkpoint.unwrap().last_signature, "s14");
        assert_eq!(batch.pages_fetched, 2);
    }

    #[test]
    fn missing_checkpoint_and_safety_limit_fail_without_a_partial_batch() {
        let checkpoint = RecoveryCheckpoint {
            program_id: "pump".to_owned(),
            last_slot: 1,
            last_transaction_index: None,
            last_signature: "missing".to_owned(),
        };
        let mut missing =
            RecoveryPager::after_checkpoint(checkpoint.clone(), 2, 4, context()).unwrap();
        let request = missing.next_request().unwrap();
        let error = missing
            .accept_page(page(request, vec![record(3, "s3")], true))
            .unwrap_err();
        assert!(matches!(error, RpcError::RecoveryCheckpointNotFound { .. }));

        let mut limited = RecoveryPager::after_checkpoint(checkpoint, 2, 2, context()).unwrap();
        let request = limited.next_request().unwrap();
        limited
            .accept_page(page(request, vec![record(4, "s4"), record(3, "s3")], false))
            .unwrap();
        let request = limited.next_request().unwrap();
        let error = limited
            .accept_page(page(request, vec![record(2, "s2")], true))
            .unwrap_err();
        assert!(matches!(error, RpcError::RecoveryLimitExceeded { .. }));
    }

    #[test]
    fn initial_head_is_explicitly_one_page_and_chronological() {
        let mut pager = RecoveryPager::initial_head("pump", 3, context()).unwrap();
        let request = pager.next_request().unwrap();
        let result = pager
            .accept_page(page(
                request,
                vec![record(9, "s9"), record(8, "s8"), record(7, "s7")],
                false,
            ))
            .unwrap();

        let RecoveryProgress::Complete(batch) = result else {
            panic!("expected initial head");
        };
        assert_eq!(batch.records_oldest_first[0].signature, "s7");
        assert_eq!(batch.head_checkpoint.unwrap().last_signature, "s9");
    }
}
