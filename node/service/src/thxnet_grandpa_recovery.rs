// Copyright 2026 THX Network Contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

//! GRANDPA aux storage recovery for stuck THX Network mainnet nodes.
//!
//! When a node has already imported blocks through the chaotic area (14,205,952–14,206,626)
//! but GRANDPA finality is stuck (set_id mismatch), the hard fork hash matching in
//! `block_import_with_authority_set_hard_forks` won't trigger because blocks are already
//! `InChain`. This module directly rewrites GRANDPA aux storage to correct the state.

use polkadot_primitives::Hash;
use sc_client_api::AuxStore;
use codec::{Decode, Encode};

/// Aux storage keys matching sc-consensus-grandpa's internal constants.
const AUTHORITY_SET_KEY: &[u8] = b"grandpa_voters";
const SET_STATE_KEY: &[u8] = b"grandpa_completed_round";
const VERSION_KEY: &[u8] = b"grandpa_schema_version";
const CURRENT_VERSION: u32 = 3;

/// SCALE-compatible replica of `sc_consensus_grandpa::AuthoritySet<H, N>`.
/// Field order must match exactly for correct decoding/encoding.
#[derive(Debug, Encode, Decode)]
struct AuthoritySetCompat<H, N> {
	current_authorities: Vec<(grandpa_primitives::AuthorityId, u64)>,
	set_id: u64,
	pending_standard_changes: ForkTreeCompat<H, N>,
	pending_forced_changes: Vec<PendingChangeCompat<H, N>>,
	authority_set_changes: Vec<(u64, N)>,
}

/// SCALE-compatible replica of `fork_tree::ForkTree` serialization.
/// ForkTree serializes as (roots: Vec<Node>, best_finalized_number: Option<N>).
#[derive(Debug, Encode, Decode)]
struct ForkTreeCompat<H, N> {
	roots: Vec<ForkTreeNodeCompat<H, N>>,
	best_finalized_number: Option<N>,
}

/// SCALE-compatible replica of `fork_tree::Node`.
#[derive(Debug, Encode, Decode)]
struct ForkTreeNodeCompat<H, N> {
	hash: H,
	number: N,
	data: PendingChangeCompat<H, N>,
	children: Vec<ForkTreeNodeCompat<H, N>>,
}

/// SCALE-compatible replica of `PendingChange<H, N>`.
#[derive(Debug, Encode, Decode)]
struct PendingChangeCompat<H, N> {
	next_authorities: Vec<(grandpa_primitives::AuthorityId, u64)>,
	delay: N,
	canon_height: N,
	canon_hash: H,
	delay_kind: DelayKindCompat<N>,
}

/// SCALE-compatible replica of `DelayKind<N>`.
#[derive(Debug, Encode, Decode)]
enum DelayKindCompat<N> {
	Finalized,
	Best { median_last_finalized: N },
}

/// Reset GRANDPA aux storage for a stuck THX Network node.
///
/// Writes a corrected `AuthoritySet` with the given `target_set_id` and authorities,
/// and deletes the `VoterSetState` so `load_persistent` recreates it fresh.
pub(crate) fn reset_grandpa_state<B: AuxStore>(
	backend: &B,
	target_set_id: u64,
	authorities: &[(grandpa_primitives::AuthorityId, u64)],
	finalized_number: u32,
	// Reserved for future use (e.g. authority_set_changes entries that reference specific hashes).
	_finalized_hash: Hash,
) -> Result<(), sp_blockchain::Error> {
	// Read existing authority set to preserve authority_set_changes (needed for warp sync)
	// and check if recovery already happened (idempotency guard).
	let (authority_set_changes, already_correct) = match backend.get_aux(AUTHORITY_SET_KEY) {
		Ok(Some(raw)) => match AuthoritySetCompat::<Hash, u32>::decode(&mut &raw[..]) {
			Ok(existing) => {
				log::info!(
					"🔧 THX Network: Existing GRANDPA state — set_id={}, authorities={}, changes={}",
					existing.set_id,
					existing.current_authorities.len(),
					existing.authority_set_changes.len(),
				);
				let already = existing.set_id == target_set_id;
				(existing.authority_set_changes, already)
			},
			Err(e) => {
				log::warn!(
					"🔧 THX Network: Failed to decode existing AuthoritySet ({}), using empty changes",
					e
				);
				(Vec::new(), false)
			},
		},
		Ok(None) => {
			log::warn!(
				"🔧 THX Network: No existing AuthoritySet in aux storage, using empty changes"
			);
			(Vec::new(), false)
		},
		Err(e) => {
			log::warn!("🔧 THX Network: Error reading AuthoritySet ({}), using empty changes", e);
			(Vec::new(), false)
		},
	};

	// Idempotency: if the existing set_id already matches the target, the recovery
	// was already applied on a previous startup. Skip to avoid wiping VoterSetState
	// that GRANDPA has been building since the last restart.
	if already_correct {
		log::info!(
			"🔧 THX Network: GRANDPA set_id already {} — skipping recovery (idempotent)",
			target_set_id,
		);
		return Ok(())
	}

	// Build corrected AuthoritySet
	let new_authority_set = AuthoritySetCompat::<Hash, u32> {
		current_authorities: authorities.to_vec(),
		set_id: target_set_id,
		pending_standard_changes: ForkTreeCompat { roots: Vec::new(), best_finalized_number: None },
		pending_forced_changes: Vec::new(),
		authority_set_changes,
	};
	let encoded_authority_set = new_authority_set.encode();

	// Write corrected AuthoritySet and schema version, delete VoterSetState.
	// Deleting SET_STATE_KEY forces `load_persistent` to create a fresh VoterSetState
	// from the authority set, avoiding set_id mismatch errors.
	let version_encoded = CURRENT_VERSION.encode();
	backend.insert_aux(
		&[(AUTHORITY_SET_KEY, &encoded_authority_set[..]), (VERSION_KEY, &version_encoded[..])],
		&[SET_STATE_KEY],
	)?;

	log::info!(
		"🔧 THX Network: Wrote corrected AuthoritySet — set_id={}, authorities={}, \
		 finalized=#{}, deleted VoterSetState for fresh init",
		target_set_id,
		authorities.len(),
		finalized_number,
	);

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::crypto::Ss58Codec;
	use std::{collections::HashMap, sync::Mutex};

	/// In-memory AuxStore for testing.
	struct MockAuxStore {
		data: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
	}

	impl MockAuxStore {
		fn new() -> Self {
			Self { data: Mutex::new(HashMap::new()) }
		}
	}

	impl AuxStore for MockAuxStore {
		fn insert_aux<
			'a,
			'b: 'a,
			'c: 'a,
			I: IntoIterator<Item = &'a (&'c [u8], &'c [u8])>,
			D: IntoIterator<Item = &'a &'b [u8]>,
		>(
			&self,
			insert: I,
			delete: D,
		) -> sp_blockchain::Result<()> {
			let mut data = self.data.lock().unwrap();
			for del_key in delete {
				data.remove(*del_key);
			}
			for (key, value) in insert {
				data.insert(key.to_vec(), value.to_vec());
			}
			Ok(())
		}

		fn get_aux(&self, key: &[u8]) -> sp_blockchain::Result<Option<Vec<u8>>> {
			Ok(self.data.lock().unwrap().get(key).cloned())
		}
	}

	fn test_authorities() -> Vec<(grandpa_primitives::AuthorityId, u64)> {
		let addr = "5DQjEK2cWN2Qnp5sFdJQAoQ5RLaveyCxYpCbc8kWK2mbkrHi";
		vec![(grandpa_primitives::AuthorityId::from_ss58check(addr).unwrap(), 1)]
	}

	#[test]
	fn authority_set_encode_decode_roundtrip() {
		let original = AuthoritySetCompat::<Hash, u32> {
			current_authorities: test_authorities(),
			set_id: 991,
			pending_standard_changes: ForkTreeCompat {
				roots: Vec::new(),
				best_finalized_number: None,
			},
			pending_forced_changes: Vec::new(),
			authority_set_changes: vec![(987, 14_206_555), (988, 14_206_564)],
		};

		let encoded = original.encode();
		let decoded = AuthoritySetCompat::<Hash, u32>::decode(&mut &encoded[..])
			.expect("decode should succeed");

		assert_eq!(decoded.set_id, 991);
		assert_eq!(decoded.current_authorities.len(), 1);
		assert_eq!(decoded.authority_set_changes.len(), 2);
		assert!(decoded.pending_standard_changes.roots.is_empty());
		assert!(decoded.pending_forced_changes.is_empty());
	}

	#[test]
	fn reset_grandpa_state_writes_correct_data() {
		let store = MockAuxStore::new();
		let hash = Hash::default();
		let authorities = test_authorities();

		reset_grandpa_state(&store, 991, &authorities, 14_205_952, hash)
			.expect("reset should succeed");

		// Verify AuthoritySet was written
		let raw = store.get_aux(AUTHORITY_SET_KEY).unwrap().expect("should exist");
		let decoded = AuthoritySetCompat::<Hash, u32>::decode(&mut &raw[..]).unwrap();
		assert_eq!(decoded.set_id, 991);
		assert_eq!(decoded.current_authorities, authorities);

		// Verify VoterSetState was deleted
		assert!(store.get_aux(SET_STATE_KEY).unwrap().is_none());

		// Verify schema version
		let ver_raw = store.get_aux(VERSION_KEY).unwrap().expect("should exist");
		let ver = u32::decode(&mut &ver_raw[..]).unwrap();
		assert_eq!(ver, CURRENT_VERSION);
	}

	#[test]
	fn reset_is_idempotent() {
		let store = MockAuxStore::new();
		let hash = Hash::default();
		let authorities = test_authorities();

		// First call — performs reset
		reset_grandpa_state(&store, 991, &authorities, 14_205_952, hash)
			.expect("first reset should succeed");

		// Simulate GRANDPA writing VoterSetState after recovery
		store
			.insert_aux(&[(&SET_STATE_KEY[..], b"fake_voter_state" as &[u8])], &[])
			.unwrap();
		assert!(store.get_aux(SET_STATE_KEY).unwrap().is_some());

		// Second call — should skip (idempotent), preserving VoterSetState
		reset_grandpa_state(&store, 991, &authorities, 14_205_952, hash)
			.expect("second reset should succeed");

		// VoterSetState must still exist (not wiped by second call)
		assert!(store.get_aux(SET_STATE_KEY).unwrap().is_some());
	}

	#[test]
	fn reset_preserves_authority_set_changes() {
		let store = MockAuxStore::new();
		let hash = Hash::default();
		let authorities = test_authorities();

		// Pre-populate with an AuthoritySet that has authority_set_changes
		let existing = AuthoritySetCompat::<Hash, u32> {
			current_authorities: authorities.clone(),
			set_id: 987,
			pending_standard_changes: ForkTreeCompat {
				roots: Vec::new(),
				best_finalized_number: None,
			},
			pending_forced_changes: Vec::new(),
			authority_set_changes: vec![(100, 1000), (200, 2000), (987, 14_205_952)],
		};
		let encoded = existing.encode();
		store.insert_aux(&[(AUTHORITY_SET_KEY, &encoded[..])], &[]).unwrap();

		// Reset should preserve authority_set_changes
		reset_grandpa_state(&store, 991, &authorities, 14_205_952, hash)
			.expect("reset should succeed");

		let raw = store.get_aux(AUTHORITY_SET_KEY).unwrap().expect("should exist");
		let decoded = AuthoritySetCompat::<Hash, u32>::decode(&mut &raw[..]).unwrap();
		assert_eq!(decoded.set_id, 991);
		assert_eq!(
			decoded.authority_set_changes,
			vec![(100, 1000), (200, 2000), (987, 14_205_952)]
		);
	}
}
