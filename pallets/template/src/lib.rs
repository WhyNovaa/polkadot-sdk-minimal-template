//! A shell pallet built with [`frame`].
//!
//! To get started with this pallet, try implementing the guide in
//! <https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html>

#![cfg_attr(not(feature = "std"), no_std)]

use polkadot_sdk::{frame_system, polkadot_sdk_frame as frame};
// Re-export all pallet parts, this is needed to properly import the pallet into the runtime.
pub use my_pallet::*;
#[frame::pallet(dev_mode)]
pub mod my_pallet {
	use super::*;
	use frame::prelude::*;

	pub type Balance = u128;

	#[pallet::storage]
	pub type TotalInsurance<T: Config> = StorageValue<_, Balance>;

	#[pallet::storage]
	pub type Balances<T: Config> = StorageMap<_, _, T::AccountId, Balance>;

	#[pallet::config]
	pub trait Config: polkadot_sdk::frame_system::Config {}
	

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
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
	}
}

mod runtime {
	use super::*;
	use polkadot_sdk::frame_support::derive_impl;
	use polkadot_sdk::frame_system;

	use my_pallet as pallet_currency;
	use frame::runtime::prelude::construct_runtime;

	construct_runtime!(
		pub enum Runtime {
			System: frame_system,
			Currency: pallet_currency,
		}
	);

	type Block = frame_system::mocking::MockBlock<Runtime>;
	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Runtime {
		type Block = Block;
		type AccountId = u64;
	}
	
	impl pallet_currency::Config for Runtime {}
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