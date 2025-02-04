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
	frame_system,
	polkadot_sdk_frame as frame
};

use frame::traits::AccountIdConversion;
// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use my_pallet::*;


type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;



#[frame::pallet(dev_mode)]
pub mod my_pallet {
	use super::*;

	use frame::{
		prelude::*,
		traits::ExistenceRequirement
	};
	use polkadot_sdk::sp_runtime::transaction_validity::InvalidTransaction;

	pub type Balance = u128;

	#[pallet::storage]
	pub type TotalInsurance<T: Config> = StorageValue<_, Balance>;

	#[pallet::storage]
	pub type Balances<T: Config> = StorageMap<_, _, T::AccountId, Balance>;

	#[pallet::storage]
	pub type LastRequests<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (BalanceOf<T>, BlockNumberFor<T>), ValueQuery>;


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
		NotEnoughFaucetBalance
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		pub fn mint_unsafe(
			origin: T::RuntimeOrigin,
			dest: T::AccountId,
			amount: Balance,
		) -> DispatchResult {
			let _anyone = ensure_signed(origin)?;

			Balances::<T>::mutate(dest, |b| *b = Some(b.unwrap_or(0) + amount));

			TotalInsurance::<T>::mutate(|t| *t = Some(t.unwrap_or(0) + amount));
			Ok(())
		}

		#[pallet::call_index(1)]
		pub fn transfer(
			origin: T::RuntimeOrigin,
			dest: T::AccountId,
			amount: Balance,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let sender_balance = Balances::<T>::get(&sender).ok_or("NonExistentAccount")?;

			sender_balance.checked_sub(amount).ok_or("InsufficientBalance")?;
			
			let reminder = sender_balance - amount;

			Balances::<T>::mutate(dest, |b| *b = Some(b.unwrap_or(0) + amount));
			Balances::<T>::insert(sender, reminder);

			Ok(())
		}

		#[pallet::call_index(2)]
		pub fn token_faucet(
			origin: T::RuntimeOrigin,
			dest: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			ensure_none(origin)?;
			ensure!(amount <= T::FaucetAmount::get(), Error::<T>::AmountTooHigh);

			let (balance, last_time) = LastRequests::<T>::get(&dest);
			let now = frame_system::Pallet::<T>::block_number();
			let period = now - last_time;

			ensure!(period >= T::AccumulationPeriod::get(), Error::<T>::RequestLimitExceeded);

			let total = amount + balance;

			let account_id = Self::account_id();
			let faucet_balance = T::Currency::free_balance(&account_id);

			ensure!(faucet_balance >= amount, Error::<T>::RequestLimitExceeded);

			T::Currency::transfer(&account_id, &dest, amount, ExistenceRequirement::AllowDeath)?;

			LastRequests::<T>::insert(&dest, (total, now));

			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::token_faucet {dest, amount} => {
					ValidTransaction::with_tag_prefix("Faucet")
						.and_provides((dest, amount))
						.propagate(true)
						.build()
				},
				_ => InvalidTransaction::Call.into(),
			}
		}

	}
}


#[cfg(test)]
pub mod runtime {
	use super::*;
	use polkadot_sdk::{
		frame_system,
		pallet_balances,
		frame_support::{derive_impl, parameter_types},
		xcm_emulator::{BlockNumberFor, Test}
	};
	use frame::runtime::prelude::construct_runtime;

	use my_pallet as pallet_currency;

	pub const BLOCKS_PER_HOUR: BlockNumberFor<Runtime> = 60 * 60 / 6;

	construct_runtime!(
		pub enum Runtime {
			System: frame_system,
			Currency: pallet_currency,
		}
	);

	parameter_types! {
    	pub const AccumulationPeriod: BlockNumberFor<Runtime> = BLOCKS_PER_HOUR * 24;
    	pub const FaucetAmount: Balance = 1000;
		pub const FaucetPalletId: PalletId = PalletId(*b"pa/facet");
	}

	type Block = frame_system::mocking::MockBlock<Runtime>;

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Runtime {
		type Block = Block;
		type AccountId = u64;
	}

	impl pallet_currency::Config for Runtime {
		type Currency = pallet_balances::Pallet<Runtime>;
		type AccumulationPeriod = AccumulationPeriod;
		type FaucetAmount = FaucetAmount;
		type PalletId = PalletId;
	}
}

#[cfg(test)]
mod test {
	use super::*;

	use polkadot_sdk::frame_support::assert_ok;
	use frame::testing_prelude::*;

	use crate::runtime::{Runtime, RuntimeOrigin};

	#[test]
	fn first_test() {
		TestState::new_empty().execute_with(|| {
			let account = 1_u64;

			assert_eq!(Balances::<Runtime>::get(&account), None);
			assert_eq!(TotalInsurance::<Runtime>::get(), None);

			assert_ok!(Pallet::<Runtime>::mint_unsafe(
				RuntimeOrigin::signed(account),
				account,
				100
			));

			assert_eq!(Balances::<Runtime>::get(&account), Some(100));
			assert_eq!(TotalInsurance::<Runtime>::get(), Some(100));
		});
	}
}