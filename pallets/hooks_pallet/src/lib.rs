//! A shell pallet built with [`frame`].

#![cfg_attr(not(feature = "std"), no_std)]

use frame::prelude::*;
use polkadot_sdk::polkadot_sdk_frame as frame;

// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            use codec::alloc::{
                string::String,
                vec
            };
            use polkadot_sdk::sp_core::offchain::HttpRequestId;
            use polkadot_sdk::sp_io::offchain::{
                http_request_start, http_response_read_body, http_response_wait,
            };
            use polkadot_sdk::sp_runtime::offchain::HttpRequestStatus;

            log::info!("Sending request");
            let id = match http_request_start("GET", "https://polkadot.js.org", &[]) {
                Ok(id) => {
                    log::info!("Request was sent successfully, id: {}", id.0);
                    id
                }
                Err(_) => {
                    log::error!("Http request send error");
                    return;
                }
            };

            log::info!("Waiting for request");
            let response_status = http_response_wait(&[id], None);

            let _response_code = match response_status[0] {
                HttpRequestStatus::Finished(response_code) => {
                    log::info!("Http response code: {}", response_code);
                    response_code
                }
                _ => {
                    log::error!("Http response error");
                    return;
                }
            };

            log::info!("Reading body request");
            let mut buff = vec![0; 4096];
            let bytes_read = match http_response_read_body(id, &mut buff, None) {
                Ok(bytes_read) => {
                    log::info!(
                        "Request's body was read successfully, bytes to read: {}",
                        bytes_read
                    );
                    bytes_read
                }
                Err(_) => {
                    log::error!("Error in reading request's");
                    return;
                }
            };

            let body = String::from_utf8_lossy(&buff[..bytes_read as usize]);

            log::info!("Body: {}", body);
        }
    }

    #[pallet::storage]
    pub type Value<T> = StorageValue<Value = u32>;
}