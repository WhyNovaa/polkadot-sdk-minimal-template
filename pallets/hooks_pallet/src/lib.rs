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
    use polkadot_sdk::sp_io::offchain::{
        http_request_start, http_response_read_body, http_response_wait, timestamp,
    };
    use polkadot_sdk::sp_runtime::offchain::{HttpRequestId, HttpRequestStatus};
    use polkadot_sdk::{frame_support, sp_core};

    #[pallet::storage]
    pub type Data<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Blake2_128Concat,
        u64,
        BoundedVec<u8, <T as Config>::MaxDataLen>,
        ValueQuery,
    >;

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        #[pallet::constant]
        type MaxDataLen: Get<u32>;

        #[pallet::constant]
        type MaxEntries: Get<u64>;

        const URL: &'static str = "https://bcsports.io/";

        const RESPONSE_TIME_LIMIT: u64 = 500;

        const READING_TIME_LIMIT: u64 = 200;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        VecToBoundedVecConvertationError,
        RequestReadingError,
        DataSavingError,
        HttpRequestSendingError,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            log::info!("Sending request");
            let id = match http_request_start("GET", <T as Config>::URL, &[]) {
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
            let duration =
                sp_core::offchain::Duration::from_millis(<T as Config>::RESPONSE_TIME_LIMIT);
            let response_deadline = now.add(duration);

            log::info!("Waiting for request");
            let response_status = http_response_wait(&[id], Some(response_deadline));

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

            if let Err(e) = Self::read_and_save_chunks(id) {
                log::error!("Error while reading and saving: {:?}", e);
                return;
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

            let k2 = Self::get_max_k2_or_0(block_number);
            let new_k2 = k2.saturating_add(1);

            log::info!(
                "Saving chunk: block_number({:?}), new_k2({})",
                block_number,
                new_k2
            );
            Data::<T>::insert(block_number, new_k2, bounded_vec);

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

    impl<T: Config> Pallet<T> {
        fn get_max_k2_or_0(k1: BlockNumberFor<T>) -> u64 {
            Data::<T>::iter_prefix(k1)
                .map(|(k2, _)| k2)
                .max()
                .unwrap_or(0)
        }

        fn read_and_save_chunks(id: HttpRequestId) -> Result<(), Error<T>> {
            let now = timestamp();
            let duration =
                sp_core::offchain::Duration::from_millis(<T as Config>::READING_TIME_LIMIT);
            let reading_deadline = now.add(duration);

            let mut buff = vec![0; <T as Config>::MaxDataLen::get() as usize];

            loop {
                log::info!("Reading chunk of body request");
                let bytes_to_read = http_response_read_body(id, &mut buff, Some(reading_deadline))
                    .map_err(|_| Error::<T>::RequestReadingError)?;

                if bytes_to_read == 0 {
                    return Ok(());
                }

                log::info!(
                    "Request's body was read successfully, bytes to read: {}",
                    bytes_to_read
                );

                let body_as_u8 = &buff[..bytes_to_read as usize];
                let body = String::from_utf8_lossy(body_as_u8);

                log::info!("Body: {}", body);

                Self::save_data(
                    frame_support::dispatch::RawOrigin::None.into(),
                    Vec::from(body_as_u8),
                )
                .map_err(|_| Error::<T>::DataSavingError)?;
                log::info!("Data was saved successfully")
            }
        }
    }
}
