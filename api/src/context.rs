// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use diem_api_types::{Error, LedgerInfo, MoveConverter, TransactionOnChainData};
use diem_crypto::HashValue;
use diem_mempool::{MempoolClientRequest, MempoolClientSender, SubmissionStatus};
use diem_types::{
    account_address::AccountAddress,
    account_state::AccountState,
    account_state_blob::AccountStateBlob,
    chain_id::ChainId,
    ledger_info::LedgerInfoWithSignatures,
    protocol_spec::DpnProto,
    transaction::{SignedTransaction, TransactionInfo},
};
use storage_interface::MoveDbReader;

use anyhow::{ensure, format_err, Result};
use futures::{channel::oneshot, SinkExt};
use std::{
    borrow::Borrow,
    convert::{Infallible, TryFrom},
    sync::Arc,
};
use warp::Filter;

// Context holds application scope context
#[derive(Clone)]
pub struct Context {
    chain_id: ChainId,
    db: Arc<dyn MoveDbReader<DpnProto>>,
    mp_sender: MempoolClientSender,
}

impl Context {
    pub fn new(
        chain_id: ChainId,
        db: Arc<dyn MoveDbReader<DpnProto>>,
        mp_sender: MempoolClientSender,
    ) -> Self {
        Self {
            chain_id,
            db,
            mp_sender,
        }
    }

    pub fn move_converter(&self) -> MoveConverter<dyn MoveDbReader<DpnProto> + '_> {
        MoveConverter::new(self.db.borrow())
    }

    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    pub fn filter(self) -> impl Filter<Extract = (Context,), Error = Infallible> + Clone {
        warp::any().map(move || self.clone())
    }

    pub async fn submit_transaction(&self, txn: SignedTransaction) -> Result<SubmissionStatus> {
        let (req_sender, callback) = oneshot::channel();
        self.mp_sender
            .clone()
            .send(MempoolClientRequest::SubmitTransaction(txn, req_sender))
            .await?;

        callback.await?
    }

    pub fn get_latest_ledger_info(&self) -> Result<LedgerInfo, Error> {
        Ok(LedgerInfo::new(
            &self.chain_id(),
            &self.get_latest_ledger_info_with_signatures()?,
        ))
    }

    pub fn get_latest_ledger_info_with_signatures(&self) -> Result<LedgerInfoWithSignatures> {
        self.db.get_latest_ledger_info()
    }

    pub fn get_account_state(
        &self,
        address: AccountAddress,
        version: u64,
    ) -> Result<Option<AccountState>> {
        let state = self.get_account_state_blob(address, version)?;
        Ok(match state {
            Some(blob) => Some(AccountState::try_from(&blob)?),
            None => None,
        })
    }

    pub fn get_account_state_blob(
        &self,
        account: AccountAddress,
        version: u64,
    ) -> Result<Option<AccountStateBlob>> {
        let (account_state_blob, _) = self
            .db
            .get_account_state_with_proof_by_version(account, version)?;
        Ok(account_state_blob)
    }

    pub fn get_transactions(
        &self,
        start_version: u64,
        limit: u16,
        ledger_version: u64,
    ) -> Result<Vec<TransactionOnChainData<TransactionInfo>>> {
        let data = self
            .db
            .get_transactions(start_version, limit as u64, ledger_version, true)?;

        let txn_start_version = data
            .first_transaction_version
            .ok_or_else(|| format_err!("no start version from database"))?;
        ensure!(
            txn_start_version == start_version,
            "invalid start version from database: {} != {}",
            txn_start_version,
            start_version
        );

        let txns = data.transactions;
        let infos = data.proof.transaction_infos;
        let events = data.events.unwrap_or_default();
        ensure!(
            txns.len() == infos.len() && txns.len() == events.len(),
            "invalid data size from database: {}, {}, {}",
            txns.len(),
            infos.len(),
            events.len()
        );

        Ok(txns
            .into_iter()
            .zip(infos.into_iter())
            .zip(events.into_iter())
            .enumerate()
            .map(|(i, ((txn, info), events))| (start_version + i as u64, txn, info, events).into())
            .collect())
    }

    pub fn get_transaction_by_hash(
        &self,
        hash: HashValue,
        ledger_version: u64,
    ) -> Result<Option<TransactionOnChainData<TransactionInfo>>> {
        Ok(self
            .db
            .get_transaction_by_hash(hash, ledger_version, true)?
            .map(|t| t.into()))
    }

    pub async fn get_pending_transaction_by_hash(
        &self,
        hash: HashValue,
    ) -> Result<Option<SignedTransaction>> {
        let (req_sender, callback) = oneshot::channel();

        self.mp_sender
            .clone()
            .send(MempoolClientRequest::GetTransactionByHash(hash, req_sender))
            .await
            .map_err(anyhow::Error::from)?;

        callback.await.map_err(anyhow::Error::from)
    }

    pub fn get_transaction_by_version(
        &self,
        version: u64,
        ledger_version: u64,
    ) -> Result<TransactionOnChainData<TransactionInfo>> {
        Ok(self
            .db
            .get_transaction_by_version(version, ledger_version, true)?
            .into())
    }
}
