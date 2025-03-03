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
    use frame_system::{
        offchain::SendTransactionTypes, offchain::SubmitTransaction, pallet_prelude::*,
    };
    use polkadot_sdk::sp_io::offchain::{
        http_request_start, http_response_read_body, http_response_wait, timestamp,
    };
    use polkadot_sdk::sp_runtime::offchain::{HttpRequestId, HttpRequestStatus};
    use polkadot_sdk::{frame_support, sp_core};
    use sp_core::offchain::{Duration, Timestamp};

    /// (k1: block number, k2: index of data for current block number) : chunk of data
    #[pallet::storage]
    pub type DataChunks<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Blake2_128Concat,
        u64,
        BoundedVec<u8, <T as Config>::MaxDataLen>,
        ValueQuery,
    >;

    /// Current amount of chunks in storage DataChunks
    #[pallet::storage]
    #[pallet::getter(fn current_amount_of_chunks)]
    pub type CurrentAmountOfChunks<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::config]
    pub trait Config:
        polkadot_sdk::frame_system::Config + SendTransactionTypes<Call<Self>>
    {
        /// Maximum data length of BoundedVec in storage DataChunks
        #[pallet::constant]
        type MaxDataLen: Get<u32>;

        /// Maximum amount of chunks in storage DataChunks
        #[pallet::constant]
        type MaxChunks: Get<u64>;

        /// URL of  HTTP request
        const URL: &'static str = "https://polkadot.js.org/";

        /// Time limit for waiting response in ms
        const RESPONSE_TIME_LIMIT: u64 = 500;

        /// Time limit for reading in ms
        const READING_TIME_LIMIT: u64 = 200;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        /// Error while converting Vec to BoundedVec, e.g. Vec is larger than BoundedVec max length
        VecToBoundedVecConvertationError,
        /// Saved chunks limit exceeded
        ChunksLimitExceeded,
    }

    #[derive(Debug)]
    pub enum DataProcessingError {
        /// Error while reading response data
        RequestReadingError,
        /// Error while saving data in chunks
        DataSavingError,
    }

    #[derive(Debug)]
    pub enum HttpRequestError {
        /// Something went wrong when sending http request
        RequestSendingError,
        /// Request status isn't correct, e.g. invalid request id
        RequestBadStatus,
        /// Http response code != 200
        ResponseBadCode,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            let id = match Self::send_http_request() {
                Ok(id) => id,
                Err(e) => {
                    log::error!("Error while sending http request: {:?}", e);
                    return;
                }
            };

            if let Err(e) = Self::read_and_save_response_in_chunks(id, block_number) {
                log::error!("Error while reading or saving http request: {:?}", e);
                return;
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10000)]
        pub fn save_data_chunk(
            origin: T::RuntimeOrigin,
            data_chunk: Vec<u8>,
            block_number: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let bounded_vec: BoundedVec<u8, <T as Config>::MaxDataLen> = data_chunk
                .try_into()
                .map_err(|_| {
                    log::error!("Convertation error");
                    Error::<T>::VecToBoundedVecConvertationError
                })?;

            // Save chunk if current amount of chunks < MaxChunks
            let current_amount = Self::current_amount_of_chunks();
            let amount_limit = <T as Config>::MaxChunks::get();

            if !(current_amount < amount_limit) {
                log::error!("Chunks limit exceeded");
                return Err(Error::<T>::ChunksLimitExceeded.into());
            }

            let k2 = Self::get_max_k2_or_0(block_number);
            let new_k2 = k2.saturating_add(1);

            DataChunks::<T>::insert(block_number, new_k2, bounded_vec);

            CurrentAmountOfChunks::<T>::mutate(|v| *v = v.saturating_add(1));

            log::info!(
                "Saved chunks: {}",
                Self::current_amount_of_chunks()
            );

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::save_data_chunk {
                    data_chunk,
                    block_number,
                } => ValidTransaction::with_tag_prefix("Data chunk")
                    .and_provides((data_chunk, block_number))
                    .propagate(true)
                    .build(),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// return current maximal key2 for StorageDoubleMap
        fn get_max_k2_or_0(k1: BlockNumberFor<T>) -> u64 {
            DataChunks::<T>::iter_prefix(k1)
                .map(|(k2, _)| k2)
                .max()
                .unwrap_or(0)
        }

        fn get_deadline_for(dur: u64) -> Timestamp {
            let now = timestamp();
            let duration = Duration::from_millis(dur);
            let deadline = now.add(duration);
            deadline
        }

        fn send_http_request() -> Result<HttpRequestId, HttpRequestError> {
            log::info!("Sending request...");
            let id = http_request_start("GET", <T as Config>::URL, &[])
                .map_err(|_| HttpRequestError::RequestSendingError)?;
            log::info!("Request was sent successfully, id: {}", id.0);

            let response_deadline = Self::get_deadline_for(<T as Config>::RESPONSE_TIME_LIMIT);

            log::info!("Waiting for request...");
            let response_status = http_response_wait(&[id], Some(response_deadline));

            let response_code = match response_status[0] {
                HttpRequestStatus::Finished(response_code) => {
                    log::info!("Http response code: {}", response_code);
                    response_code
                }
                _ => return Err(HttpRequestError::RequestBadStatus),
            };

            if response_code != 200 {
                return Err(HttpRequestError::ResponseBadCode);
            };

            Ok(id)
        }
        fn read_and_save_response_in_chunks(
            id: HttpRequestId,
            block_number: BlockNumberFor<T>,
        ) -> Result<(), DataProcessingError> {
            let reading_deadline = Self::get_deadline_for(<T as Config>::READING_TIME_LIMIT);

            let mut buff = vec![0; <T as Config>::MaxDataLen::get() as usize];

            // Chunks processing
            loop {
                log::info!("Reading chunk of body request...");
                let bytes_to_read = http_response_read_body(id, &mut buff, Some(reading_deadline))
                    .map_err(|_| DataProcessingError::RequestReadingError)?;

                if bytes_to_read == 0 {
                    return Ok(());
                }

                log::info!(
                    "Chunk was read successfully, bytes to read: {}",
                    bytes_to_read
                );

                let body_as_u8 = &buff[..bytes_to_read as usize];
                let data_chunk = Vec::from(body_as_u8);

                let call = Call::save_data_chunk {
                    data_chunk,
                    block_number,
                };
                SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
                    .map_err(|_| DataProcessingError::DataSavingError)?;
            }
        }
    }
}
