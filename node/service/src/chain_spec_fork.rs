// Copyright 2017-2024 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! State-filtering helpers for fork-based chain specs.
//!
//! [`filter_forked_storage`] removes consensus- and block-execution-transient
//! keys from a raw genesis storage snapshot so the resulting chain spec starts
//! from a clean, deterministic state.
//!
//! ## Drop logic (three tiers, evaluated in order)
//!
//! 1. **Pallet-level** (16-byte twox_128 prefix): any key whose first 16 bytes
//!    match `twox_128(pallet_name)` for a listed pallet is dropped entirely.
//! 2. **Item-level** (32-byte prefix): any key whose first 32 bytes match
//!    `twox_128(pallet) || twox_128(item)` for a listed (pallet, item) pair
//!    is dropped.
//! 3. **Well-known exact-match**: `:extrinsic_index`, `:intrablock_entropy`.
//!
//! Everything else — including `System.Account`, `:code`, `:heappages` — is
//! preserved verbatim. `children_default` is passed through untouched.

use sp_core::hashing::twox_128;

// ---------------------------------------------------------------------------
// Fork-genesis builder imports (cfg-gated to polkadot-native feature)
// ---------------------------------------------------------------------------
//
// All runtime-specific types and the keyring live behind `polkadot-native`
// because `thxnet-testnet-runtime` and `polkadot-runtime-constants` are only
// compiled when that feature is enabled (see Cargo.toml feature matrix).
// The bare `use sp_core::hashing::twox_128` above is feature-agnostic and
// must remain outside any cfg block.

#[cfg(feature = "polkadot-native")]
use {
	// Key types
	grandpa::AuthorityId as GrandpaId,
	pallet_im_online::sr25519::AuthorityId as ImOnlineId,
	// Staking pallet
	pallet_staking::Forcing,
	// ParaId needed for --register-leafchain signature
	polkadot_primitives::{AccountId, AssignmentId, Id as ParaId, ValidatorId},
	// Runtime aliases
	polkadot_runtime as polkadot,
	// DOT constant (same source as chain_spec.rs line 35)
	polkadot_runtime_constants::currency::UNITS as DOT,
	// ParaGenesisArgs — pallet_paras GenesisConfig element type
	polkadot_runtime_parachains::paras::ParaGenesisArgs,
	sp_authority_discovery::AuthorityId as AuthorityDiscoveryId,
	sp_consensus_babe::AuthorityId as BabeId,
	// Keyring
	sp_keyring::Ed25519Keyring,
	sp_keyring::Sr25519Keyring,
	// Numeric helpers
	sp_runtime::Perbill,
	thxnet_runtime as thxnet,
	thxnet_testnet_runtime as thxnet_testnet,
};

// ---------------------------------------------------------------------------
// DROP table — pallet-level (whole-pallet wipe)
// ---------------------------------------------------------------------------

/// Pallets whose *entire* storage subtree is dropped.
///
/// Each entry is a raw `&[u8]` pallet name; the 16-byte twox_128 prefix is
/// computed at runtime inside `filter_forked_storage`.
///
/// # W8 addition — `Paras`
///
/// Dropping all livenet Paras.* state forces the fresh GenesisConfig's paras
/// vec (populated via `--register-leafchain`) to be the sole source of truth
/// for para registrations. Without this drop, livenet's 5+ paraIds leak into
/// `Paras.Parachains` causing scheduler to allocate N cores with N > validator
/// count, leaving empty backing groups and periodic rotation outages.
///
/// # Contract-alignment
///
/// Every name below is verified as a pallet identifier in *both* runtime
/// `construct_runtime!` macros (runtimes are identical in this respect):
///   - `runtime/thxnet-testnet/src/lib.rs` line 1611
///   - `runtime/thxnet/src/lib.rs`          line 1611
///
/// No pallet is absent from either runtime. No divergence between runtimes.
static DROP_PALLETS: &[&[u8]] = &[
	b"AuthorityDiscovery",
	b"Babe",
	b"Dmp",
	b"ElectionProviderMultiPhase",
	b"FastUnstake",
	b"Grandpa",
	b"Historical",
	b"Hrmp",
	b"ImOnline",
	b"Offences",
	b"ParaInclusion",
	b"ParaInherent",
	b"ParaScheduler",
	b"ParaSessionInfo",
	b"Paras",
	b"ParasDisputes",
	b"Session",
	b"Staking",
	b"Ump",
	b"VoterList",
];

// ---------------------------------------------------------------------------
// DROP table — item-level (specific storage items)
// ---------------------------------------------------------------------------

/// Specific storage items to drop, given as `(pallet_name, item_name)` pairs.
///
/// The 32-byte prefix `twox_128(pallet) || twox_128(item)` is computed at
/// runtime inside `filter_forked_storage`.
///
/// # W8: Dmp + Hrmp moved to DROP_PALLETS
///
/// Previously `(b"Dmp", b"DownwardMessageQueues")`, `(b"Hrmp", b"HrmpChannelContents")`,
/// and `(b"Hrmp", b"HrmpWatermarks")` were item-level drops. This was insufficient:
/// `Dmp.DownwardMessageQueueHeads` preserved the livenet MQC head, causing the
/// collator's first proposal to panic at
/// `cumulus/pallets/parachain-system/src/lib.rs:861` (MQC-head mismatch). Promoted
/// both pallets to whole-pallet drops in `DROP_PALLETS`.
static DROP_ITEMS: &[(&[u8], &[u8])] = &[
	// System — block-scoped transient items.
	// Note: System.ExtrinsicData and System.ExtrinsicCount are the only
	// two System storage items prefixed with "Extrinsic" (verified against
	// frame-system polkadot-v0.9.40 source at
	// ~/.cargo/git/checkouts/substrate-7e08433d4c370a21/ba87188/frame/system/src/lib.rs).
	// ExtrinsicsRoot is a block HEADER field, not a pallet storage item.
	// ExtrinsicSuccess is an event, not storage. Neither appears as a pallet
	// storage declaration in the frame-system source for this version.
	// frame-system transient; killed in on_initialize/on_finalize — drop for safety
	(b"System", b"AllExtrinsicsLen"),
	(b"System", b"BlockHash"),
	// frame-system transient; killed in on_initialize/on_finalize — drop for safety
	(b"System", b"BlockWeight"),
	(b"System", b"EventCount"),
	(b"System", b"Events"),
	(b"System", b"ExecutionPhase"),
	(b"System", b"ExtrinsicCount"),
	(b"System", b"ExtrinsicData"),
	(b"System", b"LastRuntimeUpgrade"),
	(b"System", b"Number"),
	// Timestamp — always re-derived on first block
	(b"Timestamp", b"DidUpdate"),
	(b"Timestamp", b"Now"),
];

// ---------------------------------------------------------------------------
// DROP table — well-known exact-match keys
// ---------------------------------------------------------------------------

/// Bare well-known keys dropped by exact equality (not prefix).
static DROP_EXACT: &[&[u8]] = &[b":extrinsic_index", b":intrablock_entropy"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Remove consensus- and block-execution-transient storage entries from a raw
/// genesis `Storage`, returning the cleaned copy.
///
/// # Guarantees
///
/// - Pure: no I/O, no logging, no global mutation.
/// - Infallible: always returns a valid `Storage`.
/// - `children_default` is forwarded byte-for-byte.
pub fn filter_forked_storage(storage: sp_core::storage::Storage) -> sp_core::storage::Storage {
	// Compute pallet-level 16-byte prefixes.
	// Allocated once per call; the function is a one-shot fork utility, not
	// a hot path, so the allocation cost is immaterial.
	let pallet_prefixes: Vec<[u8; 16]> = DROP_PALLETS.iter().map(|name| twox_128(name)).collect();

	// Compute item-level 32-byte prefixes.
	let item_prefixes: Vec<[u8; 32]> = DROP_ITEMS
		.iter()
		.map(|(pallet, item)| {
			let mut prefix = [0u8; 32];
			prefix[..16].copy_from_slice(&twox_128(pallet));
			prefix[16..].copy_from_slice(&twox_128(item));
			prefix
		})
		.collect();

	// Filter top-level storage.
	//
	// A key is DROPPED if ANY of the following hold:
	//   (a) its first 16 bytes match a pallet prefix, OR
	//   (b) its first 32 bytes match an item prefix, OR
	//   (c) it exactly equals a well-known drop key.
	//
	// Otherwise it is KEPT.
	let top = storage
		.top
		.into_iter()
		.filter(|(key, _value)| !should_drop(key, &pallet_prefixes, &item_prefixes))
		.collect();

	sp_core::storage::Storage { top, children_default: storage.children_default }
}

// ---------------------------------------------------------------------------
// Fork-genesis builder — public API (polkadot-native only)
// ---------------------------------------------------------------------------

/// The canonical authority tuple shared by testnet and mainnet fork-genesis builders.
///
/// Field layout:
///   0 = stash  AccountId (Sr25519)
///   1 = controller AccountId (Sr25519) — stash == controller in dev/fork scenarios
///   2 = BabeId   (Sr25519)
///   3 = GrandpaId (Ed25519)
///   4 = ImOnlineId (Sr25519)
///   5 = ValidatorId (Sr25519)
///   6 = AssignmentId (Sr25519)
///   7 = AuthorityDiscoveryId (Sr25519)
#[cfg(feature = "polkadot-native")]
pub type AuthorityTuple = (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ImOnlineId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
);

/// Build an authority tuple for a given Sr25519 + Ed25519 keyring pair.
///
/// Stash == Controller (same Sr25519 account).
/// All keys use bare `//Name` derivation (no sub-derivation).
#[cfg(feature = "polkadot-native")]
fn authority_tuple_from_keyrings(sr: Sr25519Keyring, ed: Ed25519Keyring) -> AuthorityTuple {
	(
		sr.to_account_id().into(), // stash
		sr.to_account_id().into(), // controller == stash
		sr.public().into(),        // BabeId
		ed.public().into(),        // GrandpaId
		sr.public().into(),        // ImOnlineId
		sr.public().into(),        // ValidatorId
		sr.public().into(),        // AssignmentId
		sr.public().into(),        // AuthorityDiscoveryId
	)
}

/// Returns the standard Alice / Bob / Charlie dev authority set.
///
/// All keys use bare `//Alice`, `//Bob`, `//Charlie` seeds — no sub-derivation.
/// Stash == Controller for each authority.
#[cfg(feature = "polkadot-native")]
pub fn dev_authority_set() -> Vec<AuthorityTuple> {
	vec![
		authority_tuple_from_keyrings(Sr25519Keyring::Alice, Ed25519Keyring::Alice),
		authority_tuple_from_keyrings(Sr25519Keyring::Bob, Ed25519Keyring::Bob),
		authority_tuple_from_keyrings(Sr25519Keyring::Charlie, Ed25519Keyring::Charlie),
	]
}

/// Assemble a `thxnet_testnet::GenesisConfig` from parameterized inputs.
///
/// This mirrors `thxnet_testnet_config_genesis` in `chain_spec.rs` field-by-field,
/// replacing hardcoded hex keys with the parameterized `authorities` vector and
/// the hardcoded sudo key with `root_key`.
///
/// # Parameters
///
/// - `wasm_binary`: the compiled runtime WASM blob.
/// - `authorities`: authority set; each element is an [`AuthorityTuple`].
/// - `root_key`: the `AccountId` to set as sudo root.
/// - `extra_endowed`: additional `(AccountId, balance)` pairs beyond the
///   authority set's own endowment.
/// - `paras_to_register`: per-para registration `(ParaId, ParaGenesisArgs)` entries.
///   Each entry causes `pallet_paras::GenesisConfig::build()` to call
///   `initialize_para_now(id, args)` which writes `Parachains`, `Heads`,
///   `CurrentCodeHash`, `CodeByHash`, `CodeByHashRefs`, and `ParaLifecycles`.
///
/// # Invariants
///
/// - Each authority's stash account receives `ENDOWED` (20 * DOT) tokens.
/// - The staking stash receives `STASH` (ENDOWED / 2 = 10 * DOT).
/// - `validator_count` and `minimum_validator_count` both equal `authorities.len()`.
/// - Session keys are built from the parameterized authority fields (not hardcoded hex).
#[cfg(feature = "polkadot-native")]
pub fn assemble_thxnet_testnet_fork_genesis(
	wasm_binary: &[u8],
	authorities: Vec<AuthorityTuple>,
	root_key: AccountId,
	extra_endowed: Vec<(AccountId, u128)>,
	paras_to_register: Vec<(ParaId, ParaGenesisArgs)>,
) -> thxnet_testnet::GenesisConfig {
	const ENDOWED: u128 = 20 * DOT;
	const STASH: u128 = ENDOWED / 2;

	let validator_count = authorities.len() as u32;

	thxnet_testnet::GenesisConfig {
		system: thxnet_testnet::SystemConfig { code: wasm_binary.to_vec() },
		balances: thxnet_testnet::BalancesConfig {
			balances: extra_endowed
				.into_iter()
				.chain(authorities.iter().map(|a| (a.0.clone(), ENDOWED)))
				.collect(),
		},
		indices: thxnet_testnet::IndicesConfig { indices: vec![] },
		session: thxnet_testnet::SessionConfig {
			keys: authorities
				.iter()
				.map(|a| {
					(
						a.0.clone(),
						a.0.clone(),
						crate::chain_spec::thxnet_testnet_session_keys(
							a.2.clone(),
							a.3.clone(),
							a.4.clone(),
							a.5.clone(),
							a.6.clone(),
							a.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: thxnet_testnet::StakingConfig {
			validator_count,
			minimum_validator_count: validator_count,
			stakers: authorities
				.iter()
				.map(|a| (a.0.clone(), a.1.clone(), STASH, thxnet_testnet::StakerStatus::Validator))
				.collect(),
			invulnerables: authorities.iter().map(|a| a.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: thxnet_testnet::SudoConfig { key: Some(root_key) },
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: thxnet_testnet::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: thxnet_testnet::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: thxnet_testnet::BabeConfig {
			// Authorities are populated by the Session pallet at genesis — leave empty here.
			// epoch_config must be set to the runtime's genesis epoch configuration constant.
			authorities: Default::default(),
			epoch_config: Some(thxnet_testnet::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: thxnet_testnet::AuthorityDiscoveryConfig { keys: vec![] },
		claims: thxnet_testnet::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: thxnet_testnet::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: thxnet_testnet::ConfigurationConfig {
			config: crate::chain_spec::default_parachains_host_configuration(),
		},
		paras: thxnet_testnet::ParasConfig { paras: paras_to_register },
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

/// Assemble a `thxnet::GenesisConfig` (mainnet) from parameterized inputs.
///
/// This mirrors `thxnet_mainnet_config_genesis` in `chain_spec.rs` field-by-field,
/// replacing hardcoded hex keys with the parameterized `authorities` vector and
/// the hardcoded sudo key with `root_key`.
///
/// # Parameters
///
/// - `wasm_binary`: the compiled runtime WASM blob.
/// - `authorities`: authority set; each element is an [`AuthorityTuple`].
/// - `root_key`: the `AccountId` to set as sudo root.
/// - `extra_endowed`: additional `(AccountId, balance)` pairs beyond the
///   authority set's own endowment.
///
/// # Invariants
///
/// - Each authority's stash account receives `ENDOWED` (20 * DOT) tokens.
/// - The staking stash receives `STASH` (ENDOWED / 2 = 10 * DOT).
/// - `validator_count` and `minimum_validator_count` both equal `authorities.len()`.
/// - `invulnerables` is empty (matches mainnet livenet — production set manages
///   invulnerability separately).
/// - `babe.epoch_config` uses `polkadot::BABE_GENESIS_EPOCH_CONFIG` (verified at
///   `chain_spec.rs:813`; mainnet runtime uses polkadot constants, not thxnet-testnet).
/// - Session keys are built from the parameterized authority fields (not hardcoded hex).
#[cfg(feature = "polkadot-native")]
pub fn assemble_thxnet_mainnet_fork_genesis(
	wasm_binary: &[u8],
	authorities: Vec<AuthorityTuple>,
	root_key: AccountId,
	extra_endowed: Vec<(AccountId, u128)>,
	paras_to_register: Vec<(ParaId, ParaGenesisArgs)>,
) -> thxnet::GenesisConfig {
	const ENDOWED: u128 = 20 * DOT;
	const STASH: u128 = ENDOWED / 2;

	let validator_count = authorities.len() as u32;

	thxnet::GenesisConfig {
		system: thxnet::SystemConfig { code: wasm_binary.to_vec() },
		balances: thxnet::BalancesConfig {
			balances: extra_endowed
				.into_iter()
				.chain(authorities.iter().map(|a| (a.0.clone(), ENDOWED)))
				.collect(),
		},
		indices: thxnet::IndicesConfig { indices: vec![] },
		session: thxnet::SessionConfig {
			keys: authorities
				.iter()
				.map(|a| {
					(
						a.0.clone(),
						a.0.clone(),
						crate::chain_spec::thxnet_session_keys(
							a.2.clone(),
							a.3.clone(),
							a.4.clone(),
							a.5.clone(),
							a.6.clone(),
							a.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: thxnet::StakingConfig {
			validator_count,
			minimum_validator_count: validator_count,
			stakers: authorities
				.iter()
				.map(|a| (a.0.clone(), a.1.clone(), STASH, thxnet::StakerStatus::Validator))
				.collect(),
			// invulnerables is empty on mainnet livenet (chain_spec.rs:797).
			// Production validator set is managed separately; fork genesis inherits that policy.
			invulnerables: Vec::new(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: thxnet::SudoConfig { key: Some(root_key) },
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: thxnet::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: thxnet::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: thxnet::BabeConfig {
			// Authorities are populated by the Session pallet at genesis — leave empty here.
			// epoch_config uses polkadot::BABE_GENESIS_EPOCH_CONFIG per livenet chain_spec.rs:813.
			authorities: Default::default(),
			epoch_config: Some(polkadot::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: thxnet::AuthorityDiscoveryConfig { keys: vec![] },
		claims: thxnet::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: thxnet::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: thxnet::ConfigurationConfig {
			config: crate::chain_spec::default_parachains_host_configuration(),
		},
		paras: thxnet::ParasConfig { paras: paras_to_register },
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

// ---------------------------------------------------------------------------
// ParaScheduler post-build fix-up
// ---------------------------------------------------------------------------

/// Post-build override of `ParaScheduler.{ValidatorGroups, AvailabilityCores,
/// SessionStartBlock}` to compensate for the genesis-build ordering hazard in
/// `construct_runtime!` where `Session` (decl-order 9) runs before `Paras`
/// (decl-order 56).
///
/// # Why this exists
///
/// During `GenesisConfig::build_storage()`, pallet_session's genesis build
/// triggers the `SessionHandler` cascade (Initializer is one of the session
/// handlers via `SessionKeys.para_validator`). Initializer runs
/// `apply_new_session(0, validators, queued)` which calls
/// `scheduler::Pallet::initializer_on_new_session(&notification)`. At that
/// moment, `pallet_paras::build()` has NOT yet run (decl-order 56 > 9), so
/// `paras::Pallet::parachains()` returns `[]`. Scheduler then writes
/// `ValidatorGroups = Vec::new()` and `AvailabilityCores = Vec::new()`.
///
/// Later, `pallet_paras::build()` runs and (if `paras_to_register` is
/// non-empty) writes `Parachains = [id1, id2, ...]`, `Heads`, `CurrentCodeHash`,
/// etc. But `ValidatorGroups` is already frozen at empty until the next
/// session change — which in a local devnet on BABE takes a full epoch (often
/// 1+ hour wall-clock) and is not practical for cross-chain liveness testing.
///
/// # What this does
///
/// Reads `Session.Validators` (count only) and `Paras.Parachains` (count
/// only) from the merged storage, applies the shuffle formula from
/// `runtime/parachains/src/scheduler.rs:242-299`, and overwrites the three
/// scheduler storage entries with the values that *would* have been written
/// had the two pallets' genesis builds been run in the correct order.
///
/// # Invariants
///
/// - `parathread_cores = 0` (matches `default_parachains_host_configuration()`).
/// - `n_cores = max(n_parachains + parathread_cores, 0) = n_parachains`.
/// - `SessionStartBlock = 0` (fork-genesis always starts at block 0).
/// - If `Session.Validators` is missing, returns Err.
/// - If `Paras.Parachains` is missing, treats as 0 parachains (no-op overrides).
///
/// # Safety
///
/// This does NOT touch any non-ParaScheduler storage. It does NOT re-run
/// runtime logic. The manual SCALE encoding mirrors the exact bytes the
/// runtime's scheduler would have emitted.
pub fn fix_para_scheduler_state(
	storage: &mut sp_core::storage::Storage,
) -> Result<(), String> {
	use codec::{Compact, Decode, Encode};

	let key_session_validators: Vec<u8> =
		[twox_128(b"Session"), twox_128(b"Validators")].concat();
	let key_paras_parachains: Vec<u8> =
		[twox_128(b"Paras"), twox_128(b"Parachains")].concat();
	let key_sched_groups: Vec<u8> =
		[twox_128(b"ParaScheduler"), twox_128(b"ValidatorGroups")].concat();
	let key_sched_cores: Vec<u8> =
		[twox_128(b"ParaScheduler"), twox_128(b"AvailabilityCores")].concat();
	let key_sched_ssb: Vec<u8> =
		[twox_128(b"ParaScheduler"), twox_128(b"SessionStartBlock")].concat();

	// Decode only the compact length prefix of each Vec; we don't need the actual
	// values (validator identities / para ids) for the scheduler formula — only
	// their counts.
	let n_validators: usize = {
		let bytes = storage
			.top
			.get(&key_session_validators)
			.ok_or_else(|| "Session.Validators missing from merged storage".to_string())?;
		let mut input: &[u8] = &bytes[..];
		Compact::<u32>::decode(&mut input)
			.map_err(|e| format!("decode Session.Validators length prefix: {e}"))?
			.0 as usize
	};

	let n_parachains: usize = if let Some(bytes) = storage.top.get(&key_paras_parachains) {
		let mut input: &[u8] = &bytes[..];
		Compact::<u32>::decode(&mut input)
			.map_err(|e| format!("decode Paras.Parachains length prefix: {e}"))?
			.0 as usize
	} else {
		0
	};

	let parathread_cores: u32 = 0;
	let n_cores: usize =
		std::cmp::max(n_parachains as u32 + parathread_cores, 0) as usize;

	// Mirror runtime/parachains/src/scheduler.rs:268-299 exactly.
	// ValidatorGroups is SCALE `Vec<Vec<ValidatorIndex>>` where `ValidatorIndex(u32)`.
	// Since `ValidatorIndex` is a newtype wrapping `u32`, its SCALE bytes are the
	// same as `u32`, so we can encode as `Vec<Vec<u32>>`.
	let groups: Vec<Vec<u32>> = if n_cores == 0 || n_validators == 0 {
		Vec::new()
	} else {
		let base = n_validators / n_cores;
		let larger = n_validators % n_cores;
		let mut g: Vec<Vec<u32>> = Vec::with_capacity(n_cores);
		for i in 0..larger {
			let off = (base + 1) * i;
			g.push((0..base + 1).map(|j| (off + j) as u32).collect());
		}
		for i in 0..(n_cores - larger) {
			let off = larger * (base + 1) + i * base;
			g.push((0..base).map(|j| (off + j) as u32).collect());
		}
		g
	};

	// AvailabilityCores: `Vec<Option<CoreOccupied>>`. All None at genesis.
	// SCALE-encode manually: compact-length-prefix + n_cores × 0x00 (Option::None).
	let cores_encoded: Vec<u8> = {
		let mut out = Compact(n_cores as u32).encode();
		out.extend(core::iter::repeat(0u8).take(n_cores));
		out
	};

	// SessionStartBlock: BlockNumber = u32 = 0 at genesis.
	let ssb_encoded: Vec<u8> = 0u32.encode();

	let groups_encoded = groups.encode();

	let _ = storage.top.insert(key_sched_groups, groups_encoded);
	let _ = storage.top.insert(key_sched_cores, cores_encoded);
	let _ = storage.top.insert(key_sched_ssb, ssb_encoded);

	// W8b — also patch ParaSessionInfo.Sessions(0). The session_info cascade
	// captured scheduler state + HostConfiguration at cascade-time (both empty),
	// so the stored SessionInfo has validator_groups=[], n_cores=0, and all
	// config-derived approval-voting fields zeroed. Collator-protocol then
	// emits "no validators assigned to core" and refuses to advertise
	// collations. Fix by decoding Sessions(0), patching its fields from our
	// now-correct scheduler state + the host configuration we control, and
	// re-encoding.
	#[cfg(feature = "polkadot-native")]
	fix_para_session_info_session_zero(storage, n_cores as u32, groups.clone())?;

	Ok(())
}

/// Patch `ParaSessionInfo.Sessions(0)` to match the corrected scheduler state.
///
/// See the comment inside [`fix_para_scheduler_state`] for why this is needed.
///
/// # Fields patched
///
/// - `validator_groups`: [[0..n_validators-1]]-style groups from the scheduler
///   shuffle (one group per core, all three validators in the single group when
///   n_cores=1 & n_validators=3).
/// - `n_cores`: matches the final `ParaScheduler.AvailabilityCores.len()`.
/// - `zeroth_delay_tranche_width`, `relay_vrf_modulo_samples`,
///   `n_delay_tranches`, `no_show_slots`, `needed_approvals`, `dispute_period`:
///   pulled from `crate::chain_spec::default_parachains_host_configuration()` —
///   the same config we write into the fresh `ConfigurationConfig`.
/// - `discovery_keys` + `assignment_keys`: derived from
///   [`dev_authority_set`] (same source as SessionConfig.keys).
///
/// Fields NOT patched (preserved from session_info cascade): `active_validator_indices`
/// (set by shared pallet), `random_seed`, `validators`.
#[cfg(feature = "polkadot-native")]
fn fix_para_session_info_session_zero(
	storage: &mut sp_core::storage::Storage,
	n_cores: u32,
	validator_groups: Vec<Vec<u32>>,
) -> Result<(), String> {
	use codec::{Decode, Encode};
	use polkadot_primitives::{GroupIndex, IndexedVec, SessionInfo, ValidatorIndex};

	// `Sessions<T>` uses **Identity** hashing (not Twox64Concat): see
	// `runtime/parachains/src/session_info.rs:102`. The storage key is therefore
	// `twox_128("ParaSessionInfo") || twox_128("Sessions") || SCALE(0u32)` — no
	// hash of the key, just the raw SCALE-encoded key appended.
	let prefix: Vec<u8> =
		[twox_128(b"ParaSessionInfo"), twox_128(b"Sessions")].concat();
	let full_key: Vec<u8> = [prefix, 0u32.encode()].concat();

	let Some(existing_bytes) = storage.top.get(&full_key).cloned() else {
		// No Sessions(0) entry — session cascade didn't populate it. Nothing to
		// patch; let the runtime handle first session change later.
		log::warn!(
			"fix_para_session_info_session_zero: ParaSessionInfo.Sessions(0) not found; \
			 skipping (unexpected but non-fatal)"
		);
		return Ok(());
	};

	let mut session_info: SessionInfo = SessionInfo::decode(&mut &existing_bytes[..])
		.map_err(|e| format!("decode ParaSessionInfo.Sessions(0): {e}"))?;

	// Patch validator_groups + n_cores from our corrected scheduler state.
	let groups_as_indexed: IndexedVec<GroupIndex, Vec<ValidatorIndex>> = validator_groups
		.into_iter()
		.map(|g| g.into_iter().map(ValidatorIndex).collect::<Vec<_>>())
		.collect::<Vec<_>>()
		.into();
	session_info.validator_groups = groups_as_indexed;
	session_info.n_cores = n_cores;

	// Patch config-derived approval-voting fields from the HostConfiguration
	// we intend to operate under.
	let cfg = crate::chain_spec::default_parachains_host_configuration();
	session_info.zeroth_delay_tranche_width = cfg.zeroth_delay_tranche_width;
	session_info.relay_vrf_modulo_samples = cfg.relay_vrf_modulo_samples;
	session_info.n_delay_tranches = cfg.n_delay_tranches;
	session_info.no_show_slots = cfg.no_show_slots;
	session_info.needed_approvals = cfg.needed_approvals;
	session_info.dispute_period = cfg.dispute_period;

	// Patch discovery_keys + assignment_keys from the same authority set that
	// pallet_session.build() consumed. Keys here are in canonical ordering;
	// session_info.validators is already in the same ordering.
	let auth_set = dev_authority_set();
	session_info.discovery_keys = auth_set.iter().map(|a| a.7.clone()).collect();
	session_info.assignment_keys = auth_set.iter().map(|a| a.6.clone()).collect();

	let new_bytes = session_info.encode();
	let _ = storage.top.insert(full_key, new_bytes);

	Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `key` matches any drop rule.
#[inline]
fn should_drop(key: &[u8], pallet_prefixes: &[[u8; 16]], item_prefixes: &[[u8; 32]]) -> bool {
	// (c) exact well-known keys — checked first: they are short and the check
	//     is O(1) per entry with an early exit on length mismatch.
	if DROP_EXACT.iter().any(|exact| key == *exact) {
		return true
	}

	// (a) pallet-level prefix (16 bytes)
	if key.len() >= 16 && pallet_prefixes.iter().any(|p| key.starts_with(p)) {
		return true
	}

	// (b) item-level prefix (32 bytes)
	if key.len() >= 32 && item_prefixes.iter().any(|p| key.starts_with(p)) {
		return true
	}

	false
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::storage::{Storage, StorageChild};

	/// Build a `Storage` with only a `top` map; `children_default` is empty.
	fn storage_with_keys(keys: &[Vec<u8>]) -> Storage {
		let top = keys.iter().map(|k| (k.clone(), vec![0xdeu8, 0xad])).collect();
		Storage { top, children_default: Default::default() }
	}

	/// 16-byte pallet prefix helper (mirrors production code).
	fn pallet_prefix(name: &[u8]) -> Vec<u8> {
		twox_128(name).to_vec()
	}

	/// 32-byte item prefix helper.
	fn item_prefix(pallet: &[u8], item: &[u8]) -> Vec<u8> {
		let mut v = Vec::with_capacity(32);
		v.extend_from_slice(&twox_128(pallet));
		v.extend_from_slice(&twox_128(item));
		v
	}

	/// A synthetic key with the given 16/32-byte prefix followed by `suffix`.
	fn keyed(prefix: &[u8], suffix: &[u8]) -> Vec<u8> {
		let mut k = prefix.to_vec();
		k.extend_from_slice(suffix);
		k
	}

	// -----------------------------------------------------------------------
	// Test (i): Babe pallet key dropped
	// -----------------------------------------------------------------------
	#[test]
	fn babe_pallet_key_dropped() {
		let babe_key = keyed(&pallet_prefix(b"Babe"), b"CurrentSlot_suffix_bytes");
		let input = storage_with_keys(&[babe_key.clone()]);
		let output = filter_forked_storage(input);
		assert!(!output.top.contains_key(&babe_key), "Babe pallet key must be dropped");
	}

	// -----------------------------------------------------------------------
	// Test (ii): Staking pallet key dropped
	// -----------------------------------------------------------------------
	#[test]
	fn staking_pallet_key_dropped() {
		let staking_key = keyed(&pallet_prefix(b"Staking"), b"Nominators_map_suffix");
		let input = storage_with_keys(&[staking_key.clone()]);
		let output = filter_forked_storage(input);
		assert!(!output.top.contains_key(&staking_key), "Staking pallet key must be dropped");
	}

	// -----------------------------------------------------------------------
	// Test (iii): ImOnline pallet key dropped
	// -----------------------------------------------------------------------
	#[test]
	fn im_online_pallet_key_dropped() {
		let im_key = keyed(&pallet_prefix(b"ImOnline"), b"HeartbeatAfter_suffix");
		let input = storage_with_keys(&[im_key.clone()]);
		let output = filter_forked_storage(input);
		assert!(!output.top.contains_key(&im_key), "ImOnline pallet key must be dropped");
	}

	// -----------------------------------------------------------------------
	// Test (iv): System.Number dropped AND System.Account.<addr> preserved
	// -----------------------------------------------------------------------
	#[test]
	fn system_number_dropped_account_preserved() {
		// System.Number — item-level drop
		let number_key = item_prefix(b"System", b"Number");

		// System.Account — NOT in drop list; only a 48-byte prefixed map key
		// with a fake 32-byte account id appended.
		let account_key = keyed(&item_prefix(b"System", b"Account"), &[0u8; 32]);

		let input = storage_with_keys(&[number_key.clone(), account_key.clone()]);
		let output = filter_forked_storage(input);

		assert!(!output.top.contains_key(&number_key), "System.Number must be dropped");
		assert!(output.top.contains_key(&account_key), "System.Account must be preserved");
	}

	// -----------------------------------------------------------------------
	// Test (v): :code + :heappages preserved AND :extrinsic_index dropped
	// -----------------------------------------------------------------------
	#[test]
	fn code_and_heappages_preserved_extrinsic_index_dropped() {
		let code_key = b":code".to_vec();
		let heappages_key = b":heappages".to_vec();
		let extrinsic_index_key = b":extrinsic_index".to_vec();

		let input = storage_with_keys(&[
			code_key.clone(),
			heappages_key.clone(),
			extrinsic_index_key.clone(),
		]);
		let output = filter_forked_storage(input);

		assert!(output.top.contains_key(&code_key), ":code must be preserved");
		assert!(output.top.contains_key(&heappages_key), ":heappages must be preserved");
		assert!(!output.top.contains_key(&extrinsic_index_key), ":extrinsic_index must be dropped");
	}

	// -----------------------------------------------------------------------
	// Test (vi): children_default passes through byte-identical
	// -----------------------------------------------------------------------
	#[test]
	fn children_default_passes_through_unchanged() {
		let child_key = b"child-root-key".to_vec();
		let child_value = StorageChild {
			data: vec![(b"inner".to_vec(), b"value".to_vec())].into_iter().collect(),
			child_info: sp_core::storage::ChildInfo::new_default(b"child-root-key"),
		};

		let mut input = Storage::default();
		let _ = input.children_default.insert(child_key.clone(), child_value.clone());
		// Also add a key to top that will be dropped, to ensure children are
		// unaffected by the filtering pass.
		let _ = input.top.insert(b":extrinsic_index".to_vec(), b"transient".to_vec());

		let output = filter_forked_storage(input);

		assert!(
			output.children_default.contains_key(&child_key),
			"children_default entry must survive"
		);
		assert_eq!(
			output.children_default[&child_key].data, child_value.data,
			"children_default data must be byte-identical"
		);
		// The top-level transient key must be gone.
		assert!(
			!output.top.contains_key(b":extrinsic_index".as_ref()),
			":extrinsic_index must be dropped even when children exist"
		);
	}

	// -----------------------------------------------------------------------
	// Test (vii): intrablock_entropy exact-match dropped
	// -----------------------------------------------------------------------
	#[test]
	fn intrablock_entropy_dropped() {
		let entropy_key = b":intrablock_entropy".to_vec();
		let input = storage_with_keys(&[entropy_key.clone()]);
		let output = filter_forked_storage(input);
		assert!(!output.top.contains_key(&entropy_key), ":intrablock_entropy must be dropped");
	}

	// -----------------------------------------------------------------------
	// Test (viii): All other drop-pallets are removed (spot-check)
	// -----------------------------------------------------------------------
	#[test]
	fn all_drop_pallets_removed() {
		let keys: Vec<Vec<u8>> = DROP_PALLETS
			.iter()
			.map(|name| keyed(&pallet_prefix(name), b"_any_suffix"))
			.collect();

		let input = storage_with_keys(&keys);
		let output = filter_forked_storage(input);

		for (name, key) in DROP_PALLETS.iter().zip(keys.iter()) {
			assert!(
				!output.top.contains_key(key),
				"pallet {:?} key must be dropped",
				core::str::from_utf8(name).unwrap_or("<non-utf8>")
			);
		}
	}

	// -----------------------------------------------------------------------
	// Test (ix): All drop-items are removed (spot-check)
	// -----------------------------------------------------------------------
	#[test]
	fn all_drop_items_removed() {
		let keys: Vec<Vec<u8>> = DROP_ITEMS.iter().map(|(p, i)| item_prefix(p, i)).collect();

		let input = storage_with_keys(&keys);
		let output = filter_forked_storage(input);

		for ((pallet, item), key) in DROP_ITEMS.iter().zip(keys.iter()) {
			assert!(
				!output.top.contains_key(key),
				"item {}.{} must be dropped",
				core::str::from_utf8(pallet).unwrap_or("<non-utf8>"),
				core::str::from_utf8(item).unwrap_or("<non-utf8>")
			);
		}
	}

	// -----------------------------------------------------------------------
	// Tests (x–z6): KEEP-preservation — one assertion per KEEP category
	//
	// Each test synthesises the minimal key that exercises the relevant prefix
	// path and asserts it survives `filter_forked_storage` unchanged.
	// "Pallet-level" KEEPs use a 16-byte prefix + 32-byte suffix.
	// "Item-level" KEEPs use a 32-byte prefix + 16-byte suffix.
	// -----------------------------------------------------------------------

	#[test]
	fn keep_balances_pallet_preserved() {
		// Balances is a KEEP pallet — no entry in DROP_PALLETS.
		let key = keyed(&pallet_prefix(b"Balances"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Balances pallet key must be preserved");
	}

	#[test]
	fn keep_treasury_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Treasury"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Treasury pallet key must be preserved");
	}

	#[test]
	fn keep_vesting_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Vesting"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Vesting pallet key must be preserved");
	}

	#[test]
	fn keep_identity_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Identity"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Identity pallet key must be preserved");
	}

	#[test]
	fn keep_proxy_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Proxy"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Proxy pallet key must be preserved");
	}

	#[test]
	fn keep_multisig_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Multisig"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Multisig pallet key must be preserved");
	}

	#[test]
	fn keep_preimage_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Preimage"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Preimage pallet key must be preserved");
	}

	#[test]
	fn keep_scheduler_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Scheduler"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Scheduler pallet key must be preserved");
	}

	#[test]
	fn keep_dao_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Dao"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Dao pallet key must be preserved");
	}

	#[test]
	fn keep_finality_rescue_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"FinalityRescue"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "FinalityRescue pallet key must be preserved");
	}

	#[test]
	fn keep_configuration_pallet_preserved() {
		let key = keyed(&pallet_prefix(b"Configuration"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "Configuration pallet key must be preserved");
	}

	// W8: Hrmp added to DROP_PALLETS — every Hrmp.* item (including HrmpChannels
	// and HrmpOpenChannelRequests that were previously KEPT) must be dropped.
	// Reason: livenet HRMP channel MQC heads would mismatch the fresh leafchain's
	// empty MQC, causing cumulus parachain-system assertion panics at block #1.

	#[test]
	fn drop_hrmp_hrmp_channels_item() {
		let key = keyed(&item_prefix(b"Hrmp", b"HrmpChannels"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Hrmp.HrmpChannels must be dropped (Hrmp in DROP_PALLETS)"
		);
	}

	#[test]
	fn drop_hrmp_open_channel_requests_item() {
		let key = keyed(&item_prefix(b"Hrmp", b"HrmpOpenChannelRequests"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Hrmp.HrmpOpenChannelRequests must be dropped (Hrmp in DROP_PALLETS)"
		);
	}

	/// Critical W8 invariant: `Dmp.DownwardMessageQueueHeads` must be dropped.
	/// Without this, livenet's MQC head survives into the forked spec and causes
	/// cumulus parachain-system:861 `assert_eq!(dmq_head.head(), expected_dmq_mqc_head)`
	/// to panic at the fresh collator's first block proposal.
	#[test]
	fn drop_dmp_downward_message_queue_heads_item() {
		let key = keyed(&item_prefix(b"Dmp", b"DownwardMessageQueueHeads"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Dmp.DownwardMessageQueueHeads must be dropped (Dmp in DROP_PALLETS)"
		);
	}

	// W8: Paras pallet added to DROP_PALLETS — every Paras.* item must now be
	// dropped. Livenet paraIds are replaced by entries registered via
	// `--register-leafchain`, flowing through `pallet_paras::build()` in the
	// fresh GenesisConfig (see `assemble_thxnet_*_fork_genesis`).

	#[test]
	fn drop_paras_heads_item() {
		let key = keyed(&item_prefix(b"Paras", b"Heads"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Paras.Heads must be dropped (Paras in DROP_PALLETS)"
		);
	}

	#[test]
	fn drop_paras_current_code_hash_item() {
		let key = keyed(&item_prefix(b"Paras", b"CurrentCodeHash"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Paras.CurrentCodeHash must be dropped (Paras in DROP_PALLETS)"
		);
	}

	#[test]
	fn drop_paras_parachains_item() {
		let key = keyed(&item_prefix(b"Paras", b"Parachains"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Paras.Parachains must be dropped; fresh GenesisConfig.paras is sole source"
		);
	}

	#[test]
	fn drop_paras_past_code_hash_item() {
		let key = keyed(&item_prefix(b"Paras", b"PastCodeHash"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Paras.PastCodeHash must be dropped (whole-pallet wipe)"
		);
	}

	#[test]
	fn drop_paras_upgrade_go_ahead_signal_item() {
		let key = keyed(&item_prefix(b"Paras", b"UpgradeGoAheadSignal"), &[0u8; 16]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			!output.top.contains_key(&key),
			"W8: Paras.UpgradeGoAheadSignal must be dropped (whole-pallet wipe)"
		);
	}

	#[test]
	fn keep_system_account_item_preserved() {
		// Explicit KEEP assertion for System.Account (distinct from the combined
		// test in system_number_dropped_account_preserved).
		let key = keyed(&item_prefix(b"System", b"Account"), &[0u8; 32]);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(output.top.contains_key(&key), "System.Account item key must be preserved");
	}

	// -----------------------------------------------------------------------
	// Edge-case tests
	// -----------------------------------------------------------------------

	/// Empty storage → empty output (top and children both empty).
	#[test]
	fn empty_storage_yields_empty_output() {
		let input = Storage::default();
		let output = filter_forked_storage(input);
		assert!(output.top.is_empty(), "empty top must remain empty");
		assert!(output.children_default.is_empty(), "empty children_default must remain empty");
	}

	/// Storage with only `children_default` populated and empty `top` — the
	/// children pass through untouched; output `top` remains empty.
	#[test]
	fn only_children_default_populated_passes_through() {
		use sp_core::storage::StorageChild;

		let mut input = Storage::default();
		let child_key = b"para-child-root".to_vec();
		let child = StorageChild {
			data: vec![(b"k".to_vec(), b"v".to_vec())].into_iter().collect(),
			child_info: sp_core::storage::ChildInfo::new_default(b"para-child-root"),
		};
		let _ = input.children_default.insert(child_key.clone(), child);

		let output = filter_forked_storage(input);

		assert!(output.top.is_empty(), "top must remain empty");
		assert!(
			output.children_default.contains_key(&child_key),
			"children_default entry must survive"
		);
	}

	/// A key of exactly 16 bytes equal to `twox_128(b"Balances")` must NOT be
	/// dropped — the drop rule requires `key.len() >= 16 && starts_with(prefix)`,
	/// which is satisfied, BUT "Balances" is NOT in DROP_PALLETS, so it must be
	/// preserved.
	#[test]
	fn exact_16_byte_keep_pallet_prefix_preserved() {
		// twox_128("Balances") is exactly 16 bytes; Balances is not a DROP pallet.
		let key = pallet_prefix(b"Balances"); // Vec<u8>, len == 16
		assert_eq!(key.len(), 16);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			output.top.contains_key(&key),
			"16-byte Balances prefix key must be preserved (not in DROP_PALLETS)"
		);
	}

	/// A key of exactly 32 bytes equal to `twox_128(b"Balances") ||
	/// twox_128(b"TotalIssuance")` must NOT be dropped — "Balances" is not a
	/// DROP pallet and ("Balances", "TotalIssuance") is not a DROP item.
	#[test]
	fn exact_32_byte_keep_item_prefix_preserved() {
		let key = item_prefix(b"Balances", b"TotalIssuance"); // Vec<u8>, len == 32
		assert_eq!(key.len(), 32);
		let input = storage_with_keys(&[key.clone()]);
		let output = filter_forked_storage(input);
		assert!(
			output.top.contains_key(&key),
			"32-byte Balances.TotalIssuance prefix key must be preserved"
		);
	}

	// -----------------------------------------------------------------------
	// Invariant: DROP lists must be deduplicated and lexicographically sorted
	//
	// Sorting is a structural invariant that makes the lists auditable by eye,
	// detects accidental duplicate entries at compile-test time, and prevents
	// future edits from introducing confusion about canonical ordering.
	// -----------------------------------------------------------------------

	#[test]
	fn drop_lists_are_deduplicated_and_sorted() {
		// --- DROP_PALLETS ---
		let pallets: Vec<&[u8]> = DROP_PALLETS.to_vec();
		let mut sorted_pallets = pallets.clone();
		sorted_pallets.sort_unstable();
		assert_eq!(
			pallets,
			sorted_pallets,
			"DROP_PALLETS must be sorted lexicographically; got {:?}",
			pallets
				.iter()
				.map(|b| core::str::from_utf8(b).unwrap_or("<non-utf8>"))
				.collect::<Vec<_>>()
		);
		// dedup: after sort, adjacent duplicates would be equal
		let deduped: Vec<&[u8]> = {
			let mut v = sorted_pallets.clone();
			v.dedup();
			v
		};
		assert_eq!(sorted_pallets, deduped, "DROP_PALLETS must not contain duplicate entries");

		// --- DROP_ITEMS ---
		let items: Vec<(&[u8], &[u8])> = DROP_ITEMS.to_vec();
		let mut sorted_items = items.clone();
		sorted_items.sort_unstable();
		assert_eq!(
			items,
			sorted_items,
			"DROP_ITEMS must be sorted lexicographically (by pallet, then item); got {:?}",
			items
				.iter()
				.map(|(p, i)| format!(
					"{}.{}",
					core::str::from_utf8(p).unwrap_or("<non-utf8>"),
					core::str::from_utf8(i).unwrap_or("<non-utf8>")
				))
				.collect::<Vec<_>>()
		);
		let deduped_items: Vec<(&[u8], &[u8])> = {
			let mut v = sorted_items.clone();
			v.dedup();
			v
		};
		assert_eq!(sorted_items, deduped_items, "DROP_ITEMS must not contain duplicate entries");

		// --- DROP_EXACT ---
		let exact: Vec<&[u8]> = DROP_EXACT.to_vec();
		let mut sorted_exact = exact.clone();
		sorted_exact.sort_unstable();
		assert_eq!(
			exact,
			sorted_exact,
			"DROP_EXACT must be sorted lexicographically; got {:?}",
			exact
				.iter()
				.map(|b| core::str::from_utf8(b).unwrap_or("<non-utf8>"))
				.collect::<Vec<_>>()
		);
		let deduped_exact: Vec<&[u8]> = {
			let mut v = sorted_exact.clone();
			v.dedup();
			v
		};
		assert_eq!(sorted_exact, deduped_exact, "DROP_EXACT must not contain duplicate entries");
	}

	// -----------------------------------------------------------------------
	// W2 tests: dev_authority_set + assemble_thxnet_testnet_fork_genesis
	// -----------------------------------------------------------------------

	/// dev_authority_set must return exactly 3 entries (Alice, Bob, Charlie).
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn dev_authority_set_returns_three_authorities() {
		let authorities = dev_authority_set();
		assert_eq!(authorities.len(), 3, "dev_authority_set must return exactly 3 authorities");
	}

	/// Every field in every authority tuple must be non-zero (i.e., not the
	/// default 32-byte zero array).  A zero value would indicate a missing or
	/// mis-wired key derivation.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn dev_authority_set_keys_are_not_default() {
		let authorities = dev_authority_set();
		for (i, a) in authorities.iter().enumerate() {
			let stash_bytes: &[u8] = a.0.as_ref();
			assert_ne!(stash_bytes, &[0u8; 32], "authority[{}] stash must be non-zero", i);

			let controller_bytes: &[u8] = a.1.as_ref();
			assert_ne!(
				controller_bytes, &[0u8; 32],
				"authority[{}] controller must be non-zero",
				i
			);

			// BabeId / ImOnlineId / ValidatorId / AssignmentId / AuthorityDiscoveryId
			// are all 33-byte Sr25519 compressed public keys when serialised by
			// Encode; however their raw inner bytes are [u8; 32].  We test the
			// AccountId32-compatible 32-byte representation by checking the stash
			// bytes for the sr25519-derived ids, and rely on the pairwise-distinct
			// tests below for full coverage.
			//
			// GrandpaId is an Ed25519 public key: also 32 bytes.
			// We assert them all non-zero by round-tripping through `sp_core::crypto::ByteArray`.
			use sp_core::crypto::ByteArray;
			assert_ne!(a.2.as_slice(), &[0u8; 32], "authority[{}] BabeId must be non-zero", i);
			assert_ne!(a.3.as_slice(), &[0u8; 32], "authority[{}] GrandpaId must be non-zero", i);
			assert_ne!(a.4.as_slice(), &[0u8; 32], "authority[{}] ImOnlineId must be non-zero", i);
			assert_ne!(a.5.as_slice(), &[0u8; 32], "authority[{}] ValidatorId must be non-zero", i);
			assert_ne!(
				a.6.as_slice(),
				&[0u8; 32],
				"authority[{}] AssignmentId must be non-zero",
				i
			);
			assert_ne!(
				a.7.as_slice(),
				&[0u8; 32],
				"authority[{}] AuthorityDiscoveryId must be non-zero",
				i
			);
		}
	}

	/// Alice's, Bob's, and Charlie's BabeIds must be pairwise distinct.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn alice_bob_charlie_babe_keys_differ() {
		use sp_core::crypto::ByteArray;
		let auths = dev_authority_set();
		let (alice_babe, bob_babe, charlie_babe) =
			(auths[0].2.as_slice(), auths[1].2.as_slice(), auths[2].2.as_slice());
		assert_ne!(alice_babe, bob_babe, "Alice and Bob BabeIds must differ");
		assert_ne!(alice_babe, charlie_babe, "Alice and Charlie BabeIds must differ");
		assert_ne!(bob_babe, charlie_babe, "Bob and Charlie BabeIds must differ");
	}

	/// Alice's, Bob's, and Charlie's GrandpaIds must be pairwise distinct.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn alice_bob_charlie_grandpa_keys_differ() {
		use sp_core::crypto::ByteArray;
		let auths = dev_authority_set();
		let (alice_gp, bob_gp, charlie_gp) =
			(auths[0].3.as_slice(), auths[1].3.as_slice(), auths[2].3.as_slice());
		assert_ne!(alice_gp, bob_gp, "Alice and Bob GrandpaIds must differ");
		assert_ne!(alice_gp, charlie_gp, "Alice and Charlie GrandpaIds must differ");
		assert_ne!(bob_gp, charlie_gp, "Bob and Charlie GrandpaIds must differ");
	}

	/// Smoke-test the shape of the assembled fork genesis config.
	///
	/// Feeds dev_authority_set() with a dummy wasm blob and Alice as root;
	/// verifies session keys count, staking stakers count, sudo key, and that
	/// each stash balance is at least STASH.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn testnet_fork_genesis_shape() {
		use polkadot_runtime_constants::currency::UNITS as DOT;

		const ENDOWED: u128 = 20 * DOT;
		const STASH: u128 = ENDOWED / 2;

		let authorities = dev_authority_set();
		let root_key: polkadot_primitives::AccountId =
			sp_keyring::Sr25519Keyring::Alice.to_account_id().into();

		let genesis = assemble_thxnet_testnet_fork_genesis(
			&[], // dummy wasm
			authorities,
			root_key.clone(),
			vec![],
			vec![], // paras_to_register — empty for shape smoke test
		);

		// 3 session keys (one per authority)
		assert_eq!(genesis.session.keys.len(), 3, "session.keys must have 3 entries");

		// 3 stakers (one per authority)
		assert_eq!(genesis.staking.stakers.len(), 3, "staking.stakers must have 3 entries");

		// sudo key is Alice
		assert_eq!(genesis.sudo.key, Some(root_key), "sudo.key must be Alice");

		// paras vec matches the input (empty here)
		assert!(
			genesis.paras.paras.is_empty(),
			"paras must be empty when paras_to_register is empty"
		);

		// each stash balance in balances is at least STASH
		for (account, balance) in &genesis.balances.balances {
			// The dev authority stashes appear in balances.balances with ENDOWED amount.
			// We assert every balance entry is at least STASH (which is the lower bound).
			assert!(
				*balance >= STASH,
				"balance for {:?} must be >= STASH ({}), got {}",
				account,
				STASH,
				balance
			);
		}
	}

	// -----------------------------------------------------------------------
	// W2 T2 tests: assemble_thxnet_mainnet_fork_genesis
	// -----------------------------------------------------------------------

	/// Smoke-test the shape of the assembled mainnet fork genesis config.
	///
	/// Feeds dev_authority_set() with a dummy wasm blob and Alice as root;
	/// verifies session keys count, staking stakers count, sudo key, and that
	/// each stash balance is at least STASH.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn mainnet_fork_genesis_shape() {
		use polkadot_runtime_constants::currency::UNITS as DOT;

		const ENDOWED: u128 = 20 * DOT;
		const STASH: u128 = ENDOWED / 2;

		let authorities = dev_authority_set();
		let root_key: polkadot_primitives::AccountId =
			sp_keyring::Sr25519Keyring::Alice.to_account_id().into();

		let genesis = assemble_thxnet_mainnet_fork_genesis(
			&[], // dummy wasm
			authorities,
			root_key.clone(),
			vec![],
			vec![], // paras_to_register
		);

		// 3 session keys (one per authority)
		assert_eq!(genesis.session.keys.len(), 3, "session.keys must have 3 entries");

		// 3 stakers (one per authority)
		assert_eq!(genesis.staking.stakers.len(), 3, "staking.stakers must have 3 entries");

		// sudo key is Alice
		assert_eq!(genesis.sudo.key, Some(root_key), "sudo.key must be Alice");

		// each stash balance in balances is at least STASH
		for (account, balance) in &genesis.balances.balances {
			assert!(
				*balance >= STASH,
				"balance for {:?} must be >= STASH ({}), got {}",
				account,
				STASH,
				balance
			);
		}
	}

	/// Every authority account in the mainnet fork genesis must appear in
	/// `balances.balances` with a balance of at least STASH.
	#[cfg(feature = "polkadot-native")]
	#[test]
	fn authorities_are_endowed_mainnet() {
		use polkadot_runtime_constants::currency::UNITS as DOT;

		const ENDOWED: u128 = 20 * DOT;
		const STASH: u128 = ENDOWED / 2;

		let authorities = dev_authority_set();
		let root_key: polkadot_primitives::AccountId =
			sp_keyring::Sr25519Keyring::Alice.to_account_id().into();

		let genesis = assemble_thxnet_mainnet_fork_genesis(
			&[],
			authorities.clone(),
			root_key,
			vec![],
			vec![],
		);

		// Build a lookup map from the assembled balances.
		let balance_map: std::collections::HashMap<_, _> =
			genesis.balances.balances.iter().cloned().collect();

		for (i, auth) in authorities.iter().enumerate() {
			let stash = &auth.0;
			let balance = balance_map.get(stash).copied().unwrap_or(0);
			assert!(
				balance >= STASH,
				"authority[{}] stash must have balance >= STASH ({}), got {}",
				i,
				STASH,
				balance
			);
		}
	}

	// -----------------------------------------------------------------------
	// W8 tests: b"Paras" drop + fix_para_scheduler_state shuffle formula
	// -----------------------------------------------------------------------

	/// `Paras` must be in DROP_PALLETS (W8). Without this, livenet's 5+ paraIds
	/// leak into `Paras.Parachains`, causing scheduler to allocate more cores
	/// than we have validators and leaving empty backing groups.
	#[test]
	fn paras_pallet_is_in_drop_list() {
		assert!(
			DROP_PALLETS.iter().any(|p| *p == b"Paras"),
			"Paras must be in DROP_PALLETS (W8 — livenet paraIds must be wiped)"
		);
	}

	/// Empty storage → `Session.Validators` missing → function returns Err.
	/// Guarantees we fail loud instead of silently writing garbage.
	#[test]
	fn fix_para_scheduler_errors_when_session_validators_missing() {
		let mut storage = Storage { top: Default::default(), children_default: Default::default() };
		let err = fix_para_scheduler_state(&mut storage).expect_err("must error");
		assert!(
			err.contains("Session.Validators missing"),
			"error should mention missing Session.Validators, got: {err}"
		);
	}

	/// With 3 validators and 1 registered parachain, the scheduler formula
	/// (mirror of runtime scheduler.rs:268-299 for base_size=3, larger=0) must
	/// produce exactly one group `[0,1,2]` and one unoccupied availability core.
	#[test]
	fn fix_para_scheduler_single_parachain_three_validators() {
		use codec::{Compact, Decode, Encode};

		let mut storage = Storage { top: Default::default(), children_default: Default::default() };

		// Session.Validators — we only need the compact length prefix to decode as
		// Vec::<()>::len == 3. Use `Vec<u8>` of 3 zero bytes (any content works because
		// the function only reads the length prefix).
		let session_validators_key: Vec<u8> =
			[twox_128(b"Session"), twox_128(b"Validators")].concat();
		let validators_stub: Vec<u8> = vec![0u8, 0u8, 0u8];
		let _ = storage.top.insert(session_validators_key, validators_stub.encode());

		// Paras.Parachains — length 1, content doesn't matter for the formula.
		let paras_parachains_key: Vec<u8> =
			[twox_128(b"Paras"), twox_128(b"Parachains")].concat();
		let parachains_stub: Vec<u32> = vec![1003u32];
		let _ = storage.top.insert(paras_parachains_key, parachains_stub.encode());

		fix_para_scheduler_state(&mut storage).expect("must succeed");

		let groups_key: Vec<u8> =
			[twox_128(b"ParaScheduler"), twox_128(b"ValidatorGroups")].concat();
		let cores_key: Vec<u8> =
			[twox_128(b"ParaScheduler"), twox_128(b"AvailabilityCores")].concat();
		let ssb_key: Vec<u8> =
			[twox_128(b"ParaScheduler"), twox_128(b"SessionStartBlock")].concat();

		let groups_bytes = storage.top.get(&groups_key).expect("groups key written");
		let groups: Vec<Vec<u32>> =
			Vec::decode(&mut &groups_bytes[..]).expect("decode groups");
		assert_eq!(groups.len(), 1, "n_cores=1 → exactly 1 group");
		assert_eq!(groups[0], vec![0u32, 1u32, 2u32], "group must contain all 3 validators");

		let cores_bytes = storage.top.get(&cores_key).expect("cores key written");
		let mut cores_input: &[u8] = &cores_bytes[..];
		let cores_len = Compact::<u32>::decode(&mut cores_input).expect("decode cores length");
		assert_eq!(cores_len.0, 1, "n_cores=1");
		assert_eq!(cores_input, &[0u8][..], "the 1 core must be None (0x00)");

		let ssb_bytes = storage.top.get(&ssb_key).expect("ssb key written");
		let ssb: u32 = Decode::decode(&mut &ssb_bytes[..]).expect("decode ssb");
		assert_eq!(ssb, 0, "SessionStartBlock must be 0 at genesis");
	}

	/// Absent `Paras.Parachains` → treated as 0 parachains → n_cores=0 → both
	/// ValidatorGroups and AvailabilityCores written as empty Vec (`0x00`).
	#[test]
	fn fix_para_scheduler_no_parachains_emits_empty_groups() {
		use codec::Encode;

		let mut storage = Storage { top: Default::default(), children_default: Default::default() };
		let session_validators_key: Vec<u8> =
			[twox_128(b"Session"), twox_128(b"Validators")].concat();
		let validators_stub: Vec<u8> = vec![0u8, 0u8, 0u8];
		let _ = storage.top.insert(session_validators_key, validators_stub.encode());

		fix_para_scheduler_state(&mut storage).expect("must succeed");

		let groups_key: Vec<u8> =
			[twox_128(b"ParaScheduler"), twox_128(b"ValidatorGroups")].concat();
		let cores_key: Vec<u8> =
			[twox_128(b"ParaScheduler"), twox_128(b"AvailabilityCores")].concat();

		assert_eq!(
			storage.top.get(&groups_key).map(|v| v.as_slice()),
			Some(&[0u8][..]),
			"empty Vec<Vec<_>> encodes to single 0x00 byte"
		);
		assert_eq!(
			storage.top.get(&cores_key).map(|v| v.as_slice()),
			Some(&[0u8][..]),
			"empty Vec<Option<_>> encodes to single 0x00 byte"
		);
	}
}
