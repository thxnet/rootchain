#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{pallet_prelude::*, storage};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Zero;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_grandpa::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Minimum number of blocks between rescue calls.
		#[pallet::constant]
		type RescueCooldown: Get<Self::BlockNumber>;
	}

	#[pallet::storage]
	#[pallet::getter(fn last_rescue_block)]
	pub type LastRescueBlock<T: Config> = StorageValue<_, T::BlockNumber>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Finality rescue executed successfully.
		FinalityRescueExecuted {
			block_number: T::BlockNumber,
			median: T::BlockNumber,
			authority_count: u32,
			old_set_id: u64,
			new_set_id: u64,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Cooldown period has not elapsed since last rescue.
		CooldownNotElapsed,
		/// No GRANDPA authorities found.
		NoAuthorities,
		/// Failed to schedule authority change.
		ScheduleChangeFailed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Emergency rescue of GRANDPA finality.
		///
		/// Clears stale GRANDPA state (PendingChange, NextForced, Stalled),
		/// schedules a forced authority change with the current authorities,
		/// and increments CurrentSetId.
		///
		/// `median` must be the last finalized block number, obtained from
		/// `chain_getFinalizedHead` RPC.
		///
		/// Can only be called by root (via sudo).
		#[pallet::call_index(0)]
		#[pallet::weight(T::DbWeight::get().reads_writes(5, 6))]
		pub fn rescue_finality(
			origin: OriginFor<T>,
			median: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let block_number = <frame_system::Pallet<T>>::block_number();

			// Cooldown check
			if let Some(last) = LastRescueBlock::<T>::get() {
				ensure!(
					block_number >= last + T::RescueCooldown::get(),
					Error::<T>::CooldownNotElapsed
				);
			}

			// Storage key prefixes for GRANDPA internal state
			let current_set_id_key = storage::storage_prefix(b"Grandpa", b"CurrentSetId");
			let pending_change_key = storage::storage_prefix(b"Grandpa", b"PendingChange");
			let next_forced_key = storage::storage_prefix(b"Grandpa", b"NextForced");
			let stalled_key = storage::storage_prefix(b"Grandpa", b"Stalled");

			// Step 1: Clear all stale GRANDPA state
			if storage::unhashed::exists(&pending_change_key) {
				log::info!(
					target: "runtime::finality-rescue",
					"Clearing stale PendingChange",
				);
			}
			storage::unhashed::kill(&pending_change_key);
			storage::unhashed::kill(&next_forced_key);
			storage::unhashed::kill(&stalled_key);

			// Step 2: Get current authorities
			let authorities = pallet_grandpa::Pallet::<T>::grandpa_authorities();
			ensure!(!authorities.is_empty(), Error::<T>::NoAuthorities);
			let authority_count = authorities.len() as u32;

			// Step 3: Schedule forced authority change
			// With delay=0, on_finalize in the same block will:
			//   1. Emit ForcedChange(median, ScheduledChange) consensus log
			//   2. Apply the change (set_grandpa_authorities + kill PendingChange)
			pallet_grandpa::Pallet::<T>::schedule_change(authorities, Zero::zero(), Some(median))
				.map_err(|e| {
					log::error!(
						target: "runtime::finality-rescue",
						"schedule_change failed: {:?}",
						e,
					);
					Error::<T>::ScheduleChangeFailed
				})?;

			// Step 4: Increment CurrentSetId
			// on_finalize does NOT increment CurrentSetId for forced changes,
			// but the GRANDPA client does: new_set_id = self.set_id + 1
			let old_set_id: u64 = storage::unhashed::get_or_default(&current_set_id_key);
			let new_set_id: u64 = old_set_id + 1;
			storage::unhashed::put(&current_set_id_key, &new_set_id);

			// Step 5: Record and emit
			LastRescueBlock::<T>::put(block_number);

			log::info!(
				target: "runtime::finality-rescue",
				"Finality rescue applied at block #{:?}: ForcedChange(median={:?}) \
				 scheduled with {} authorities, CurrentSetId {} -> {}",
				block_number,
				median,
				authority_count,
				old_set_id,
				new_set_id,
			);

			Self::deposit_event(Event::FinalityRescueExecuted {
				block_number,
				median,
				authority_count,
				old_set_id,
				new_set_id,
			});

			// Emergency operation - no fee
			Ok(Pays::No.into())
		}
	}
}
