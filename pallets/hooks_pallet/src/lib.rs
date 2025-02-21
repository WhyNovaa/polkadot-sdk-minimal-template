//! A shell pallet built with [`frame`].

#![cfg_attr(not(feature = "std"), no_std)]

use frame::prelude::*;
use polkadot_sdk::polkadot_sdk_frame as frame;

// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;
    use codec::alloc::{string::String, vec, vec::Vec};
    use polkadot_sdk::{frame_support, sp_core};

    #[pallet::storage]
    pub type Data<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<u8, <T as Config>::MaxDataLen>,
        ValueQuery,
    >;

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        #[pallet::constant]
        type MaxDataLen: Get<u32>;

        #[pallet::constant]
        type URL: Get<&'static str>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        VecToBoundedVecConvertationError,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            use polkadot_sdk::sp_io::offchain::{
                http_request_start, http_response_read_body, http_response_wait, timestamp,
            };
            use polkadot_sdk::sp_runtime::offchain::HttpRequestStatus;

            log::info!("Sending request");
            let id = match http_request_start("GET", <T as Config>::URL::get(), &[]) {
                Ok(id) => {
                    log::info!("Request was sent successfully, id: {}", id.0);
                    id
                }
                Err(_) => {
                    log::error!("Http request send error");
                    return;
                }
            };

            let now = timestamp();
            let duration = sp_core::offchain::Duration::from_millis(1000);
            let wait_deadline = now.add(duration);

            log::info!("Waiting for request");
            let response_status = http_response_wait(&[id], Some(wait_deadline));

            let response_code = match response_status[0] {
                HttpRequestStatus::Finished(response_code) => {
                    log::info!("Http response code: {}", response_code);
                    response_code
                }
                _ => {
                    log::error!("Http response error");
                    return;
                }
            };

            if response_code != 200 {
                log::error!("Bad response code -> stopping");
                return;
            }

            let now = timestamp();
            let duration = sp_core::offchain::Duration::from_millis(1000);
            let read_deadline = now.add(duration);

            let mut buff = vec![0; 4096];

            log::info!("Reading body request");
            let bytes_read = match http_response_read_body(id, &mut buff, Some(read_deadline)) {
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

            let body_as_u8 = &buff[..bytes_read as usize];
            let body = String::from_utf8_lossy(body_as_u8);

            log::info!("Body: {}", body);

            log::info!("Saving data");
            match Self::save_data(
                frame_support::dispatch::RawOrigin::None.into(),
                Vec::from(body_as_u8),
            ) {
                Ok(_) => log::info!("Data was saved successfully"),
                Err(_) => log::error!("Data wasn't saved successfully"),
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10000)]
        pub fn save_data(origin: T::RuntimeOrigin, data: Vec<u8>) -> DispatchResult {
            ensure_none(origin)?;

            let block_number = frame_system::Pallet::<T>::block_number();

            let bounded_vec: BoundedVec<u8, <T as Config>::MaxDataLen> = data
                .try_into()
                .map_err(|_| Error::<T>::VecToBoundedVecConvertationError)?;

            Data::<T>::insert(block_number, bounded_vec);

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::save_data { data } => ValidTransaction::with_tag_prefix("Data")
                    .and_provides(data)
                    .propagate(true)
                    .build(),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}
