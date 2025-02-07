//! A shell pallet built with [`frame`].
//!
//! To get started with this pallet, try implementing the guide in
//! <https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html>

#![cfg_attr(not(feature = "std"), no_std)]

use polkadot_sdk::{
    frame_support::{
        traits::{Currency, Get},
        PalletId,
    },
    frame_system, polkadot_sdk_frame as frame,
};

use frame::traits::AccountIdConversion;
// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use my_pallet::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame::pallet]
pub mod my_pallet {
    use super::*;

    use frame::{prelude::*, traits::ExistenceRequirement};
    use polkadot_sdk::{
        sp_arithmetic::traits::Saturating, sp_runtime::transaction_validity::InvalidTransaction,
    };

    #[pallet::storage]
    pub type LastRequests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (BalanceOf<T>, BlockNumberFor<T>),
        ValueQuery,
    >;

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        type Currency: Currency<Self::AccountId>;

        #[pallet::constant]
        type AccumulationPeriod: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type FaucetAmount: Get<BalanceOf<Self>>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;
    }

    impl<T: Config> Pallet<T> {
        /// The account ID to transfer faucet amount to user.
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        AmountTooHigh,
        RequestLimitExceeded,
        NotEnoughFaucetBalance,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10000)]
        pub fn token_faucet(
            origin: T::RuntimeOrigin,
            dest: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(amount <= T::FaucetAmount::get(), Error::<T>::AmountTooHigh);

            let (balance, last_time) = LastRequests::<T>::get(&dest);
            let now = frame_system::Pallet::<T>::block_number();
            let period = now.saturating_sub(last_time);

            let (total, now) = if period >= T::AccumulationPeriod::get() {
                (amount, now)
            } else {
                (balance + amount, last_time)
            };

            ensure!(
                total <= T::FaucetAmount::get(),
                Error::<T>::RequestLimitExceeded
            );

            let account_id = Self::account_id();
            let faucet_balance = T::Currency::free_balance(&account_id);

            ensure!(faucet_balance >= amount, Error::<T>::NotEnoughFaucetBalance);

            T::Currency::transfer(&account_id, &dest, amount, ExistenceRequirement::AllowDeath)?;

            LastRequests::<T>::insert(&dest, (total, now));

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10000)]
        pub fn refill_pallet(origin: T::RuntimeOrigin, amount: BalanceOf<T>) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            let account_id = Self::account_id();

            T::Currency::transfer(
                &sender,
                &account_id,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10000)]
        pub fn set_balance(
            origin: T::RuntimeOrigin,
            who: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            T::Currency::deposit_into_existing(&who, amount)?;

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::token_faucet { dest, amount } => ValidTransaction::with_tag_prefix("Faucet")
                    .and_provides((dest, amount))
                    .propagate(true)
                    .build(),
                Call::set_balance { who, amount } => ValidTransaction::with_tag_prefix("Faucet")
                    .and_provides((who, amount))
                    .propagate(true)
                    .build(),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}
