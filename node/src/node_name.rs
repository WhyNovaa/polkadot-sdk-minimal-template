//! RPC client for getting node's network name

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use polkadot_sdk::polkadot_service::BlockT;
use polkadot_sdk::sp_api::ProvideRuntimeApi;
use polkadot_sdk::sp_blockchain::HeaderBackend;
use std::sync::Arc;

#[rpc(client, server)]
pub trait NodeNameApi<Block> {
    #[method(name = "nodeName")]
    fn get_name(&self) -> RpcResult<String>;
}

pub struct NodeName<C, P> {
    client: Arc<C>,
    node_name: Arc<String>,
    _marker: std::marker::PhantomData<P>,
}

impl<C, P> NodeName<C, P> {
    pub fn new(client: Arc<C>, node_name: Arc<String>) -> Self {
        Self {
            client,
            node_name,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> NodeNameApiServer<<Block as BlockT>::Hash> for NodeName<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
{
    fn get_name(&self) -> RpcResult<String> {
        Ok((*self.node_name).clone())
    }
}
