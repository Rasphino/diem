// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{context::Context, index};

use diem_config::config::ApiConfig;
use diem_mempool::MempoolClientSender;
use diem_types::{chain_id::ChainId, protocol_spec::DpnProto};
use storage_interface::MoveDbReader;

use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

/// Creates HTTP server (warp-based)
/// Returns handle to corresponding Tokio runtime
pub fn bootstrap(
    chain_id: ChainId,
    db: Arc<dyn MoveDbReader<DpnProto>>,
    config: &ApiConfig,
    mp_sender: MempoolClientSender,
) -> Runtime {
    let runtime = Builder::new_multi_thread()
        .thread_name("api")
        .enable_all()
        .build()
        .expect("[api] failed to create runtime");

    let address = config.address;
    runtime.spawn(async move {
        let service = Context::new(chain_id, db, mp_sender);
        let routes = index::routes(service);
        let server = warp::serve(routes).bind(address);
        server.await
    });
    runtime
}
