use crate::mock::*;
use frame_support::{assert_noop, assert_ok, storage, traits::Hooks};

#[test]
fn rescue_finality_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Seed stale PendingChange to simulate deadlock
		let pending_change_key = storage::storage_prefix(b"Grandpa", b"PendingChange");
		storage::unhashed::put_raw(&pending_change_key, &[1, 2, 3]);

		let median = 0u64;
		assert_ok!(FinalityRescue::rescue_finality(RuntimeOrigin::root(), median));

		// Verify event was emitted
		let events = System::events();
		assert!(events.iter().any(|e| matches!(
			e.event,
			RuntimeEvent::FinalityRescue(crate::Event::FinalityRescueExecuted {
				block_number: 1,
				median: 0,
				authority_count: 3,
				old_set_id: 0,
				new_set_id: 1,
			})
		)));

		// Verify CurrentSetId was incremented
		let current_set_id_key = storage::storage_prefix(b"Grandpa", b"CurrentSetId");
		let set_id: u64 = storage::unhashed::get_or_default(&current_set_id_key);
		assert_eq!(set_id, 1);

		// Verify LastRescueBlock was set
		assert_eq!(FinalityRescue::last_rescue_block(), Some(1));
	});
}

#[test]
fn rescue_finality_requires_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			FinalityRescue::rescue_finality(RuntimeOrigin::signed(1), 0),
			frame_support::error::BadOrigin
		);
	});
}

#[test]
fn rescue_finality_cooldown_enforced() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0));

		// Try again at block 5 (within cooldown of 10)
		System::set_block_number(5);
		assert_noop!(
			FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0),
			crate::Error::<Test>::CooldownNotElapsed
		);

		// Should work at block 11 (cooldown elapsed)
		System::set_block_number(11);
		assert_ok!(FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0));

		// Verify set_id incremented again
		let current_set_id_key = storage::storage_prefix(b"Grandpa", b"CurrentSetId");
		let set_id: u64 = storage::unhashed::get_or_default(&current_set_id_key);
		assert_eq!(set_id, 2);
	});
}

#[test]
fn rescue_finality_fails_without_authorities() {
	new_test_ext_no_authorities().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0),
			crate::Error::<Test>::NoAuthorities
		);
	});
}

#[test]
fn rescue_finality_works_without_stale_state() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// No stale state seeded - should still work (kill on non-existent key is no-op)
		assert_ok!(FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0));

		// Verify it worked
		assert_eq!(FinalityRescue::last_rescue_block(), Some(1));
	});
}

#[test]
fn rescue_finality_on_finalize_applies_change() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(FinalityRescue::rescue_finality(RuntimeOrigin::root(), 0));

		// PendingChange should be set by schedule_change
		let pending_change_key = storage::storage_prefix(b"Grandpa", b"PendingChange");
		assert!(storage::unhashed::exists(&pending_change_key));

		// Run on_finalize for Grandpa — this processes the pending change
		Grandpa::on_finalize(1);

		// PendingChange should be cleared after on_finalize (delay=0 means same block)
		assert!(!storage::unhashed::exists(&pending_change_key));

		// Authorities should still be the same (we scheduled the same set)
		let authorities = pallet_grandpa::Pallet::<Test>::grandpa_authorities();
		assert_eq!(authorities.len(), 3);
	});
}
