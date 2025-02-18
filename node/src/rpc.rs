// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::RpcModule;
use minimal_template_runtime::interface::{AccountId, Nonce, OpaqueBlock};
use minimal_template_runtime::NodeNameApi;
use polkadot_sdk::polkadot_service::BlockT;
use polkadot_sdk::{
    sc_transaction_pool_api::TransactionPool,
    sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata},
    *,
};
use sp_api::ProvideRuntimeApi;
use std::sync::Arc;

#[rpc(client, server)]
pub trait NodeNameApi<Block> {
    #[method(name = "node_name")]
    fn get_name(&self) -> RpcResult<u32>;
}

pub struct NodeName<C, P> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<P>,
}

impl<C, P> NodeName<C, P> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> NodeNameApiServer<<Block as BlockT>::Hash> for NodeName<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: NodeNameApi<Block>,
{
    fn get_name(&self) -> RpcResult<u32> {
        Ok(32)
    }
}

/// Full client dependencies.
pub struct FullDeps<C, P> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
}

#[docify::export]
/// Instantiate all full RPC extensions.
pub fn create_full<C, P>(
    deps: FullDeps<C, P>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: Send
        + Sync
        + 'static
        + sp_api::ProvideRuntimeApi<OpaqueBlock>
        + HeaderBackend<OpaqueBlock>
        + HeaderMetadata<OpaqueBlock, Error = BlockChainError>
        + 'static,
    C::Api: sp_block_builder::BlockBuilder<OpaqueBlock>,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<OpaqueBlock, AccountId, Nonce>,
    C::Api: NodeNameApi<OpaqueBlock>,
    P: TransactionPool + 'static,
{
    use polkadot_sdk::substrate_frame_rpc_system::{System, SystemApiServer};
    let mut module = RpcModule::new(());
    let FullDeps { client, pool } = deps;

    module.merge(System::new(client.clone(), pool.clone()).into_rpc())?;

    module.merge(NodeName::new(client.clone()).into_rpc())?;
    Ok(module)
}
