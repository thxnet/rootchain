// Copyright 2017-2020 Parity Technologies (UK) Ltd.
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

//! Polkadot chain configurations.

use thxnet_runtime as thxnet;
use thxnet_testnet_runtime as thxnet_testnet;

use beefy_primitives::crypto::AuthorityId as BeefyId;
use frame_support::weights::Weight;
use grandpa::AuthorityId as GrandpaId;
#[cfg(feature = "kusama-native")]
use kusama_runtime as kusama;
#[cfg(feature = "kusama-native")]
use kusama_runtime_constants::currency::UNITS as KSM;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::Forcing;
use polkadot_primitives::{AccountId, AccountPublic, AssignmentId, ValidatorId};
#[cfg(feature = "polkadot-native")]
use polkadot_runtime as polkadot;
#[cfg(feature = "polkadot-native")]
use polkadot_runtime_constants::currency::UNITS as DOT;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;

#[cfg(feature = "rococo-native")]
use rococo_runtime as rococo;
#[cfg(feature = "rococo-native")]
use rococo_runtime_constants::currency::UNITS as ROC;
use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;
#[cfg(feature = "westend-native")]
use westend_runtime as westend;
#[cfg(feature = "westend-native")]
use westend_runtime_constants::currency::UNITS as WND;

#[cfg(feature = "kusama-native")]
const KUSAMA_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(feature = "westend-native")]
const WESTEND_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(feature = "rococo-native")]
const ROCOCO_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(feature = "rococo-native")]
const VERSI_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const THXNET_DEFAULT_PROTOCOL_ID: &str = "thx";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<polkadot_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<polkadot_primitives::Block>,
	/// The light sync state.
	///
	/// This value will be set by the `sync-state rpc` implementation.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// The `ChainSpec` parameterized for the polkadot runtime.
#[cfg(feature = "polkadot-native")]
pub type PolkadotChainSpec = service::GenericChainSpec<polkadot::GenesisConfig, Extensions>;

#[cfg(feature = "polkadot-native")]
pub type ThxnetChainSpec = service::GenericChainSpec<thxnet::GenesisConfig, Extensions>;

#[cfg(feature = "polkadot-native")]
pub type ThxnetTestnetChainSpec =
	service::GenericChainSpec<thxnet_testnet::GenesisConfig, Extensions>;

// Dummy chain spec, in case when we don't have the native runtime.
pub type DummyChainSpec = service::GenericChainSpec<(), Extensions>;

// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "polkadot-native"))]
pub type PolkadotChainSpec = DummyChainSpec;

/// The `ChainSpec` parameterized for the kusama runtime.
#[cfg(feature = "kusama-native")]
pub type KusamaChainSpec = service::GenericChainSpec<kusama::GenesisConfig, Extensions>;

/// The `ChainSpec` parameterized for the kusama runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "kusama-native"))]
pub type KusamaChainSpec = DummyChainSpec;

/// The `ChainSpec` parameterized for the westend runtime.
#[cfg(feature = "westend-native")]
pub type WestendChainSpec = service::GenericChainSpec<westend::GenesisConfig, Extensions>;

/// The `ChainSpec` parameterized for the westend runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "westend-native"))]
pub type WestendChainSpec = DummyChainSpec;

/// The `ChainSpec` parameterized for the rococo runtime.
#[cfg(feature = "rococo-native")]
pub type RococoChainSpec = service::GenericChainSpec<RococoGenesisExt, Extensions>;

/// The `ChainSpec` parameterized for the `versi` runtime.
///
/// As of now `Versi` will just be a clone of `Rococo`, until we need it to differ.
pub type VersiChainSpec = RococoChainSpec;

/// The `ChainSpec` parameterized for the rococo runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "rococo-native"))]
pub type RococoChainSpec = DummyChainSpec;

/// Extension for the Rococo genesis config to support a custom changes to the genesis state.
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg(feature = "rococo-native")]
pub struct RococoGenesisExt {
	/// The runtime genesis config.
	runtime_genesis_config: rococo::GenesisConfig,
	/// The session length in blocks.
	///
	/// If `None` is supplied, the default value is used.
	session_length_in_blocks: Option<u32>,
}

#[cfg(feature = "rococo-native")]
impl sp_runtime::BuildStorage for RococoGenesisExt {
	fn assimilate_storage(&self, storage: &mut sp_core::storage::Storage) -> Result<(), String> {
		sp_state_machine::BasicExternalities::execute_with_storage(storage, || {
			if let Some(length) = self.session_length_in_blocks.as_ref() {
				rococo_runtime_constants::time::EpochDurationInBlocks::set(length);
			}
		});
		self.runtime_genesis_config.assimilate_storage(storage)
	}
}

pub fn polkadot_config() -> Result<PolkadotChainSpec, String> {
	PolkadotChainSpec::from_json_bytes(&include_bytes!("../chain-specs/polkadot.json")[..])
}

pub fn kusama_config() -> Result<KusamaChainSpec, String> {
	KusamaChainSpec::from_json_bytes(&include_bytes!("../chain-specs/kusama.json")[..])
}

pub fn westend_config() -> Result<WestendChainSpec, String> {
	WestendChainSpec::from_json_bytes(&include_bytes!("../chain-specs/westend.json")[..])
}

pub fn rococo_config() -> Result<RococoChainSpec, String> {
	RococoChainSpec::from_json_bytes(&include_bytes!("../chain-specs/rococo.json")[..])
}

/// This is a temporary testnet that uses the same runtime as rococo.
pub fn wococo_config() -> Result<RococoChainSpec, String> {
	RococoChainSpec::from_json_bytes(&include_bytes!("../chain-specs/wococo.json")[..])
}

/// The default parachains host configuration.
#[cfg(any(
	feature = "rococo-native",
	feature = "kusama-native",
	feature = "westend-native",
	feature = "polkadot-native"
))]
fn default_parachains_host_configuration(
) -> polkadot_runtime_parachains::configuration::HostConfiguration<polkadot_primitives::BlockNumber>
{
	use polkadot_primitives::{MAX_CODE_SIZE, MAX_POV_SIZE};

	polkadot_runtime_parachains::configuration::HostConfiguration {
		validation_upgrade_cooldown: 2u32,
		validation_upgrade_delay: 2,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		group_rotation_frequency: 20,
		chain_availability_period: 4,
		thread_availability_period: 4,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024 * 1024,
		ump_service_total_weight: Weight::from_parts(100_000_000_000, MAX_POV_SIZE as u64),
		max_upward_message_size: 50 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_max_parathread_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_parathread_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		minimum_validation_upgrade_delay: 5,
		..Default::default()
	}
}

#[cfg(any(
	feature = "rococo-native",
	feature = "kusama-native",
	feature = "westend-native",
	feature = "polkadot-native"
))]
#[test]
fn default_parachains_host_configuration_is_consistent() {
	default_parachains_host_configuration().panic_if_not_consistent();
}

#[cfg(feature = "polkadot-native")]
fn polkadot_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> polkadot::SessionKeys {
	polkadot::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

#[cfg(feature = "polkadot-native")]
fn thxnet_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> thxnet::SessionKeys {
	thxnet::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

#[cfg(feature = "polkadot-native")]
fn thxnet_testnet_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> thxnet_testnet::SessionKeys {
	thxnet_testnet::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

#[cfg(feature = "kusama-native")]
fn kusama_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> kusama::SessionKeys {
	kusama::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

#[cfg(feature = "westend-native")]
fn westend_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> westend::SessionKeys {
	westend::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

#[cfg(feature = "rococo-native")]
fn rococo_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
	beefy: BeefyId,
) -> rococo_runtime::SessionKeys {
	rococo_runtime::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
		beefy,
	}
}

#[cfg(feature = "polkadot-native")]
fn thxnet_mainnet_config_genesis(wasm_binary: &[u8]) -> thxnet::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	struct InitialAuthorityConfiguration {
		stash_account_id: AccountId,
		controller_account_id: AccountId,
		babe_id: BabeId,
		grandpa_id: GrandpaId,
		im_online_id: ImOnlineId,
		validator_id: ValidatorId,
		assignment_id: AssignmentId,
		authority_discovery_id: AuthorityDiscoveryId,
		endowed_balance: u128,
		stash: u128,
	}

	let initial_authorities: Vec<InitialAuthorityConfiguration> = vec![
		InitialAuthorityConfiguration {
			// validator-thxlab-01
			// 5D4TQmU3gD9cYkvqRoMQqjDdmEsmVtGeFvr8C7kbPSbPiKbE
			stash_account_id: hex![
				"2c0b9a19737f7a05ca1a3073cfee8c8fc7e80b48e5344e748058b435f1070538"
			]
			.into(),
			// 5Ev6mnYb1PoiPmd1LahHkHCtRbbsg5cTArrqSZpZeDFxNbad
			controller_account_id: hex![
				"7e24a21b806d49905159c183828be1f2598d06ddab3dfeb57ac6e21ee6645c7e"
			]
			.into(),
			// 5CXsyHNCP5BhvqEdjJDJNkcf11Lc4EGcP1hz5kTxg7LnhAmJ
			babe_id: hex!["14b9ac3d042c6e457da9fd7f4f28c22e3b86ffdf594248d581bcc792e418a34f"]
				.unchecked_into(),
			// 5CNfCS5SZ6zEu9YtW1HKeyBxWibrwedgd6by4y9W1D2R1NbA
			grandpa_id: hex!["0db1772ae73b8287137e1fc40a918fdbb3a0855cdd86c2117f867f7c1351ae12"]
				.unchecked_into(),
			// 5CXbxZGvTfFmTQZxuTAVuv8SxgjHoh3ikQNBLxd29q3BT9Za
			im_online_id: hex!["1483c51730d676c1f4f277f6e670d2450b90fb351ad084a58606c26044b6361c"]
				.unchecked_into(),
			// 5GWntGfMm2AgX1nidEMYMWXFHd3MGRZLN4JMpjsWZybUz5wW
			validator_id: hex!["c4d63d22acb407d72541a00c7407419f50db0f7f2b273cdf00401b46c66a2b63"]
				.unchecked_into(),
			// 5HY36bKFzDLhkAYqd92MX1GcK6nyr3SrnEWJ1nrEUPaVRic5
			assignment_id: hex!["f2058f9e67c9556c23165b08013b5a0a95d50f694aedf2eda1193393fc2ec714"]
				.unchecked_into(),
			// 5CFwitxmm1XuDww1pYUrgT9DwLzCBD1D3BZiFTjGSBJhiVsV
			authority_discovery_id: hex![
				"0892606660d6b48b30d31e1ffaec74ce6f99c0f646ae30e6944fca59692e030a"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxlab-02
			// 5FsRmkNi5jTqo6N4nRrCYUWjpdrA8Z4fSm8MrCohHpLPJrmy
			stash_account_id: hex![
				"a85716874709c6d35ab73354b349ee65ac01fd96e3b9d54360f46733f99f894c"
			]
			.into(),
			// 5F9myTBz5jgc7Tbv6vUrpwKtk9Grmb8uBifxDkwiPnivY8wh
			controller_account_id: hex![
				"8892c94ae7b688da0e512399b49edd3dcab3255de0e679dfc5c3be3daaa07004"
			]
			.into(),
			// 5He6cKoXeMETiqzq1oQCRbGtcPig6cJdzgmURNSqmKErGeR2
			babe_id: hex!["f6a4dba621c33e52112d1baaa762e2efeae7924d87ee42aac7da7db9cae0381a"]
				.unchecked_into(),
			// 5Fv7rAvMJaKGEWJr1DNxhn5AeaiPot2TuHNvsCzAk9LyPDLR
			grandpa_id: hex!["aa647ccf0d3ba9be191d3daccf69a891bc3b66e2dea3d27ecdb8b3371bbb9ffd"]
				.unchecked_into(),
			// 5FnyuTTxFwm9RHyYQXSoHHNcFU7C8TNgDNbhyHF4jTWcxVe8
			im_online_id: hex!["a4f307aca9c05ca0867c584226f7ef09c7b55786aa0f3866c0c6006bca43b627"]
				.unchecked_into(),
			// 5HmXGskzdWQkXoiUx5XaT6BD7wH4gFLMU7CbwisEd64wbpWg
			validator_id: hex!["fc4e9ac1e5eeb703930b42a3e73df015592f53c17067a8024a14a1065ce9531f"]
				.unchecked_into(),
			// 5HQJhiHVkf82zUgKZ9sYK3TGLEf7RFHCTzGzdAYJkU5Vwmzq
			assignment_id: hex!["ec202171959b1dbb5a3390dc19482e6565212ced3c612b27f814a6b441656652"]
				.unchecked_into(),
			// 5FpVRiwko6FKQ1Zxa8iRwDaskaLaQK8R1Y1HPJDJoU3kjWGy
			authority_discovery_id: hex![
				"a619a79ea64db768cf90da598b9d2486079107a27343cb1dcb31669e7c30a721"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxlab-03
			// 5EUeWw1fCimAermEFuKqspcfPyyA8pyphunwuNFUUSZeSrQ4
			stash_account_id: hex![
				"6abb1cb91058c905bd8419467e5a892376cf1715e4a47240fe2aea86513b3429"
			]
			.into(),
			// 5CJwEpu1ALMVZ5WJAh16JfToNr2hV5nteYVXyg6hK3PHaone
			controller_account_id: hex![
				"0ada7bcd0532fa2cd1ea73dc597e7109484ded65b481ccca1bf120e6c8d82f2d"
			]
			.into(),
			// 5GHhqAQfSTrCCAnRx7oUcAtz8w7Sv5SpdbFSq5ce3aQPdtMh
			babe_id: hex!["badb0963a4270b3ba7cc455d8126fccd8a244d01d72839940c62046b82eb417d"]
				.unchecked_into(),
			// 5Dn9oyDjpcm6yp3bNRnsHEDgzxnkRgqvinChpt3WfZScjt48
			grandpa_id: hex!["4bd8a9865773acc466e76de6538299431c419c4b14e0f4ba2c9f4e28184cd5f1"]
				.unchecked_into(),
			// 5HEgZFwa7aSRR8ViiPzupRyyFY1whqAQ2KVfsP2B7KEW3wEC
			im_online_id: hex!["e4c93e98c640835139fe7545297b95f9953a8e12c19514241a0fbf5002377d37"]
				.unchecked_into(),
			// 5CotZK9tmFxTfoV2wkQFvLsSvAUsTDEhWv8RaZoT8SXPLYCh
			validator_id: hex!["20ef930cbed9f2ee502666d501c4f7304485ee7c7026b7c75b0245bde9abe061"]
				.unchecked_into(),
			// 5Ev7vcMkLsgwZmmKqDgMANECS5aVPFRNiGuaHNM56zurxtxB
			assignment_id: hex!["7e2882fe763f621c0ed52bcbf98a1400b1e02c1cecf8739d7f6c34e936bd794e"]
				.unchecked_into(),
			// 5FvDvVNceFRzGP5i3BK7RvzbYa3qpPMRcy4ty5tRC5jruLDu
			authority_discovery_id: hex![
				"aa78ef9b006cdb7f0abc323bc5f9afa5a5b8ecd6724968be374241034fde0104"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxlab-04
			// 5CP5k9GxHiLBbFfVBGDP5uxYw8CKFfVGHCMZFkH31vJLccdN
			stash_account_id: hex![
				"0e0418df3ae3ad45318637dfbdc52024818199097156fa2c6a85b3742eed551b"
			]
			.into(),
			// 5E1pZya82RdeiJZ3yiUteDtEEHmAjHYb2ENydfAJKoRNjkA7
			controller_account_id: hex![
				"564553724320d3d4113d9440946e43518e536358f45318ac231fa24b9ab2f547"
			]
			.into(),
			// 5Fk8sD6tsGFVFZJC6GZHMe2AiXFK5eUC39xHJGcjD6X7rAJu
			babe_id: hex!["a2c775c3763ada17a8902c9c7ab21bd8954b66b1a70ade23ecae5b09a7836a6b"]
				.unchecked_into(),
			// 5CLCUaSjUhmukZEsp9bTgWi6gBDCMEVLXebN79U46q68Qzh1
			grandpa_id: hex!["0bd105873f82c71c02d22569b8ef8713bf90c290826a2ed75f23b45c65d0b380"]
				.unchecked_into(),
			// 5D9eTF1DmhYdVykWxeeAVavSuU1LvCiUS4Wonn2GAvGpWpmY
			im_online_id: hex!["3001007d1da04c41f18f3a5264f8e1900094d1c58708d41917b0d06cda735500"]
				.unchecked_into(),
			// 5D2UqX9qwi8LCMN7syDQtxStz9HsGv3MxH2fMfqj8vwqHiYD
			validator_id: hex!["2a89ea031ce285a28be58c4156f1c75d4af6489092f95ea131b44c2eb7ac5138"]
				.unchecked_into(),
			// 5E7YSk5jGcXBUgskz2wenen9KJDZDnpRqEjsWMtZV7xmDY9S
			assignment_id: hex!["5aa2848f40f55cc43966b67ad7cbcae522badffe4a146f496d65054d80d43b30"]
				.unchecked_into(),
			// 5CPtKqZaD24T9zne64TEiJCwpuZhC2QrTwNhPKebnia5pYB4
			authority_discovery_id: hex![
				"0ea0e70fa4585af657cfb929689af89065c0c2ff963c47f2a6467324afb5ee0c"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxlab-05
			// 5CfaKGdo79tuzCCcL6qZhUSkoPSfcCyCeGsqMozmBbPXdszT
			stash_account_id: hex![
				"1a9833e438f1180db341138476aeae1d2b66f6f69ca31386ff43fdd10f837743"
			]
			.into(),
			// 5H5yBENm5cYGA1m5eJrU7srdB7WrLh49B3NsSCBJksfraABN
			controller_account_id: hex![
				"de23fc0ae8b013fbf0ec6d0bbd51763f29666c487cf3624213b7f1cee4ba0d79"
			]
			.into(),
			// 5GZZV7ijJBUrn33USBqBShrxccAT7VeNuRGzAZWM623kPGcN
			babe_id: hex!["c6f2de87fa71351f5e800099b8b2a606409777ad5ba01c44493f372c8ea09948"]
				.unchecked_into(),
			// 5FMYd9YVje234kxfCwZ5UmWoEQ6Zjz78GjjN3hQLM7SH3wDi
			grandpa_id: hex!["918cca71ce0278acc7fe8e1540a04a0a29ba5b2be5f8fbc8ebdd4bb35d87e3f6"]
				.unchecked_into(),
			// 5GshjczXkSpUYnn18MTCV5KQkyCzyHAgPHDpJpeR6VakKY4i
			im_online_id: hex!["d4c90f0ccd9d91b1b9c7a8453160861eed77cf51e4cdee9cf4698c9b7b03012e"]
				.unchecked_into(),
			// 5Ck7TuqMn25di6WtYZFqm1gSoz5cpxcLrEm2s5UF1Xauuro9
			validator_id: hex!["1e0e0a89a830c25a7fa8293394c32b2b8cffd62a0d52ab65c3ee343d2509a43c"]
				.unchecked_into(),
			// 5DFeNGeQb7pwRw4Q9EsqZBzPsSbPV9Pd7KSXxXoCwtB3e7wy
			assignment_id: hex!["34942f8f3af6588b0cc5782da5e8dc8e0ff421804a49e42f4ffd055d4146702f"]
				.unchecked_into(),
			// 5DFaRGzDZdotjVmxo2tos9zKps3o49Aje1sRhgtS68ge5QkB
			authority_discovery_id: hex![
				"3486e52152c4dfdc2154bf085787b5a4266237f2e61799d3dd5c19cec5191677"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxlab-06
			// 5HNUHr4BN81Yb5bjK6EKp8gT4ryQwjjXHLnWecUKqWAudAsm
			stash_account_id: hex![
				"eab9eaeb76bb357ecbd91f769f1065260a241a86e4ee668a03961b229f24fc65"
			]
			.into(),
			// 5CowpDXBbgnkJX7HDpddfkeB92MGuJCb4vedXEjDC8pvm8VK
			controller_account_id: hex![
				"20fa89cc29a76ac3575260f3821800f1a86d1f1e5dcbb98647d1e7c2ca6a1a0b"
			]
			.into(),
			// 5FZD3BKZTg3wGgEB3Bp6SmK7wxBdWSBuyW5ZqBv8y5774gpz
			babe_id: hex!["9a71cdebd59f9632ef504e56933433f971533eb41f180f8b0be96ac2ffff9841"]
				.unchecked_into(),
			// 5ECrXnTf7R7W5wF8bv4xJiJYYyQUgnZfGy6uce4t36puANrT
			grandpa_id: hex!["5eafa17ce5922539a755717db9069a897f99e764b9570125944baccfd762ae93"]
				.unchecked_into(),
			// 5HfuEWtpg7m1jU2tsrKFahT4LZvjT7sdEGwdqQYRVu9GocDm
			im_online_id: hex!["f8050dc1c681b6c81c00793306fcf76d69ff0d826ed329d2223e625cfb8c493a"]
				.unchecked_into(),
			// 5GKi4LSFjakjSnkTE1MCx1qEW8EewbxUrF6xei4cRjmDsYe5
			validator_id: hex!["bc624ac2d1b2f2997d16a7d0a1c124005cd1f4fcc0911f58437fc191fec2f241"]
				.unchecked_into(),
			// 5HNMTXEtKEMNYnnXtbZCS2vkL5D7vwhQQpPehckWtEaETvwG
			assignment_id: hex!["eaa2ea4582cd08270b23a3250fcf1178aa204d1e1aae1c06dcbf4489f97e720e"]
				.unchecked_into(),
			// 5ELDLCNRqX61SxCVyi2mKjPXyG8gPvXf1BXX1UA3Ez16Q5RD
			authority_discovery_id: hex![
				"644c5e4f194babc5f07bdffc12257ca21dbf4d82293b93409f609570b20df308"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-mw3w
			// 5C7uufNA2UA1Xhuw2CbWA8CDnjDLPGYzsLpo2TSAfLp4CaV1
			stash_account_id: hex![
				"02724e38ff79e163d059f48055e35033ba82970c5aeec44679eebede7ee36d2e"
			]
			.into(),
			// 5HnQDWZL7VEuDxmh834QiTbBm9wFgBJfAgwHGL539XZWcJn3
			controller_account_id: hex![
				"fcfa172a61425916914c2ffc5e4b8eb8d40422c5b94e172a90a4245a3915b973"
			]
			.into(),
			// 5HKgqN3fqkEZ4UBupvKKEenqQpgbhmGZSigVLHapegrV5jue
			babe_id: hex!["e89a68023f4977a5703430297114f51d605d3ba7122c7cfbfd51d51ad43b3a0a"]
				.unchecked_into(),
			// 5DQjEK2cWN2Qnp5sFdJQAoQ5RLaveyCxYpCbc8kWK2mbkrHi
			grandpa_id: hex!["3b81c394986154d62f52b7d24a825a8b0068c8c7103359b35629894610ff8f3b"]
				.unchecked_into(),
			// 5GsNmaQyLZH9ZaoSYjQK6S4NkTSWtqpKPuM6WorY6RTUDduB
			im_online_id: hex!["d489366c13d6b5a31733c88180eee6667d70d4ff146d52aed4fe6901c536d259"]
				.unchecked_into(),
			// 5CvZ26r67wwh9mjpKCsPkDpupTCg5szPfHLcFg2ZyW56cXk8
			validator_id: hex!["260485601946173bbb60297e43743feba7b5965cbd5d343d87e75a07af52e639"]
				.unchecked_into(),
			// 5DkvxywcaZmKNfxYQtargUf7it67gZpFhmAzoqadLoPq3PLU
			assignment_id: hex!["4aea2d901c0e288b12a12dfcd5c1d40c9bca980cc9b59931a408a9fd9aefa274"]
				.unchecked_into(),
			// 5FzzqjbgkbxkLhpb33VwtTA9HX5RtwUVmNE1nveH47ojvmNf
			authority_discovery_id: hex![
				"ae1d2007a2ead4297513068c09ece1f4690381881e17b97f643a78597511ab16"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-bit3x
			// 5F1cw9ritqUhSR2tFgHfm8FGytPKK5CmNwHgsR7k5wNpudFQ
			stash_account_id: hex![
				"825a6497bd926a881d39b1cba8f38418384521725c81fcb3df86eeca286cd139"
			]
			.into(),
			// 5DAfUALKbNGrvM4BiEU8AZ5GQCVat3xyiZhk5ry6ud17XwPY
			controller_account_id: hex![
				"30c7aabf284eb4df638ecc2277860043a6d804fe00af7bfdb346de5ab3d00438"
			]
			.into(),
			// 5Cz212oEAed58Y7dPLZgr4uUVndBbpNXZxHxJ7WsC4Go47Y6
			babe_id: hex!["28a915d10a6da23ca97f54a85a324f8fe42c03f4bd23d128707c98b777e70025"]
				.unchecked_into(),
			// 5FWBTpBSv4vCR4SC5Q5XT4zGvXF3cAT7AHfe7i45yRdUxwAL
			grandpa_id: hex!["9822c3f17e42e248abe129320f72cdefed3d8e17800428bb871abb606c756320"]
				.unchecked_into(),
			// 5GEPMogBRejbdtfwmE5oNgK8bgJPnbEf4yvBUSVFgXkYhnJu
			im_online_id: hex!["b8531e86d49ac0cfb244722ba26cb9c0ac843b7d863ea673d558b137956b080b"]
				.unchecked_into(),
			// 5ChdhgWM151SKTJo5UU9jR4QWKxmGA6kK3G6CXA4Stqd2x93
			validator_id: hex!["1c2a17da041d5d0b456dbcdc9880ee628af58b486ff77ffc85ecf5e7c46a7018"]
				.unchecked_into(),
			// 5HdT6E71boch8V9Uwr1ae3MvCgdjQkDFDscPY6rsTYLL9toN
			assignment_id: hex!["f6268ed9a98947122b001ffcb5035cc63f61843fd4119a61eb8c46f282271553"]
				.unchecked_into(),
			// 5GKYqj16W8BHbZHXrx36RwW2ncZkvADsTRymsp3rqYRHbi5K
			authority_discovery_id: hex![
				"bc43436e397e074f950e5e628ec6aa7876818b86dd01b6fd673c55e4ff19f25f"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-aro
			// 5Dtrv7DtoAygS9tNsc9yjCfnLnKqrWL1VLXbme15xNqUa6WV
			stash_account_id: hex![
				"50f68259b78b5d98ae264afdbe048ec8195dc0c24d9766bc5c82641f0f86770c"
			]
			.into(),
			// 5G6iwhKUmTp9tcBJg9kVf17kcoMGyWZ87QQD1s6EEJKyqcJW
			controller_account_id: hex![
				"b27b152bb9ab4421934d44e244416e8fbba6f41978ef0f6b3902e7b0863a402c"
			]
			.into(),
			// 5D2EP4Fzap3WhFVudHDYRap27icoTQWDELbEH8LBC3NaxcBr
			babe_id: hex!["2a593ffdbd46573e7ddb944dcf8bc93fc79607b96d0b9c20decd3c5805cef15a"]
				.unchecked_into(),
			// 5CKRFQnViKUtpyEmETsG2TxmzbWHDpGt9n9r1NWEVh9CU4RY
			grandpa_id: hex!["0b38c61efa576a0df925baa9ddba97dfcb36f3d2e32084da0d09ab8d737ef5e0"]
				.unchecked_into(),
			// 5C8XGQheYHJYo3S6nzbFJWFfhozNDdpPvo46DqE1pb6RhPQN
			im_online_id: hex!["02e954893604e4b91371d84d8f8d8dbda682fc29a87b17708f9c6605414e6903"]
				.unchecked_into(),
			// 5G1oVXujLY94p3BsJ8H7ZuYMkrWETxX3mERBRbbGe4MGATdt
			validator_id: hex!["aeba2b31e4780def6a35f56eb5a0f5ef30109402736f58538f87fa78a6d3fa37"]
				.unchecked_into(),
			// 5DZpFuureoTg74t3QCWFeUsMsPbURQ2tiukr7McJQQZHAEBX
			assignment_id: hex!["426fe5a68fecb20f382161d02465a677b97c94ce9683b07cdea60bfffb37991b"]
				.unchecked_into(),
			// 5E9di4V9UW77sy78EzTM5ASNvPT4CksWWj29tW2QFzaDAYtf
			authority_discovery_id: hex![
				"5c3abaa856e2ec0eb354ce330e5150a97a7a5f493ea638261d7ac541030d3717"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
		InitialAuthorityConfiguration {
			// validator-thxfdn
			// 5ENfZS3mZRd8aMTVpDZqN2DbqMsJMfH1p2Ymqxq1Bb7fwZBB
			stash_account_id: hex![
				"662b26b596347d8bacd5b661f52c9ec32a2b7079ee03e4e59d9e00fcff6fe52a"
			]
			.into(),
			// 5F4RQCwzuxwK6iWnR6EMta7FYh3bB3uPYm2HiYE7wiQZsThg
			controller_account_id: hex![
				"847d4dc05b142db525beb4a8869a1663789d47427551cac09f2760ed7d80e15f"
			]
			.into(),
			// 5FHEAHSTnkRrNcmf8bf9V9RZtDYbUhUcXhcySvZgETDxnQu8
			babe_id: hex!["8e41a817a660904b75a7d71321e6f3c420be3fcb9f74795209c6cf502466df3f"]
				.unchecked_into(),
			// 5FW1LVeZKtrJB8RE3uWSEVsXSyFEkJA6PF5oEeKAnwi8cUMq
			grandpa_id: hex!["9800ad75e2d3a5924b02bdff0346400916e4ced6afbbaca582d25c093d67dbf8"]
				.unchecked_into(),
			// 5EWUxzw6xDvsUvUTHVXn2Sxm4oRX8hAigNvvQejUjsjAtg54
			im_online_id: hex!["6c2173f551a144379a53d4ce27d8cabc183debfb84edf4961abdc2342d5fab34"]
				.unchecked_into(),
			// 5GsRfLt27HHPESdn6WdSyec47imVrC6Gezh53xSVGBvkHPvm
			validator_id: hex!["d492f31feeccf464d2da61c2734ac8d963f9ddf303f150270b352fe3e5d7d95a"]
				.unchecked_into(),
			// 5CkkEReymKzfrVJ12G1eEARth5CjdPvQrm7XjEVM9A2FD3Ng
			assignment_id: hex!["1e89cfc1df4186b06ea615cc54b196bf7a050fce57f9722286934b937e62153f"]
				.unchecked_into(),
			// 5D4W4TiQXERPSTNJBeqntwgFVSvdVNMod4d684QADZzrcBmx
			authority_discovery_id: hex![
				"2c1485b504fceb98abd94ab0eababf08b0b616a6cb1c8066423b9600677e4a19"
			]
			.unchecked_into(),
			endowed_balance: 100 * DOT,
			stash: 50 * DOT,
		},
	];

	// subkey inspect "$SECRET"
	let endowed_accounts: Vec<(AccountId, u128)> = vec![
		// mainnetpool
		// 5EKwWcAFzP8wAqjaTo7uANPp3GNZDcMFjahbDMh75hU8hReW
		(
			hex!["64171c9eadeebc44f3d667cdbf447fefd3b66cf0337a419177332a4082685220"].into(),
			100 * DOT,
		),
		// mainnetthxfoundation (thxtreasury)
		// 5FxVFABwRiVRZo3YhdPMsDownhjephwa4mRmTsqTQ3gdvHBq
		(
			hex!["ac33013c3677c74c2a2ea265c5b876ba01050cd6454944b7af0ea03739ac9c70"].into(),
			20_000_000 * DOT,
		),
	];

	thxnet::GenesisConfig {
		system: thxnet::SystemConfig { code: wasm_binary.to_vec() },
		balances: thxnet::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|x| (x.0.clone(), x.1))
				.chain(
					initial_authorities
						.iter()
						.map(|x| (x.stash_account_id.clone(), x.endowed_balance)),
				)
				.collect(),
		},
		indices: thxnet::IndicesConfig { indices: vec![] },
		session: thxnet::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.stash_account_id.clone(),
						x.stash_account_id.clone(),
						thxnet_session_keys(
							x.babe_id.clone(),
							x.grandpa_id.clone(),
							x.im_online_id.clone(),
							x.validator_id.clone(),
							x.assignment_id.clone(),
							x.authority_discovery_id.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: thxnet::StakingConfig {
			validator_count: 5,
			minimum_validator_count: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.stash_account_id.clone(),
						x.controller_account_id.clone(),
						x.stash,
						thxnet::StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.stash_account_id.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: thxnet::SudoConfig { key: Some(thxnet_runtime_constants::staking::get_root_id()) },
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: thxnet::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: thxnet::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: thxnet::BabeConfig {
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
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

#[cfg(feature = "polkadot-native")]
fn thxnet_testnet_config_genesis(wasm_binary: &[u8]) -> thxnet_testnet::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			// 5CkwmRM4iQwtdcsJKCfkRiU6tw5Yib1PC6cChdETwCPJsBrB
			hex!["1eb0a3c82b6e651e19bf56cb20e151007457d633d73af54b2444cd315650a842"].into(),
			// 5F1GDwGkKSKpnCvsNzFheuryGoLdbxrXhBifLRQynT3b2PyA
			hex!["8214acba5b2943cf4e7d0b245f4e6798028a8214d83e9f25008961a30b603132"].into(),
			// 5HGxRsCHrs7sygUafTFFnCuAYsyQsRd9cGqLfrRKuJNFYGEm
			hex!["e6852ac65bf2c673dbca3e211a1956a118d5aea433c53dae1078b2a83feef803"]
				.unchecked_into(),
			// 5DRg6LS63WrZUj6EgyzRvbqsMHJDfjpT3vqf4JZGyJ2ggoVE
			hex!["3c3a729484ca64c217c1223475b8d71d8c963ae22878fc0c1d553543d9278e72"]
				.unchecked_into(),
			// 5F7JXKSrULgfWjA8As9eWEPjKdJsXjb944JQ9LQueYUEqVYT
			hex!["86afe37bd7f5f9c7211e87fb4d6fa581c71954b073fad0afd3e4770979e87f45"]
				.unchecked_into(),
			// 5FRL91hAWCweUsWav2by8CgqP5GMqv1pAqLG3sF41ijLN6jE
			hex!["946fbe6ce7eeae65030b5af0e2765d208dc69092de041617da0bbb5aceab8163"]
				.unchecked_into(),
			// 5Hpr4uiqheHV74EvKv7swLVS43uRaxfMEKb6F7Gb5JbSHvbD
			hex!["fed79b1dd5b4ff816280e4dcba4dabd70d26e0fb0c032b3ff868692dbd0b425d"]
				.unchecked_into(),
			// 5HVuHc5RrfPvNoacXbaCuAJxmEofecBsorxSFxB5efoFeyWd
			hex!["f064c76c87d89cdaefed25300948a17819796644217db5616ea54a69594aec39"]
				.unchecked_into(),
		),
		(
			// 5EqkNPVSs7fodEU4ySTzwWaGJdqo29BpQcjUo3boLTZJsCjV
			hex!["7ad2f7b6ad65c37f90940ef8bb1bbac9af5f1d6c12854dc049f6faf39c585c77"].into(),
			// 5HRRErCwzXLx7DMm2pR4eA7dhgw3fW7ytQ4MWXawgSnmReaW
			hex!["ecf96189411463e3ddf1b5fc9022d4d3929e78a9c51fe8c0904cf99c0d4ba847"].into(),
			// 5DM6MwXneqrYQmqxhqCBRHXDasvkwSwa7saKA6tbKLWD4UuF
			hex!["38bbeadfb243f7e2b609bb24a6eff5e6042850781bb1bdebdc65187b2d4a3e1d"]
				.unchecked_into(),
			// 5DmLUaMya6zfV2VMu1HFZNgJDYWDCTGxCqCsA7Xfb41z7jkM
			hex!["4b39520aaf1043f932f986788ea401024e7e27196233f81a7c3e802d3f60df0f"]
				.unchecked_into(),
			// 5HgD7vpb3EHhjMgJ7wd9Z8ST5d2Mdj2dY9cYd1cU7B2R25dA
			hex!["f84143ca44d116ddb40e54c5d49fe82448d76f0d32a3fd1053c807a102dc8b3a"]
				.unchecked_into(),
			// 5E2LvDQYFEKdbEDeUE8FYs2YW9MfxKmgv2C9QyLtzQ2qNKut
			hex!["56ab7d568c8ffec76e2de459b6f598619d07f26b592b8fc478b16193d8275366"]
				.unchecked_into(),
			// 5DfZir5hkFdLUMQdzc5cZFTjTBG95FZv5dKE5BeHBYu6vKMp
			hex!["46d270303707ed6d15ed00af25769920c17caeaed8938dbea3e5be615c30e364"]
				.unchecked_into(),
			// 5DZU8CU8AoEGKMReGnhKprxkLGpiqmRnpA5RZ6ZsU2983fLK
			hex!["422c1f92029bf1202ce9d0612c678462ec4ca82d568f3ce62dc8ca15e0d96277"]
				.unchecked_into(),
		),
		(
			// 5FcHt7sPwQcJBSwCpcVVxYaHJjPpr8dCZgNQPyhaL4TMJpn7
			hex!["9ccbd8a5fec38fd48cc9c2b20713ae096c3da5239b6ce90c16b8141d5cc3961b"].into(),
			// 5Csw8Q1mUtDDmUBe253RYeF22FRuUvCzzMx1GL4NFSp4A2xc
			hex!["240535741850f9f47d712ffa20bcd74a53daf6e89d51df923064776602291a2b"].into(),
			// 5EA8WYJph49LzDguVaCSUgRDc67Saajkru6mhpakAKoMBJCf
			hex!["5c9baee6188926c848b4d402fc3d73b3fe165434f1b07f239ab30c82b841096f"]
				.unchecked_into(),
			// 5GJzPdpUak8APiv7K1E32Bnx7PYFk8YEe7Q7bDH7mwX6mJk3
			hex!["bbd6070f8491206c3f28d96673e14df32ee83d523c363d2eb8fd23682171e1cc"]
				.unchecked_into(),
			// 5EJLdDcd2NKYyqq8T9EcQrmfrU2fCxRi56PWvpBr3d11RKNW
			hex!["62de6dfaa82b302d246868d17eb91f82d92665e3f93dee5ea7100d747e30e279"]
				.unchecked_into(),
			// 5FjwbQTsrwCNLGrThaAjnGvouB1QvcfyEFgPvWjPJsFN58ng
			hex!["a2a18371537968250062aba938fa0162fd2f22cf7b364127e48a164035327857"]
				.unchecked_into(),
			// 5DUbUYph6YxNRzEB74JZt6VjfMa2T8pvxrXd3BRa31QbRVqu
			hex!["3e74a43d6967109acd5fefbca47e2b2e70db6086d498fe431c9002bcfa659d19"]
				.unchecked_into(),
			// 5EqiMmUi7VeLPok9eX4j8VkaC9pmmegBAN7RfWVBFhjpjkGo
			hex!["7acc32f14055684dd6342f847cc337a7a0f4a5b4f1d34a156cb5aac021d73743"]
				.unchecked_into(),
		),
		(
			// 5GpCZcvJ77asez67rCXH3TqNxViMU3jCcV4K2XWLGCubqtrA
			hex!["d21d1e9146357728cf1ebd31f26aa8683930128f66fda9b32bce7932c28af43a"].into(),
			// 5CGDSWgNycvNh1VNvoBEHsvBbde3Ar2vuHVp3w4rEonsazdE
			hex!["08c749538aba7f401649a69d3ddbde89e2c7945ea31be19bf028216b39790426"].into(),
			// 5FjbJBheYdckzrERnjYQxzGRgNG6vcgTXf7NkkktSVMgkH8V
			hex!["a25d3020d560599c06fa7fe4d0c7d6da11b907a14bb5f2005b2a8fe91bc8a21d"]
				.unchecked_into(),
			// 5F2d8ztYGrV92YRvZ5d1FKEyHz9dmiP3exfrDszBT21SarYr
			hex!["831e536aa186cdf509160e5bd71d2b62f4cc45d64b032396c2a3661ecaf65084"]
				.unchecked_into(),
			// 5HW6hcJr7XYA3JPW8rrseb1hdS5EuSDHCFvKT2uKSGvm2Ykf
			hex!["f08b339472aa8a2a48d3f0f54996448ec0bb780ecd18f8158ca89364f8c4b967"]
				.unchecked_into(),
			// 5GHY1TKTkVdqmriYWmYnGRzyULFF7nP3qV1d46KurRoAPX1a
			hex!["bab9f89315e720ab439e858e5ea4c74e13e443beb55ce41b2137283d847ffc1d"]
				.unchecked_into(),
			// 5EvuS2A5ZgM4wjsCuvdzTc9XrncHWLvEKFwfELYKVERMKTN8
			hex!["7ec1b3c10198473f7579b44c8743b2f6a8d209db59299dd4474ea4d0c3170b28"]
				.unchecked_into(),
			// 5Ef5uTeUMpejJsRcVy6CLC9WyWVv5qAa9mXrphTz4RCiLT78
			hex!["72b10801788a4ee606a5cd435f09ed094ba01e88f4c79aebc3570fe1c1267233"]
				.unchecked_into(),
		),
		(
			// 5EkCS6EqvLHxuhiWiWo5sJUGx9EkR8Jon3Uise1d13AcFFGi
			hex!["76973be100ee6d4676763deabf645c10f8fbbd5d4390a7f41b442f82a610c46c"].into(),
			// 5FxZAATm5E9ZNprEyKzpL1Rej4Xz6ihBd4tFNjpKyDxNBi3d
			hex!["ac402e1b08d2fb31251280653748382ff175885885eeb30eef91921d8131934f"].into(),
			// 5CDdFBKHGLj9T7HUzirFaD8P25B7Pe3wriNufWnmsSnGo9Qc
			hex!["06cdaf00d5bfa2d3eb81d29a05599c8ff8d48b40184e6ad8b853a2a892a33957"]
				.unchecked_into(),
			// 5CSx4PUonZrEUXUZMiEmSX97d8Q8KmXeaT4AKJUSiv82rFWQ
			hex!["10f73511ad077e0c4e43759ab937a701a703673897776179adf0d1ec73d8e9f6"]
				.unchecked_into(),
			// 5GjmgVm462wFq479re3n5wSdhX7JTtmm97Y5ANjqVM2Chkmo
			hex!["cebc61203e4517ffbe88c9dcc7e89218a61cf43ce0007fa0568ad474c82dec74"]
				.unchecked_into(),
			// 5CcESnaBXSoWyRWoT4cWGN6DSjFhd1eagJc1ZmcTG5drMenW
			hex!["180b93a412f8a5ef77b4840c21be4bec16623ea5ab0b16a913843f529bde8966"]
				.unchecked_into(),
			// 5DF7FvHP6f9QBKZsx3kXN668o58eJFKbXD8z1TgJFTx2Jphr
			hex!["342b763ca00abaa4fafbd8e7c064dbebc66c1a6b4b965c1f19a32a5958291d7b"]
				.unchecked_into(),
			// 5DNvEGo7KYgyuMWev6VfBifaB4z1Qgi12Mn99NAUkcfa7xgf
			hex!["3a204cdcc8b404e783bcdb8028cf8261ffd4fdfa79c02639e32d8e6c00c84a40"]
				.unchecked_into(),
		),
		(
			// 5GbGrPQkero2nmR8uSpAAmWdEvcXhbEzf8fRvLxio15CawfD
			hex!["c8415e14b39b034eec9b7f6932d81395d0edd74ab3694bced061512c0a36a837"].into(),
			// 5EHRfAp4n1J2kS66NxCf7mSgjoWvC45dtJXqCRHFu2ENTSiA
			hex!["622c21049dd35f8394aa77aa6849b6696cc8df6e3ab84a6841472eb7f486186f"].into(),
			// 5GzZ9N1jPPwevaTsDCYyNC5EriCxRh61uSg4NLZmYpr4Xzph
			hex!["da02db9e19ddae2652726dba6617fa03cf95722ec0b63d1d75ac7a114cc15b70"]
				.unchecked_into(),
			// 5HqQbcxnSBxrnQaSGVDijNYhqitaURZxJyYAEPX6aYj7sLPW
			hex!["ff451c27f50d7f41d9bc1f520ccc717eeff2f0040754b6411ceba8bdc51441bf"]
				.unchecked_into(),
			// 5G8pK2sDTNH96ac9yeEPRXEsU4npUU7d9A7tCHYGqbMw5o7Y
			hex!["b413a4b5eb53b75486648a273f2f35504da48311f7cda6fcd8f1f8961e1d2455"]
				.unchecked_into(),
			// 5Gzkhhd6wrVwrGdpzhiK1FfqMMmKeigMJEKUNKmPCf6QUsZC
			hex!["da29c39a5f3b1f6f90c5274df124e7ba36cce515dba0f27e6b1d21f3f90b4217"]
				.unchecked_into(),
			// 5HmiLfFNXRAgpvyUse3wErWQtD8XhohPxnbRxh5Z8RgmkWQP
			hex!["fc73da7c70dfd520cd764762fc5870435f740e0c245127730be6f4b768318403"]
				.unchecked_into(),
			// 5GGuK83vnSqcm9i1iRncKn3D6JUTjbJ9ZVTyp5HeFBZxwnhr
			hex!["ba3e717db4e832e005b292172c4ba3318881d076c352b487fdd1cb1a25666800"]
				.unchecked_into(),
		),
	];

	const ENDOWED: u128 = 20 * DOT;
	const STASH: u128 = ENDOWED / 2;

	// subkey inspect "$SECRET"
	let endowed_accounts: Vec<(AccountId, u128)> = vec![
		// testnetbit3x
		// 5FqTnw7x2oWaDQYGom7FBFMieHUcP7n9ntUp6cagbyGZ3geT
		(
			hex!["a6d76609b529a4c1388878a292ec0b9fc0899c1dc7cfe0a4589b7bc310a5df5b"].into(),
			75_000_000 * DOT,
		),
		// testnet001
		// 5HGdWCWcY8cp1qnY5FvCPXHWcJJaC5q2JEoyZDbFUFk4voGq
		(
			hex!["e6457578ddbe1bc069cb9f4ea788c23952774613659682df4be1e3f73a817150"].into(),
			75_000_000 * DOT,
		),
		// testnet002
		// 5ELLS1vdojt9PPiK5F2XAHMtEd6koF9pLRiaEUiaPcER8fuz
		(
			hex!["6464453123b14dd0752ee20fb7b4099ac60719ee8cfa24d65c83766ba51df010"].into(),
			75_000_000 * DOT,
		),
		// testnet003
		// 5EUzr2kfs1nXmk7boHi5PVv2k19kcLor2XLUNXfben3qWRnu
		(
			hex!["6aff8bf47cb8dc63fd776af5326b66aa201e2aa97a9724ba82e31a2287d1a77b"].into(),
			50_000_000 * DOT,
		),
		// testnetthxlab
		// 5G3XM6tJ2Q7aQTa4FheT24VbTxABvViRPDuVVFeATGvvyTxY
		(
			hex!["b00a4f336cecf197bb4d57b9332e393bf1e1cb2ff0ecc8da10bbb88f62fb516e"].into(),
			50_000_000 * DOT,
		),
		// testnetpool
		// 5D22dYGvG7ucZZBvFJRQQwKfSKK7LtuiUTshtC3UAukQz7RD
		(
			hex!["2a31b1e8908eb70be0a2688991189bdf4dda1732a43bd73d1ed6482d40343839"].into(),
			75_000_000 * DOT - (initial_authorities.len() as u128) * ENDOWED,
		),
		// testnetthxfoundation
		// 5GL2teb1jKjHgenowY4Y6EQvHkpRHJUpQCYVBDAXk4SADAib
		(
			hex!["bca1b0834cf3b7b0d9258e7a61e5169b16aabfd9233685bfaa8d15c8726b566f"].into(),
			100_000_000 * DOT,
		),
	];

	thxnet_testnet::GenesisConfig {
		system: thxnet_testnet::SystemConfig { code: wasm_binary.to_vec() },
		balances: thxnet_testnet::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|x| (x.0.clone(), x.1))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), ENDOWED)))
				.collect(),
		},
		indices: thxnet_testnet::IndicesConfig { indices: vec![] },
		session: thxnet_testnet::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						thxnet_testnet_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: thxnet_testnet::StakingConfig {
			validator_count: 15,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, thxnet_testnet::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: thxnet_testnet::SudoConfig {
			key: Some(thxnet_testnet_runtime_constants::staking::get_root_id()),
		},
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: thxnet_testnet::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: thxnet_testnet::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: thxnet_testnet::BabeConfig {
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
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

#[cfg(feature = "polkadot-native")]
fn polkadot_staging_testnet_config_genesis(wasm_binary: &[u8]) -> polkadot::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			// 5CkwmRM4iQwtdcsJKCfkRiU6tw5Yib1PC6cChdETwCPJsBrB
			hex!["1eb0a3c82b6e651e19bf56cb20e151007457d633d73af54b2444cd315650a842"].into(),
			// 5F1GDwGkKSKpnCvsNzFheuryGoLdbxrXhBifLRQynT3b2PyA
			hex!["8214acba5b2943cf4e7d0b245f4e6798028a8214d83e9f25008961a30b603132"].into(),
			// 5HGxRsCHrs7sygUafTFFnCuAYsyQsRd9cGqLfrRKuJNFYGEm
			hex!["e6852ac65bf2c673dbca3e211a1956a118d5aea433c53dae1078b2a83feef803"]
				.unchecked_into(),
			// 5DRg6LS63WrZUj6EgyzRvbqsMHJDfjpT3vqf4JZGyJ2ggoVE
			hex!["3c3a729484ca64c217c1223475b8d71d8c963ae22878fc0c1d553543d9278e72"]
				.unchecked_into(),
			// 5F7JXKSrULgfWjA8As9eWEPjKdJsXjb944JQ9LQueYUEqVYT
			hex!["86afe37bd7f5f9c7211e87fb4d6fa581c71954b073fad0afd3e4770979e87f45"]
				.unchecked_into(),
			// 5FRL91hAWCweUsWav2by8CgqP5GMqv1pAqLG3sF41ijLN6jE
			hex!["946fbe6ce7eeae65030b5af0e2765d208dc69092de041617da0bbb5aceab8163"]
				.unchecked_into(),
			// 5Hpr4uiqheHV74EvKv7swLVS43uRaxfMEKb6F7Gb5JbSHvbD
			hex!["fed79b1dd5b4ff816280e4dcba4dabd70d26e0fb0c032b3ff868692dbd0b425d"]
				.unchecked_into(),
			// 5HVuHc5RrfPvNoacXbaCuAJxmEofecBsorxSFxB5efoFeyWd
			hex!["f064c76c87d89cdaefed25300948a17819796644217db5616ea54a69594aec39"]
				.unchecked_into(),
		),
		(
			// 5EqkNPVSs7fodEU4ySTzwWaGJdqo29BpQcjUo3boLTZJsCjV
			hex!["7ad2f7b6ad65c37f90940ef8bb1bbac9af5f1d6c12854dc049f6faf39c585c77"].into(),
			// 5HRRErCwzXLx7DMm2pR4eA7dhgw3fW7ytQ4MWXawgSnmReaW
			hex!["ecf96189411463e3ddf1b5fc9022d4d3929e78a9c51fe8c0904cf99c0d4ba847"].into(),
			// 5DM6MwXneqrYQmqxhqCBRHXDasvkwSwa7saKA6tbKLWD4UuF
			hex!["38bbeadfb243f7e2b609bb24a6eff5e6042850781bb1bdebdc65187b2d4a3e1d"]
				.unchecked_into(),
			// 5DmLUaMya6zfV2VMu1HFZNgJDYWDCTGxCqCsA7Xfb41z7jkM
			hex!["4b39520aaf1043f932f986788ea401024e7e27196233f81a7c3e802d3f60df0f"]
				.unchecked_into(),
			// 5HgD7vpb3EHhjMgJ7wd9Z8ST5d2Mdj2dY9cYd1cU7B2R25dA
			hex!["f84143ca44d116ddb40e54c5d49fe82448d76f0d32a3fd1053c807a102dc8b3a"]
				.unchecked_into(),
			// 5E2LvDQYFEKdbEDeUE8FYs2YW9MfxKmgv2C9QyLtzQ2qNKut
			hex!["56ab7d568c8ffec76e2de459b6f598619d07f26b592b8fc478b16193d8275366"]
				.unchecked_into(),
			// 5DfZir5hkFdLUMQdzc5cZFTjTBG95FZv5dKE5BeHBYu6vKMp
			hex!["46d270303707ed6d15ed00af25769920c17caeaed8938dbea3e5be615c30e364"]
				.unchecked_into(),
			// 5DZU8CU8AoEGKMReGnhKprxkLGpiqmRnpA5RZ6ZsU2983fLK
			hex!["422c1f92029bf1202ce9d0612c678462ec4ca82d568f3ce62dc8ca15e0d96277"]
				.unchecked_into(),
		),
		(
			// 5FcHt7sPwQcJBSwCpcVVxYaHJjPpr8dCZgNQPyhaL4TMJpn7
			hex!["9ccbd8a5fec38fd48cc9c2b20713ae096c3da5239b6ce90c16b8141d5cc3961b"].into(),
			// 5Csw8Q1mUtDDmUBe253RYeF22FRuUvCzzMx1GL4NFSp4A2xc
			hex!["240535741850f9f47d712ffa20bcd74a53daf6e89d51df923064776602291a2b"].into(),
			// 5EA8WYJph49LzDguVaCSUgRDc67Saajkru6mhpakAKoMBJCf
			hex!["5c9baee6188926c848b4d402fc3d73b3fe165434f1b07f239ab30c82b841096f"]
				.unchecked_into(),
			// 5GJzPdpUak8APiv7K1E32Bnx7PYFk8YEe7Q7bDH7mwX6mJk3
			hex!["bbd6070f8491206c3f28d96673e14df32ee83d523c363d2eb8fd23682171e1cc"]
				.unchecked_into(),
			// 5EJLdDcd2NKYyqq8T9EcQrmfrU2fCxRi56PWvpBr3d11RKNW
			hex!["62de6dfaa82b302d246868d17eb91f82d92665e3f93dee5ea7100d747e30e279"]
				.unchecked_into(),
			// 5FjwbQTsrwCNLGrThaAjnGvouB1QvcfyEFgPvWjPJsFN58ng
			hex!["a2a18371537968250062aba938fa0162fd2f22cf7b364127e48a164035327857"]
				.unchecked_into(),
			// 5DUbUYph6YxNRzEB74JZt6VjfMa2T8pvxrXd3BRa31QbRVqu
			hex!["3e74a43d6967109acd5fefbca47e2b2e70db6086d498fe431c9002bcfa659d19"]
				.unchecked_into(),
			// 5EqiMmUi7VeLPok9eX4j8VkaC9pmmegBAN7RfWVBFhjpjkGo
			hex!["7acc32f14055684dd6342f847cc337a7a0f4a5b4f1d34a156cb5aac021d73743"]
				.unchecked_into(),
		),
		(
			// 5GpCZcvJ77asez67rCXH3TqNxViMU3jCcV4K2XWLGCubqtrA
			hex!["d21d1e9146357728cf1ebd31f26aa8683930128f66fda9b32bce7932c28af43a"].into(),
			// 5CGDSWgNycvNh1VNvoBEHsvBbde3Ar2vuHVp3w4rEonsazdE
			hex!["08c749538aba7f401649a69d3ddbde89e2c7945ea31be19bf028216b39790426"].into(),
			// 5FjbJBheYdckzrERnjYQxzGRgNG6vcgTXf7NkkktSVMgkH8V
			hex!["a25d3020d560599c06fa7fe4d0c7d6da11b907a14bb5f2005b2a8fe91bc8a21d"]
				.unchecked_into(),
			// 5F2d8ztYGrV92YRvZ5d1FKEyHz9dmiP3exfrDszBT21SarYr
			hex!["831e536aa186cdf509160e5bd71d2b62f4cc45d64b032396c2a3661ecaf65084"]
				.unchecked_into(),
			// 5HW6hcJr7XYA3JPW8rrseb1hdS5EuSDHCFvKT2uKSGvm2Ykf
			hex!["f08b339472aa8a2a48d3f0f54996448ec0bb780ecd18f8158ca89364f8c4b967"]
				.unchecked_into(),
			// 5GHY1TKTkVdqmriYWmYnGRzyULFF7nP3qV1d46KurRoAPX1a
			hex!["bab9f89315e720ab439e858e5ea4c74e13e443beb55ce41b2137283d847ffc1d"]
				.unchecked_into(),
			// 5EvuS2A5ZgM4wjsCuvdzTc9XrncHWLvEKFwfELYKVERMKTN8
			hex!["7ec1b3c10198473f7579b44c8743b2f6a8d209db59299dd4474ea4d0c3170b28"]
				.unchecked_into(),
			// 5Ef5uTeUMpejJsRcVy6CLC9WyWVv5qAa9mXrphTz4RCiLT78
			hex!["72b10801788a4ee606a5cd435f09ed094ba01e88f4c79aebc3570fe1c1267233"]
				.unchecked_into(),
		),
		(
			// 5EkCS6EqvLHxuhiWiWo5sJUGx9EkR8Jon3Uise1d13AcFFGi
			hex!["76973be100ee6d4676763deabf645c10f8fbbd5d4390a7f41b442f82a610c46c"].into(),
			// 5FxZAATm5E9ZNprEyKzpL1Rej4Xz6ihBd4tFNjpKyDxNBi3d
			hex!["ac402e1b08d2fb31251280653748382ff175885885eeb30eef91921d8131934f"].into(),
			// 5CDdFBKHGLj9T7HUzirFaD8P25B7Pe3wriNufWnmsSnGo9Qc
			hex!["06cdaf00d5bfa2d3eb81d29a05599c8ff8d48b40184e6ad8b853a2a892a33957"]
				.unchecked_into(),
			// 5CSx4PUonZrEUXUZMiEmSX97d8Q8KmXeaT4AKJUSiv82rFWQ
			hex!["10f73511ad077e0c4e43759ab937a701a703673897776179adf0d1ec73d8e9f6"]
				.unchecked_into(),
			// 5GjmgVm462wFq479re3n5wSdhX7JTtmm97Y5ANjqVM2Chkmo
			hex!["cebc61203e4517ffbe88c9dcc7e89218a61cf43ce0007fa0568ad474c82dec74"]
				.unchecked_into(),
			// 5CcESnaBXSoWyRWoT4cWGN6DSjFhd1eagJc1ZmcTG5drMenW
			hex!["180b93a412f8a5ef77b4840c21be4bec16623ea5ab0b16a913843f529bde8966"]
				.unchecked_into(),
			// 5DF7FvHP6f9QBKZsx3kXN668o58eJFKbXD8z1TgJFTx2Jphr
			hex!["342b763ca00abaa4fafbd8e7c064dbebc66c1a6b4b965c1f19a32a5958291d7b"]
				.unchecked_into(),
			// 5DNvEGo7KYgyuMWev6VfBifaB4z1Qgi12Mn99NAUkcfa7xgf
			hex!["3a204cdcc8b404e783bcdb8028cf8261ffd4fdfa79c02639e32d8e6c00c84a40"]
				.unchecked_into(),
		),
		(
			// 5GbGrPQkero2nmR8uSpAAmWdEvcXhbEzf8fRvLxio15CawfD
			hex!["c8415e14b39b034eec9b7f6932d81395d0edd74ab3694bced061512c0a36a837"].into(),
			// 5EHRfAp4n1J2kS66NxCf7mSgjoWvC45dtJXqCRHFu2ENTSiA
			hex!["622c21049dd35f8394aa77aa6849b6696cc8df6e3ab84a6841472eb7f486186f"].into(),
			// 5GzZ9N1jPPwevaTsDCYyNC5EriCxRh61uSg4NLZmYpr4Xzph
			hex!["da02db9e19ddae2652726dba6617fa03cf95722ec0b63d1d75ac7a114cc15b70"]
				.unchecked_into(),
			// 5HqQbcxnSBxrnQaSGVDijNYhqitaURZxJyYAEPX6aYj7sLPW
			hex!["ff451c27f50d7f41d9bc1f520ccc717eeff2f0040754b6411ceba8bdc51441bf"]
				.unchecked_into(),
			// 5G8pK2sDTNH96ac9yeEPRXEsU4npUU7d9A7tCHYGqbMw5o7Y
			hex!["b413a4b5eb53b75486648a273f2f35504da48311f7cda6fcd8f1f8961e1d2455"]
				.unchecked_into(),
			// 5Gzkhhd6wrVwrGdpzhiK1FfqMMmKeigMJEKUNKmPCf6QUsZC
			hex!["da29c39a5f3b1f6f90c5274df124e7ba36cce515dba0f27e6b1d21f3f90b4217"]
				.unchecked_into(),
			// 5HmiLfFNXRAgpvyUse3wErWQtD8XhohPxnbRxh5Z8RgmkWQP
			hex!["fc73da7c70dfd520cd764762fc5870435f740e0c245127730be6f4b768318403"]
				.unchecked_into(),
			// 5GGuK83vnSqcm9i1iRncKn3D6JUTjbJ9ZVTyp5HeFBZxwnhr
			hex!["ba3e717db4e832e005b292172c4ba3318881d076c352b487fdd1cb1a25666800"]
				.unchecked_into(),
		),
	];

	const ENDOWED: u128 = 20 * DOT;
	const STASH: u128 = ENDOWED / 2;

	// subkey inspect "$SECRET"
	let endowed_accounts: Vec<(AccountId, u128)> = vec![
		// testnetbit3x
		// 5FqTnw7x2oWaDQYGom7FBFMieHUcP7n9ntUp6cagbyGZ3geT
		(
			hex!["a6d76609b529a4c1388878a292ec0b9fc0899c1dc7cfe0a4589b7bc310a5df5b"].into(),
			75_000_000 * DOT,
		),
		// testnet001
		// 5HGdWCWcY8cp1qnY5FvCPXHWcJJaC5q2JEoyZDbFUFk4voGq
		(
			hex!["e6457578ddbe1bc069cb9f4ea788c23952774613659682df4be1e3f73a817150"].into(),
			75_000_000 * DOT,
		),
		// testnet002
		// 5ELLS1vdojt9PPiK5F2XAHMtEd6koF9pLRiaEUiaPcER8fuz
		(
			hex!["6464453123b14dd0752ee20fb7b4099ac60719ee8cfa24d65c83766ba51df010"].into(),
			75_000_000 * DOT,
		),
		// testnet003
		// 5EUzr2kfs1nXmk7boHi5PVv2k19kcLor2XLUNXfben3qWRnu
		(
			hex!["6aff8bf47cb8dc63fd776af5326b66aa201e2aa97a9724ba82e31a2287d1a77b"].into(),
			50_000_000 * DOT,
		),
		// testnetthxlab
		// 5G3XM6tJ2Q7aQTa4FheT24VbTxABvViRPDuVVFeATGvvyTxY
		(
			hex!["b00a4f336cecf197bb4d57b9332e393bf1e1cb2ff0ecc8da10bbb88f62fb516e"].into(),
			50_000_000 * DOT,
		),
		// testnetpool
		// 5D22dYGvG7ucZZBvFJRQQwKfSKK7LtuiUTshtC3UAukQz7RD
		(
			hex!["2a31b1e8908eb70be0a2688991189bdf4dda1732a43bd73d1ed6482d40343839"].into(),
			75_000_000 * DOT - (initial_authorities.len() as u128) * ENDOWED,
		),
		// testnetthxfoundation
		// 5GL2teb1jKjHgenowY4Y6EQvHkpRHJUpQCYVBDAXk4SADAib
		(
			hex!["bca1b0834cf3b7b0d9258e7a61e5169b16aabfd9233685bfaa8d15c8726b566f"].into(),
			100_000_000 * DOT,
		),
	];

	polkadot::GenesisConfig {
		system: polkadot::SystemConfig { code: wasm_binary.to_vec() },
		balances: polkadot::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|x| (x.0.clone(), x.1))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), ENDOWED)))
				.collect(),
		},
		indices: polkadot::IndicesConfig { indices: vec![] },
		session: polkadot::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						polkadot_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: polkadot::StakingConfig {
			validator_count: 15,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, polkadot::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: polkadot::SudoConfig {
			key: Some(polkadot_runtime_constants::staking::get_root_id()),
		},
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: polkadot::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: polkadot::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: polkadot::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(polkadot::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: polkadot::AuthorityDiscoveryConfig { keys: vec![] },
		claims: polkadot::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: polkadot::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: polkadot::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

#[cfg(feature = "westend-native")]
fn westend_staging_testnet_config_genesis(wasm_binary: &[u8]) -> westend::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	// subkey inspect "$SECRET"
	let endowed_accounts = vec![
		// 5DaVh5WRfazkGaKhx1jUu6hjz7EmRe4dtW6PKeVLim84KLe8
		hex!["42f4a4b3e0a89c835ee696205caa90dd85c8ea1d7364b646328ee919a6b2fc1e"].into(),
	];
	// SECRET='...' ./scripts/prepare-test-net.sh 4
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			//5ERCqy118nnXDai8g4t3MjdX7ZC5PrQzQpe9vwex5cELWqbt
			hex!["681af4f93073484e1acd6b27395d0d258f1a6b158c808846c8fd05ee2435056e"].into(),
			//5GTS114cfQNBgpQULhMaNCPXGds6NokegCnikxDe1vqANhtn
			hex!["c2463372598ebabd21ee5bc33e1d7e77f391d2df29ce2fbe6bed0d13be629a45"].into(),
			//5FhGbceKeH7fuGogcBwd28ZCkAwDGYBADCTeHiYrvx2ztyRd
			hex!["a097bfc6a33499ed843b711f52f523f8a7174f798a9f98620e52f4170dbe2948"]
				.unchecked_into(),
			//5Es7nDkJt2by5qVCCD7PZJdp76KJw1LdRCiNst5S5f4eecnz
			hex!["7bde49dda82c2c9f082b807ef3ceebff96437d67b3e630c584db7a220ecafacf"]
				.unchecked_into(),
			//5D4e8zRjaYzFamqChGPPtu26PcKbKgUrhb7WqcNbKa2RDFUR
			hex!["2c2fb730a7d9138e6d62fcf516f9ecc2d712af3f2f03ca330c9564b8c0c1bb33"]
				.unchecked_into(),
			//5DD3JY5ENkjcgVFbVSgUbZv7WmrnyJ8bxxu56ee6hZFiRdnh
			hex!["3297a8622988cc23dd9c131e3fb8746d49e007f6e58a81d43420cd539e250e4c"]
				.unchecked_into(),
			//5Gpodowhud8FG9xENXR5YwTFbUAWyoEtw7sYFytFsG4z7SU6
			hex!["d2932edf775088bd088dc5a112ad867c24cc95858f77f8a1ab014de8d4f96a3f"]
				.unchecked_into(),
			//5GUMj8tnjL3PJZgXoiWtgLCaMVNHBNeSeTqDsvcxmaVAjKn9
			hex!["c2fb0f74591a00555a292bc4882d3158bafc4c632124cb60681f164ef81bcf72"]
				.unchecked_into(),
		),
		(
			//5HgDCznTkHKUjzPkQoTZGWbvbyqB7sqHDBPDKdF1FyVYM7Er
			hex!["f8418f189f84814fd40cc1b2e90873e72ea789487f3b98ed42811ba76d10fc37"].into(),
			//5GQTryeFwuvgmZ2tH5ZeAKZHRM9ch5WGVGo6ND9P8f9uMsNY
			hex!["c002bb4af4a1bd2f33d104aef8a41878fe1ac94ba007029c4dfdefa8b698d043"].into(),
			//5C7YkWSVH1zrpsE5KwW1ua1qatyphzYxiZrL24mjkxz7mUbn
			hex!["022b14fbcf65a93b81f453105b9892c3fc4aa74c22c53b4abab019e1d58fbd41"]
				.unchecked_into(),
			//5GwFC6Tmg4fhj4PxSqHycgJxi3PDfnC9RGDsNHoRwAvXvpnZ
			hex!["d77cafd3b32c8b52b0e2780a586a6e527c94f1bdec117c4e4acb0a491461ffa3"]
				.unchecked_into(),
			//5DSVrGURuDuh8Luzo8FYq7o2NWiUSLSN6QAVNrj9BtswWH6R
			hex!["3cdb36a5a14715999faffd06c5b9e5dcdc24d4b46bc3e4df1aaad266112a7b49"]
				.unchecked_into(),
			//5DLEG2AupawCXGwhJtrzBRc3zAhuP8V662dDrUTzAsCiB9Ec
			hex!["38134245c9919ecb20bf2eedbe943b69ba92ceb9eb5477b92b0afd3cb6ce2858"]
				.unchecked_into(),
			//5D83o9fDgnHxaKPkSx59hk8zYzqcgzN2mrf7cp8fiVEi7V4E
			hex!["2ec917690dc1d676002e3504c530b2595490aa5a4603d9cc579b9485b8d0d854"]
				.unchecked_into(),
			//5DwBJquZgncRWXFxj2ydbF8LBUPPUbiq86sXWXgm8Z38m8L2
			hex!["52bae9b8dedb8058dda93ec6f57d7e5a517c4c9f002a4636fada70fed0acf376"]
				.unchecked_into(),
		),
		(
			//5DMHpkRpQV7NWJFfn2zQxCLiAKv7R12PWFRPHKKk5X3JkYfP
			hex!["38e280b35d08db46019a210a944e4b7177665232ab679df12d6a8bbb317a2276"].into(),
			//5FbJpSHmFDe5FN3DVGe1R345ZePL9nhcC9V2Cczxo7q8q6rN
			hex!["9c0bc0e2469924d718ae683737f818a47c46b0612376ecca06a2ac059fe1f870"].into(),
			//5E5Pm3Udzxy26KGkLE5pc8JPfQrvkYHiaXWtuEfmQsBSgep9
			hex!["58fecadc2df8182a27e999e7e1fd7c99f8ec18f2a81f9a0db38b3653613f3f4d"]
				.unchecked_into(),
			//5FxcystSLHtaWoy2HEgRNerj9PrUs452B6AvHVnQZm5ZQmqE
			hex!["ac4d0c5e8f8486de05135c10a707f58aa29126d5eb28fdaaba00f9a505f5249d"]
				.unchecked_into(),
			//5E7KqVXaVGuAqiqMigpuH8oXHLVh4tmijmpJABLYANpjMkem
			hex!["5a781385a0235fe8594dd101ec55ef9ba01883f8563a0cdd37b89e0303f6a578"]
				.unchecked_into(),
			//5H9AybjkpyZ79yN5nHuBqs6RKuZPgM7aAVVvTQsDFovgXb2A
			hex!["e09570f62a062450d4406b4eb43e7f775ff954e37606646cd590d1818189501f"]
				.unchecked_into(),
			//5Ccgs7VwJKBawMbwMENDmj2eFAxhFdGksVHdk8aTAf4w7xox
			hex!["1864832dae34df30846d5cc65973f58a2d01b337d094b1284ec3466ecc90251d"]
				.unchecked_into(),
			//5EsSaZZ7niJs7hmAtp4QeK19AcAuTp7WXB7N7gRipVooerq4
			hex!["7c1d92535e6d94e21cffea6633a855a7e3c9684cd2f209e5ddbdeaf5111e395b"]
				.unchecked_into(),
		),
		(
			//5Ea11qhmGRntQ7pyEkEydbwxvfrYwGMKW6rPERU4UiSBB6rd
			hex!["6ed057d2c833c45629de2f14b9f6ce6df1edbf9421b7a638e1fb4828c2bd2651"].into(),
			//5CZomCZwPB78BZMZsCiy7WSpkpHhdrN8QTSyjcK3FFEZHBor
			hex!["1631ff446b3534d031adfc37b7f7aed26d2a6b3938d10496aab3345c54707429"].into(),
			//5CSM6vppouFHzAVPkVFWN76DPRUG7B9qwJe892ccfSfJ8M5f
			hex!["108188c43a7521e1abe737b343341c2179a3a89626c7b017c09a5b10df6f1c42"]
				.unchecked_into(),
			//5GwkG4std9KcjYi3ThSC7QWfhqokmYVvWEqTU9h7iswjhLnr
			hex!["d7de8a43f7ee49fa3b3aaf32fb12617ec9ff7b246a46ab14e9c9d259261117fa"]
				.unchecked_into(),
			//5CoUk3wrCGJAWbiJEcsVjYhnd2JAHvR59jBRbSw77YrBtRL1
			hex!["209f680bc501f9b59358efe3636c51fd61238a8659bac146db909aea2595284b"]
				.unchecked_into(),
			//5EcSu96wprFM7G2HfJTjYu8kMParnYGznSUNTsoEKXywEsgG
			hex!["70adf80395b3f59e4cab5d9da66d5a286a0b6e138652a06f72542e46912df922"]
				.unchecked_into(),
			//5Ge3sjpD43Cuy7rNoJQmE9WctgCn6Faw89Pe7xPs3i55eHwJ
			hex!["ca5f6b970b373b303f64801a0c2cadc4fc05272c6047a2560a27d0c65589ca1d"]
				.unchecked_into(),
			//5EFcjHLvB2z5vd5g63n4gABmhzP5iPsKvTwd8sjfvTehNNrk
			hex!["60cae7fa5a079d9fc8061d715fbcc35ef57c3b00005694c2badce22dcc5a9f1b"]
				.unchecked_into(),
		),
	];

	const ENDOWMENT: u128 = 1_000_000 * WND;
	const STASH: u128 = 100 * WND;

	westend::GenesisConfig {
		system: westend::SystemConfig { code: wasm_binary.to_vec() },
		balances: westend::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		indices: westend::IndicesConfig { indices: vec![] },
		session: westend::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						westend_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: westend::StakingConfig {
			validator_count: 50,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, westend::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		babe: westend::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(westend::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: westend::AuthorityDiscoveryConfig { keys: vec![] },
		vesting: westend::VestingConfig { vesting: vec![] },
		sudo: westend::SudoConfig { key: Some(endowed_accounts[0].clone()) },
		hrmp: Default::default(),
		configuration: westend::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		registrar: westend_runtime::RegistrarConfig {
			next_free_para_id: polkadot_primitives::LOWEST_PUBLIC_ID,
		},
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

#[cfg(feature = "kusama-native")]
fn kusama_staging_testnet_config_genesis(wasm_binary: &[u8]) -> kusama::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	// subkey inspect "$SECRET"
	let endowed_accounts = vec![
		// 5CVFESwfkk7NmhQ6FwHCM9roBvr9BGa4vJHFYU8DnGQxrXvz
		hex!["12b782529c22032ed4694e0f6e7d486be7daa6d12088f6bc74d593b3900b8438"].into(),
	];

	// for i in 1 2 3 4; do for j in stash controller; do subkey inspect "$SECRET//$i//$j"; done; done
	// for i in 1 2 3 4; do for j in babe; do subkey --sr25519 inspect "$SECRET//$i//$j"; done; done
	// for i in 1 2 3 4; do for j in grandpa; do subkey --ed25519 inspect "$SECRET//$i//$j"; done; done
	// for i in 1 2 3 4; do for j in im_online; do subkey --sr25519 inspect "$SECRET//$i//$j"; done; done
	// for i in 1 2 3 4; do for j in para_validator para_assignment; do subkey --sr25519 inspect "$SECRET//$i//$j"; done; done
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			// 5DD7Q4VEfPTLEdn11CnThoHT5f9xKCrnofWJL5SsvpTghaAT
			hex!["32a5718e87d16071756d4b1370c411bbbb947eb62f0e6e0b937d5cbfc0ea633b"].into(),
			// 5GNzaEqhrZAtUQhbMe2gn9jBuNWfamWFZHULryFwBUXyd1cG
			hex!["bee39fe862c85c91aaf343e130d30b643c6ea0b4406a980206f1df8331f7093b"].into(),
			// 5FpewyS2VY8Cj3tKgSckq8ECkjd1HKHvBRnWhiHqRQsWfFC1
			hex!["a639b507ee1585e0b6498ff141d6153960794523226866d1b44eba3f25f36356"]
				.unchecked_into(),
			// 5EjvdwATjyFFikdZibVvx1q5uBHhphS2Mnsq5c7yfaYK25vm
			hex!["76620f7c98bce8619979c2b58cf2b0aff71824126d2b039358729dad993223db"]
				.unchecked_into(),
			// 5FpewyS2VY8Cj3tKgSckq8ECkjd1HKHvBRnWhiHqRQsWfFC1
			hex!["a639b507ee1585e0b6498ff141d6153960794523226866d1b44eba3f25f36356"]
				.unchecked_into(),
			// 5FpewyS2VY8Cj3tKgSckq8ECkjd1HKHvBRnWhiHqRQsWfFC1
			hex!["a639b507ee1585e0b6498ff141d6153960794523226866d1b44eba3f25f36356"]
				.unchecked_into(),
			// 5FpewyS2VY8Cj3tKgSckq8ECkjd1HKHvBRnWhiHqRQsWfFC1
			hex!["a639b507ee1585e0b6498ff141d6153960794523226866d1b44eba3f25f36356"]
				.unchecked_into(),
			// 5FpewyS2VY8Cj3tKgSckq8ECkjd1HKHvBRnWhiHqRQsWfFC1
			hex!["a639b507ee1585e0b6498ff141d6153960794523226866d1b44eba3f25f36356"]
				.unchecked_into(),
		),
		(
			// 5G9VGb8ESBeS8Ca4or43RfhShzk9y7T5iTmxHk5RJsjZwsRx
			hex!["b496c98a405ceab59b9e970e59ef61acd7765a19b704e02ab06c1cdfe171e40f"].into(),
			// 5F7V9Y5FcxKXe1aroqvPeRiUmmeQwTFcL3u9rrPXcMuMiCNx
			hex!["86d3a7571dd60139d297e55d8238d0c977b2e208c5af088f7f0136b565b0c103"].into(),
			// 5GvuM53k1Z4nAB5zXJFgkRSHv4Bqo4BsvgbQWNWkiWZTMwWY
			hex!["765e46067adac4d1fe6c783aa2070dfa64a19f84376659e12705d1734b3eae01"]
				.unchecked_into(),
			// 5HBDAaybNqjmY7ww8ZcZZY1L5LHxvpnyfqJwoB7HhR6raTmG
			hex!["e2234d661bee4a04c38392c75d1566200aa9e6ae44dd98ee8765e4cc9af63cb7"]
				.unchecked_into(),
			// 5GvuM53k1Z4nAB5zXJFgkRSHv4Bqo4BsvgbQWNWkiWZTMwWY
			hex!["765e46067adac4d1fe6c783aa2070dfa64a19f84376659e12705d1734b3eae01"]
				.unchecked_into(),
			// 5GvuM53k1Z4nAB5zXJFgkRSHv4Bqo4BsvgbQWNWkiWZTMwWY
			hex!["765e46067adac4d1fe6c783aa2070dfa64a19f84376659e12705d1734b3eae01"]
				.unchecked_into(),
			// 5GvuM53k1Z4nAB5zXJFgkRSHv4Bqo4BsvgbQWNWkiWZTMwWY
			hex!["765e46067adac4d1fe6c783aa2070dfa64a19f84376659e12705d1734b3eae01"]
				.unchecked_into(),
			// 5GvuM53k1Z4nAB5zXJFgkRSHv4Bqo4BsvgbQWNWkiWZTMwWY
			hex!["765e46067adac4d1fe6c783aa2070dfa64a19f84376659e12705d1734b3eae01"]
				.unchecked_into(),
		),
		(
			// 5FzwpgGvk2kk9agow6KsywLYcPzjYc8suKej2bne5G5b9YU3
			hex!["ae12f70078a22882bf5135d134468f77301927aa67c376e8c55b7ff127ace115"].into(),
			// 5EqoZhVC2BcsM4WjvZNidu2muKAbu5THQTBKe3EjvxXkdP7A
			hex!["7addb914ec8486bbc60643d2647685dcc06373401fa80e09813b630c5831d54b"].into(),
			// 5CXNq1mSKJT4Sc2CbyBBdANeSkbUvdWvE4czJjKXfBHi9sX5
			hex!["664eae1ca4713dd6abf8c15e6c041820cda3c60df97dc476c2cbf7cb82cb2d2e"]
				.unchecked_into(),
			// 5E8ULLQrDAtWhfnVfZmX41Yux86zNAwVJYguWJZVWrJvdhBe
			hex!["5b57ed1443c8967f461db1f6eb2ada24794d163a668f1cf9d9ce3235dfad8799"]
				.unchecked_into(),
			// 5CXNq1mSKJT4Sc2CbyBBdANeSkbUvdWvE4czJjKXfBHi9sX5
			hex!["664eae1ca4713dd6abf8c15e6c041820cda3c60df97dc476c2cbf7cb82cb2d2e"]
				.unchecked_into(),
			// 5CXNq1mSKJT4Sc2CbyBBdANeSkbUvdWvE4czJjKXfBHi9sX5
			hex!["664eae1ca4713dd6abf8c15e6c041820cda3c60df97dc476c2cbf7cb82cb2d2e"]
				.unchecked_into(),
			// 5CXNq1mSKJT4Sc2CbyBBdANeSkbUvdWvE4czJjKXfBHi9sX5
			hex!["664eae1ca4713dd6abf8c15e6c041820cda3c60df97dc476c2cbf7cb82cb2d2e"]
				.unchecked_into(),
			// 5CXNq1mSKJT4Sc2CbyBBdANeSkbUvdWvE4czJjKXfBHi9sX5
			hex!["664eae1ca4713dd6abf8c15e6c041820cda3c60df97dc476c2cbf7cb82cb2d2e"]
				.unchecked_into(),
		),
		(
			// 5CFj6Kg9rmVn1vrqpyjau2ztyBzKeVdRKwNPiA3tqhB5HPqq
			hex!["0867dbb49721126df589db100dda728dc3b475cbf414dad8f72a1d5e84897252"].into(),
			// 5CwQXP6nvWzigFqNhh2jvCaW9zWVzkdveCJY3tz2MhXMjTon
			hex!["26ab2b4b2eba2263b1e55ceb48f687bb0018130a88df0712fbdaf6a347d50e2a"].into(),
			// 5FCd9Y7RLNyxz5wnCAErfsLbXGG34L2BaZRHzhiJcMUMd5zd
			hex!["2adb17a5cafbddc7c3e00ec45b6951a8b12ce2264235b4def342513a767e5d3d"]
				.unchecked_into(),
			// 5HGLmrZsiTFTPp3QoS1W8w9NxByt8PVq79reqvdxNcQkByqK
			hex!["e60d23f49e93c1c1f2d7c115957df5bbd7faf5ebf138d1e9d02e8b39a1f63df0"]
				.unchecked_into(),
			// 5FCd9Y7RLNyxz5wnCAErfsLbXGG34L2BaZRHzhiJcMUMd5zd
			hex!["2adb17a5cafbddc7c3e00ec45b6951a8b12ce2264235b4def342513a767e5d3d"]
				.unchecked_into(),
			// 5FCd9Y7RLNyxz5wnCAErfsLbXGG34L2BaZRHzhiJcMUMd5zd
			hex!["2adb17a5cafbddc7c3e00ec45b6951a8b12ce2264235b4def342513a767e5d3d"]
				.unchecked_into(),
			// 5FCd9Y7RLNyxz5wnCAErfsLbXGG34L2BaZRHzhiJcMUMd5zd
			hex!["2adb17a5cafbddc7c3e00ec45b6951a8b12ce2264235b4def342513a767e5d3d"]
				.unchecked_into(),
			// 5FCd9Y7RLNyxz5wnCAErfsLbXGG34L2BaZRHzhiJcMUMd5zd
			hex!["2adb17a5cafbddc7c3e00ec45b6951a8b12ce2264235b4def342513a767e5d3d"]
				.unchecked_into(),
		),
	];

	const ENDOWMENT: u128 = 1_000_000 * KSM;
	const STASH: u128 = 100 * KSM;

	kusama::GenesisConfig {
		system: kusama::SystemConfig { code: wasm_binary.to_vec() },
		balances: kusama::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		indices: kusama::IndicesConfig { indices: vec![] },
		session: kusama::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						kusama_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: kusama::StakingConfig {
			validator_count: 50,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, kusama::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: kusama::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: kusama::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: kusama::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(kusama::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: kusama::AuthorityDiscoveryConfig { keys: vec![] },
		claims: kusama::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: kusama::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: kusama::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
		nis_counterpart_balances: Default::default(),
	}
}

#[cfg(feature = "rococo-native")]
fn rococo_staging_testnet_config_genesis(wasm_binary: &[u8]) -> rococo_runtime::GenesisConfig {
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	// subkey inspect "$SECRET"
	let endowed_accounts = vec![
		// 5DwBmEFPXRESyEam5SsQF1zbWSCn2kCjyLW51hJHXe9vW4xs
		hex!["52bc71c1eca5353749542dfdf0af97bf764f9c2f44e860cd485f1cd86400f649"].into(),
	];

	// ./scripts/prepare-test-net.sh 8
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
		BeefyId,
	)> = vec![
		(
			//5EHZkbp22djdbuMFH9qt1DVzSCvqi3zWpj6DAYfANa828oei
			hex!["62475fe5406a7cb6a64c51d0af9d3ab5c2151bcae982fb812f7a76b706914d6a"].into(),
			//5FeSEpi9UYYaWwXXb3tV88qtZkmSdB3mvgj3pXkxKyYLGhcd
			hex!["9e6e781a76810fe93187af44c79272c290c2b9e2b8b92ee11466cd79d8023f50"].into(),
			//5Fh6rDpMDhM363o1Z3Y9twtaCPfizGQWCi55BSykTQjGbP7H
			hex!["a076ef1280d768051f21d060623da3ab5b56944d681d303ed2d4bf658c5bed35"]
				.unchecked_into(),
			//5CPd3zoV9Aaah4xWucuDivMHJ2nEEmpdi864nPTiyRZp4t87
			hex!["0e6d7d1afbcc6547b92995a394ba0daed07a2420be08220a5a1336c6731f0bfa"]
				.unchecked_into(),
			//5F7BEa1LGFksUihyatf3dCDYneB8pWzVyavnByCsm5nBgezi
			hex!["86975a37211f8704e947a365b720f7a3e2757988eaa7d0f197e83dba355ef743"]
				.unchecked_into(),
			//5CP6oGfwqbEfML8efqm1tCZsUgRsJztp9L8ZkEUxA16W8PPz
			hex!["0e07a51d3213842f8e9363ce8e444255990a225f87e80a3d651db7841e1a0205"]
				.unchecked_into(),
			//5HQdwiDh8Qtd5dSNWajNYpwDvoyNWWA16Y43aEkCNactFc2b
			hex!["ec60e71fe4a567ef9fef99d4bbf37ffae70564b41aa6f94ef0317c13e0a5477b"]
				.unchecked_into(),
			//5HbSgM72xVuscsopsdeG3sCSCYdAeM1Tay9p79N6ky6vwDGq
			hex!["f49eae66a0ac9f610316906ec8f1a0928e20d7059d76a5ca53cbcb5a9b50dd3c"]
				.unchecked_into(),
			//5DPSWdgw38Spu315r6LSvYCggeeieBAJtP5A1qzuzKhqmjVu
			hex!["034f68c5661a41930c82f26a662276bf89f33467e1c850f2fb8ef687fe43d62276"]
				.unchecked_into(),
		),
		(
			//5DvH8oEjQPYhzCoQVo7WDU91qmQfLZvxe9wJcrojmJKebCmG
			hex!["520b48452969f6ddf263b664de0adb0c729d0e0ad3b0e5f3cb636c541bc9022a"].into(),
			//5ENZvCRzyXJJYup8bM6yEzb2kQHEb1NDpY2ZEyVGBkCfRdj3
			hex!["6618289af7ae8621981ffab34591e7a6486e12745dfa3fd3b0f7e6a3994c7b5b"].into(),
			//5DLjSUfqZVNAADbwYLgRvHvdzXypiV1DAEaDMjcESKTcqMoM
			hex!["38757d0de00a0c739e7d7984ef4bc01161bd61e198b7c01b618425c16bb5bd5f"]
				.unchecked_into(),
			//5HnDVBN9mD6mXyx8oryhDbJtezwNSj1VRXgLoYCBA6uEkiao
			hex!["fcd5f87a6fd5707a25122a01b4dac0a8482259df7d42a9a096606df1320df08d"]
				.unchecked_into(),
			//5DhyXZiuB1LvqYKFgT5tRpgGsN3is2cM9QxgW7FikvakbAZP
			hex!["48a910c0af90898f11bd57d37ceaea53c78994f8e1833a7ade483c9a84bde055"]
				.unchecked_into(),
			//5EPEWRecy2ApL5n18n3aHyU1956zXTRqaJpzDa9DoqiggNwF
			hex!["669a10892119453e9feb4e3f1ee8e028916cc3240022920ad643846fbdbee816"]
				.unchecked_into(),
			//5ES3fw5X4bndSgLNmtPfSbM2J1kLqApVB2CCLS4CBpM1UxUZ
			hex!["68bf52c482630a8d1511f2edd14f34127a7d7082219cccf7fd4c6ecdb535f80d"]
				.unchecked_into(),
			//5HeXbwb5PxtcRoopPZTp5CQun38atn2UudQ8p2AxR5BzoaXw
			hex!["f6f8fe475130d21165446a02fb1dbce3a7bf36412e5d98f4f0473aed9252f349"]
				.unchecked_into(),
			//5F7nTtN8MyJV4UsXpjg7tHSnfANXZ5KRPJmkASc1ZSH2Xoa5
			hex!["03a90c2bb6d3b7000020f6152fe2e5002fa970fd1f42aafb6c8edda8dacc2ea77e"]
				.unchecked_into(),
		),
		(
			//5FPMzsezo1PRxYbVpJMWK7HNbR2kUxidsAAxH4BosHa4wd6S
			hex!["92ef83665b39d7a565e11bf8d18d41d45a8011601c339e57a8ea88c8ff7bba6f"].into(),
			//5G6NQidFG7YiXsvV7hQTLGArir9tsYqD4JDxByhgxKvSKwRx
			hex!["b235f57244230589523271c27b8a490922ffd7dccc83b044feaf22273c1dc735"].into(),
			//5GpZhzAVg7SAtzLvaAC777pjquPEcNy1FbNUAG2nZvhmd6eY
			hex!["d2644c1ab2c63a3ad8d40ad70d4b260969e3abfe6d7e6665f50dc9f6365c9d2a"]
				.unchecked_into(),
			//5HAes2RQYPbYKbLBfKb88f4zoXv6pPA6Ke8CjN7dob3GpmSP
			hex!["e1b68fbd84333e31486c08e6153d9a1415b2e7e71b413702b7d64e9b631184a1"]
				.unchecked_into(),
			//5HTXBf36LXmkFWJLokNUK6fPxVpkr2ToUnB1pvaagdGu4c1T
			hex!["ee93e26259decb89afcf17ef2aa0fa2db2e1042fb8f56ecfb24d19eae8629878"]
				.unchecked_into(),
			//5FtAGDZYJKXkhVhAxCQrXmaP7EE2mGbBMfmKDHjfYDgq2BiU
			hex!["a8e61ffacafaf546283dc92d14d7cc70ea0151a5dd81fdf73ff5a2951f2b6037"]
				.unchecked_into(),
			//5CtK7JHv3h6UQZ44y54skxdwSVBRtuxwPE1FYm7UZVhg8rJV
			hex!["244f3421b310c68646e99cdbf4963e02067601f57756b072a4b19431448c186e"]
				.unchecked_into(),
			//5D4r6YaB6F7A7nvMRHNFNF6zrR9g39bqDJFenrcaFmTCRwfa
			hex!["2c57f81fd311c1ab53813c6817fe67f8947f8d39258252663b3384ab4195494d"]
				.unchecked_into(),
			//5EPoHj8uV4fFKQHYThc6Z9fDkU7B6ih2ncVzQuDdNFb8UyhF
			hex!["039d065fe4f9234f0a4f13cc3ae585f2691e9c25afa469618abb6645111f607a53"]
				.unchecked_into(),
		),
		(
			//5DMNx7RoX6d7JQ38NEM7DWRcW2THu92LBYZEWvBRhJeqcWgR
			hex!["38f3c2f38f6d47f161e98c697bbe3ca0e47c033460afda0dda314ab4222a0404"].into(),
			//5GGdKNDr9P47dpVnmtq3m8Tvowwf1ot1abw6tPsTYYFoKm2v
			hex!["ba0898c1964196474c0be08d364cdf4e9e1d47088287f5235f70b0590dfe1704"].into(),
			//5EjkyPCzR2SjhDZq8f7ufsw6TfkvgNRepjCRQFc4TcdXdaB1
			hex!["764186bc30fd5a02477f19948dc723d6d57ab174debd4f80ed6038ec960bfe21"]
				.unchecked_into(),
			//5DJV3zCBTJBLGNDCcdWrYxWDacSz84goGTa4pFeKVvehEBte
			hex!["36be9069cdb4a8a07ecd51f257875150f0a8a1be44a10d9d98dabf10a030aef4"]
				.unchecked_into(),
			//5FHf8kpK4fPjEJeYcYon2gAPwEBubRvtwpzkUbhMWSweKPUY
			hex!["8e95b9b5b4dc69790b67b566567ca8bf8cdef3a3a8bb65393c0d1d1c87cd2d2c"]
				.unchecked_into(),
			//5F9FsRjpecP9GonktmtFL3kjqNAMKjHVFjyjRdTPa4hbQRZA
			hex!["882d72965e642677583b333b2d173ac94b5fd6c405c76184bb14293be748a13b"]
				.unchecked_into(),
			//5F1FZWZSj3JyTLs8sRBxU6QWyGLSL9BMRtmSKDmVEoiKFxSP
			hex!["821271c99c958b9220f1771d9f5e29af969edfa865631dba31e1ab7bc0582b75"]
				.unchecked_into(),
			//5CtgRR74VypK4h154s369abs78hDUxZSJqcbWsfXvsjcHJNA
			hex!["2496f28d887d84705c6dae98aee8bf90fc5ad10bb5545eca1de6b68425b70f7c"]
				.unchecked_into(),
			//5CPx6dsr11SCJHKFkcAQ9jpparS7FwXQBrrMznRo4Hqv1PXz
			hex!["0307d29bbf6a5c4061c2157b44fda33b7bb4ec52a5a0305668c74688cedf288d58"]
				.unchecked_into(),
		),
		(
			//5C8AL1Zb4bVazgT3EgDxFgcow1L4SJjVu44XcLC9CrYqFN4N
			hex!["02a2d8cfcf75dda85fafc04ace3bcb73160034ed1964c43098fb1fe831de1b16"].into(),
			//5FLYy3YKsAnooqE4hCudttAsoGKbVG3hYYBtVzwMjJQrevPa
			hex!["90cab33f0bb501727faa8319f0845faef7d31008f178b65054b6629fe531b772"].into(),
			//5Et3tfbVf1ByFThNAuUq5pBssdaPPskip5yob5GNyUFojXC7
			hex!["7c94715e5dd8ab54221b1b6b2bfa5666f593f28a92a18e28052531de1bd80813"]
				.unchecked_into(),
			//5EX1JBghGbQqWohTPU6msR9qZ2nYPhK9r3RTQ2oD1K8TCxaG
			hex!["6c878e33b83c20324238d22240f735457b6fba544b383e70bb62a27b57380c81"]
				.unchecked_into(),
			//5GqL8RbVAuNXpDhjQi1KrS1MyNuKhvus2AbmQwRGjpuGZmFu
			hex!["d2f9d537ffa59919a4028afdb627c14c14c97a1547e13e8e82203d2049b15b1a"]
				.unchecked_into(),
			//5EUNaBpX9mJgcmLQHyG5Pkms6tbDiKuLbeTEJS924Js9cA1N
			hex!["6a8570b9c6408e54bacf123cc2bb1b0f087f9c149147d0005badba63a5a4ac01"]
				.unchecked_into(),
			//5CaZuueRVpMATZG4hkcrgDoF4WGixuz7zu83jeBdY3bgWGaG
			hex!["16c69ea8d595e80b6736f44be1eaeeef2ac9c04a803cc4fd944364cb0d617a33"]
				.unchecked_into(),
			//5DABsdQCDUGuhzVGWe5xXzYQ9rtrVxRygW7RXf9Tsjsw1aGJ
			hex!["306ac5c772fe858942f92b6e28bd82fb7dd8cdd25f9a4626c1b0eee075fcb531"]
				.unchecked_into(),
			//5H91T5mHhoCw9JJG4NjghDdQyhC6L7XcSuBWKD3q3TAhEVvQ
			hex!["02fb0330356e63a35dd930bc74525edf28b3bf5eb44aab9e9e4962c8309aaba6a6"]
				.unchecked_into(),
		),
		(
			//5C8XbDXdMNKJrZSrQURwVCxdNdk8AzG6xgLggbzuA399bBBF
			hex!["02ea6bfa8b23b92fe4b5db1063a1f9475e3acd0ab61e6b4f454ed6ba00b5f864"].into(),
			//5GsyzFP8qtF8tXPSsjhjxAeU1v7D1PZofuQKN9TdCc7Dp1JM
			hex!["d4ffc4c05b47d1115ad200f7f86e307b20b46c50e1b72a912ec4f6f7db46b616"].into(),
			//5GHWB8ZDzegLcMW7Gdd1BS6WHVwDdStfkkE4G7KjPjZNJBtD
			hex!["bab3cccdcc34401e9b3971b96a662686cf755aa869a5c4b762199ce531b12c5b"]
				.unchecked_into(),
			//5GzDPGbUM9uH52ZEwydasTj8edokGUJ7vEpoFWp9FE1YNuFB
			hex!["d9c056c98ca0e6b4eb7f5c58c007c1db7be0fe1f3776108f797dd4990d1ccc33"]
				.unchecked_into(),
			//5GWZbVkJEfWZ7fRca39YAQeqri2Z7pkeHyd7rUctUHyQifLp
			hex!["c4a980da30939d5bb9e4a734d12bf81259ae286aa21fa4b65405347fa40eff35"]
				.unchecked_into(),
			//5CmLCFeSurRXXtwMmLcVo7sdJ9EqDguvJbuCYDcHkr3cpqyE
			hex!["1efc23c0b51ad609ab670ecf45807e31acbd8e7e5cb7c07cf49ee42992d2867c"]
				.unchecked_into(),
			//5DnsSy8a8pfE2aFjKBDtKw7WM1V4nfE5sLzP15MNTka53GqS
			hex!["4c64d3f06d28adeb36a892fdaccecace150bec891f04694448a60b74fa469c22"]
				.unchecked_into(),
			//5CZdFnyzZvKetZTeUwj5APAYskVJe4QFiTezo5dQNsrnehGd
			hex!["160ea09c5717270e958a3da42673fa011613a9539b2e4ebcad8626bc117ca04a"]
				.unchecked_into(),
			//5HgoR9JJkdBusxKrrs3zgd3ToppgNoGj1rDyAJp4e7eZiYyT
			hex!["020019a8bb188f8145d02fa855e9c36e9914457d37c500e03634b5223aa5702474"]
				.unchecked_into(),
		),
		(
			//5HinEonzr8MywkqedcpsmwpxKje2jqr9miEwuzyFXEBCvVXM
			hex!["fa373e25a1c4fe19c7148acde13bc3db1811cf656dc086820f3dda736b9c4a00"].into(),
			//5EHJbj6Td6ks5HDnyfN4ttTSi57osxcQsQexm7XpazdeqtV7
			hex!["62145d721967bd88622d08625f0f5681463c0f1b8bcd97eb3c2c53f7660fd513"].into(),
			//5EeCsC58XgJ1DFaoYA1WktEpP27jvwGpKdxPMFjicpLeYu96
			hex!["720537e2c1c554654d73b3889c3ef4c3c2f95a65dd3f7c185ebe4afebed78372"]
				.unchecked_into(),
			//5DnEySxbnppWEyN8cCLqvGjAorGdLRg2VmkY96dbJ1LHFK8N
			hex!["4bea0b37e0cce9bddd80835fa2bfd5606f5dcfb8388bbb10b10c483f0856cf14"]
				.unchecked_into(),
			//5E1Y1FJ7dVP7qtE3wm241pTm72rTMcDT5Jd8Czv7Pwp7N3AH
			hex!["560d90ca51e9c9481b8a9810060e04d0708d246714960439f804e5c6f40ca651"]
				.unchecked_into(),
			//5CAC278tFCHAeHYqE51FTWYxHmeLcENSS1RG77EFRTvPZMJT
			hex!["042f07fc5268f13c026bbe199d63e6ac77a0c2a780f71cda05cee5a6f1b3f11f"]
				.unchecked_into(),
			//5HjRTLWcQjZzN3JDvaj1UzjNSayg5ZD9ZGWMstaL7Ab2jjAa
			hex!["fab485e87ed1537d089df521edf983a777c57065a702d7ed2b6a2926f31da74f"]
				.unchecked_into(),
			//5ELv74v7QcsS6FdzvG4vL2NnYDGWmRnJUSMKYwdyJD7Xcdi7
			hex!["64d59feddb3d00316a55906953fb3db8985797472bd2e6c7ea1ab730cc339d7f"]
				.unchecked_into(),
			//5FaUcPt4fPz93vBhcrCJqmDkjYZ7jCbzAF56QJoCmvPaKrmx
			hex!["033f1a6d47fe86f88934e4b83b9fae903b92b5dcf4fec97d5e3e8bf4f39df03685"]
				.unchecked_into(),
		),
		(
			//5Ey3NQ3dfabaDc16NUv7wRLsFCMDFJSqZFzKVycAsWuUC6Di
			hex!["8062e9c21f1d92926103119f7e8153cebdb1e5ab3e52d6f395be80bb193eab47"].into(),
			//5HiWsuSBqt8nS9pnggexXuHageUifVPKPHDE2arTKqhTp1dV
			hex!["fa0388fa88f3f0cb43d583e2571fbc0edad57dff3a6fd89775451dd2c2b8ea00"].into(),
			//5H168nKX2Yrfo3bxj7rkcg25326Uv3CCCnKUGK6uHdKMdPt8
			hex!["da6b2df18f0f9001a6dcf1d301b92534fe9b1f3ccfa10c49449fee93adaa8349"]
				.unchecked_into(),
			//5DrA2fZdzmNqT5j6DXNwVxPBjDV9jhkAqvjt6Us3bQHKy3cF
			hex!["4ee66173993dd0db5d628c4c9cb61a27b76611ad3c3925947f0d0011ee2c5dcc"]
				.unchecked_into(),
			//5FNFDUGNLUtqg5LgrwYLNmBiGoP8KRxsvQpBkc7GQP6qaBUG
			hex!["92156f54a114ee191415898f2da013d9db6a5362d6b36330d5fc23e27360ab66"]
				.unchecked_into(),
			//5Gx6YeNhynqn8qkda9QKpc9S7oDr4sBrfAu516d3sPpEt26F
			hex!["d822d4088b20dca29a580a577a97d6f024bb24c9550bebdfd7d2d18e946a1c7d"]
				.unchecked_into(),
			//5DhDcHqwxoes5s89AyudGMjtZXx1nEgrk5P45X88oSTR3iyx
			hex!["481538f8c2c011a76d7d57db11c2789a5e83b0f9680dc6d26211d2f9c021ae4c"]
				.unchecked_into(),
			//5DqAvikdpfRdk5rR35ZobZhqaC5bJXZcEuvzGtexAZP1hU3T
			hex!["4e262811acdfe94528bfc3c65036080426a0e1301b9ada8d687a70ffcae99c26"]
				.unchecked_into(),
			//5E41Znrr2YtZu8bZp3nvRuLVHg3jFksfQ3tXuviLku4wsao7
			hex!["025e84e95ed043e387ddb8668176b42f8e2773ddd84f7f58a6d9bf436a4b527986"]
				.unchecked_into(),
		),
	];

	const ENDOWMENT: u128 = 1_000_000 * ROC;
	const STASH: u128 = 100 * ROC;

	rococo_runtime::GenesisConfig {
		system: rococo_runtime::SystemConfig { code: wasm_binary.to_vec() },
		balances: rococo_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		beefy: Default::default(),
		indices: rococo_runtime::IndicesConfig { indices: vec![] },
		session: rococo_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						rococo_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
							x.8.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		phragmen_election: Default::default(),
		babe: rococo_runtime::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(rococo_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		democracy: rococo_runtime::DemocracyConfig::default(),
		council: rococo::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: rococo::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		treasury: Default::default(),
		authority_discovery: rococo_runtime::AuthorityDiscoveryConfig { keys: vec![] },
		claims: rococo::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: rococo::VestingConfig { vesting: vec![] },
		sudo: rococo_runtime::SudoConfig { key: Some(endowed_accounts[0].clone()) },
		paras: rococo_runtime::ParasConfig { paras: vec![] },
		hrmp: Default::default(),
		configuration: rococo_runtime::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		registrar: rococo_runtime::RegistrarConfig {
			next_free_para_id: polkadot_primitives::LOWEST_PUBLIC_ID,
		},
		xcm_pallet: Default::default(),
		nis_counterpart_balances: Default::default(),
	}
}

/// Returns the properties for the [`PolkadotChainSpec`].
pub fn polkadot_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"tokenSymbol": "THXDEV",
		"tokenDecimals": 10,
		"ss58Format": 42,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Polkadot staging testnet config.
#[cfg(feature = "polkadot-native")]
pub fn polkadot_staging_testnet_config() -> Result<PolkadotChainSpec, String> {
	let wasm_binary = polkadot::WASM_BINARY.ok_or("Polkadot development wasm not available")?;
	let boot_nodes = vec![];

	Ok(PolkadotChainSpec::from_genesis(
		"THXNET. Testnet",
		"thxnet_testnet",
		ChainType::Live,
		move || polkadot_staging_testnet_config_genesis(wasm_binary),
		boot_nodes,
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		Some(polkadot_chain_spec_properties()),
		Default::default(),
	))
}

/// Returns the properties for the [`PolkadotChainSpec`].
pub fn thxnet_testnet_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"tokenSymbol": "THXDEV",
		"tokenDecimals": 10,
		"ss58Format": 42,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Polkadot staging testnet config.
pub fn thxnet_testnet_config() -> Result<ThxnetTestnetChainSpec, String> {
	let wasm_binary =
		thxnet_testnet::WASM_BINARY.ok_or("THXNET. development wasm not available")?;
	let boot_nodes = Vec::new();

	Ok(ThxnetTestnetChainSpec::from_genesis(
		"THXNET. Testnet",
		"thxnet_testnet",
		ChainType::Live,
		move || thxnet_testnet_config_genesis(wasm_binary),
		boot_nodes,
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		Some(thxnet_testnet_chain_spec_properties()),
		Default::default(),
	))
}

/// Returns the properties for the [`PolkadotChainSpec`].
pub fn thxnet_mainnet_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"tokenSymbol": "THX",
		"tokenDecimals": 10,
		"ss58Format": 42,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Polkadot staging testnet config.
pub fn thxnet_mainnet_config() -> Result<ThxnetChainSpec, String> {
	let wasm_binary = thxnet::WASM_BINARY.ok_or("THXNET. development wasm not available")?;
	let boot_nodes = Vec::new();

	Ok(ThxnetChainSpec::from_genesis(
		"THXNET. Mainnet",
		"thxnet_mainnet",
		ChainType::Live,
		move || thxnet_mainnet_config_genesis(wasm_binary),
		boot_nodes,
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		Some(thxnet_mainnet_chain_spec_properties()),
		Default::default(),
	))
}

/// Staging testnet config.
#[cfg(feature = "kusama-native")]
pub fn kusama_staging_testnet_config() -> Result<KusamaChainSpec, String> {
	let wasm_binary = kusama::WASM_BINARY.ok_or("Kusama development wasm not available")?;
	let boot_nodes = vec![];

	Ok(KusamaChainSpec::from_genesis(
		"Kusama Staging Testnet",
		"kusama_staging_testnet",
		ChainType::Live,
		move || kusama_staging_testnet_config_genesis(wasm_binary),
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(KUSAMA_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Kusama Staging telemetry url is valid; qed"),
		),
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// Westend staging testnet config.
#[cfg(feature = "westend-native")]
pub fn westend_staging_testnet_config() -> Result<WestendChainSpec, String> {
	let wasm_binary = westend::WASM_BINARY.ok_or("Westend development wasm not available")?;
	let boot_nodes = vec![];

	Ok(WestendChainSpec::from_genesis(
		"Westend Staging Testnet",
		"westend_staging_testnet",
		ChainType::Live,
		move || westend_staging_testnet_config_genesis(wasm_binary),
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(WESTEND_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Westend Staging telemetry url is valid; qed"),
		),
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// Rococo staging testnet config.
#[cfg(feature = "rococo-native")]
pub fn rococo_staging_testnet_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Rococo development wasm not available")?;
	let boot_nodes = vec![];

	Ok(RococoChainSpec::from_genesis(
		"Rococo Staging Testnet",
		"rococo_staging_testnet",
		ChainType::Live,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_staging_testnet_config_genesis(wasm_binary),
			session_length_in_blocks: None,
		},
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(ROCOCO_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Rococo Staging telemetry url is valid; qed"),
		),
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

pub fn versi_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"ss58Format": 42,
		"tokenDecimals": 12,
		"tokenSymbol": "VRS",
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Versi staging testnet config.
#[cfg(feature = "rococo-native")]
pub fn versi_staging_testnet_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Versi development wasm not available")?;
	let boot_nodes = vec![];

	Ok(RococoChainSpec::from_genesis(
		"Versi Staging Testnet",
		"versi_staging_testnet",
		ChainType::Live,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_staging_testnet_config_genesis(wasm_binary),
			session_length_in_blocks: Some(100),
		},
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(VERSI_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Versi Staging telemetry url is valid; qed"),
		),
		Some("versi"),
		None,
		Some(versi_chain_spec_properties()),
		Default::default(),
	))
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ImOnlineId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
	BeefyId,
) {
	let keys = get_authority_keys_from_seed_no_beefy(seed);
	(keys.0, keys.1, keys.2, keys.3, keys.4, keys.5, keys.6, keys.7, get_from_seed::<BeefyId>(seed))
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed_no_beefy(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ImOnlineId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<ValidatorId>(seed),
		get_from_seed::<AssignmentId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

fn testnet_accounts() -> Vec<AccountId> {
	vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
	]
}

/// Helper function to create polkadot `GenesisConfig` for testing
#[cfg(feature = "polkadot-native")]
pub fn polkadot_testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)>,
	_root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> polkadot::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * DOT;
	const STASH: u128 = 100 * DOT;

	polkadot::GenesisConfig {
		system: polkadot::SystemConfig { code: wasm_binary.to_vec() },
		indices: polkadot::IndicesConfig { indices: vec![] },
		balances: polkadot::BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		session: polkadot::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						polkadot_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: polkadot::StakingConfig {
			minimum_validator_count: 1,
			validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, polkadot::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::NotForcing,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		sudo: polkadot::SudoConfig { key: Some(_root_key) },
		phragmen_election: Default::default(),
		democracy: polkadot::DemocracyConfig::default(),
		council: polkadot::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: polkadot::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: polkadot::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(polkadot::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: polkadot::AuthorityDiscoveryConfig { keys: vec![] },
		claims: polkadot::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: polkadot::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: polkadot::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

/// Helper function to create kusama `GenesisConfig` for testing
#[cfg(feature = "kusama-native")]
pub fn kusama_testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)>,
	_root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> kusama::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * KSM;
	const STASH: u128 = 100 * KSM;

	kusama::GenesisConfig {
		system: kusama::SystemConfig { code: wasm_binary.to_vec() },
		indices: kusama::IndicesConfig { indices: vec![] },
		balances: kusama::BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		session: kusama::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						kusama_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: kusama::StakingConfig {
			minimum_validator_count: 1,
			validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, kusama::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::NotForcing,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: kusama::DemocracyConfig::default(),
		council: kusama::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: kusama::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: kusama::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(kusama::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: kusama::AuthorityDiscoveryConfig { keys: vec![] },
		claims: kusama::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: kusama::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		hrmp: Default::default(),
		configuration: kusama::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
		nis_counterpart_balances: Default::default(),
	}
}

/// Helper function to create westend `GenesisConfig` for testing
#[cfg(feature = "westend-native")]
pub fn westend_testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> westend::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * WND;
	const STASH: u128 = 100 * WND;

	westend::GenesisConfig {
		system: westend::SystemConfig { code: wasm_binary.to_vec() },
		indices: westend::IndicesConfig { indices: vec![] },
		balances: westend::BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		session: westend::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						westend_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: westend::StakingConfig {
			minimum_validator_count: 1,
			validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, westend::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::NotForcing,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		babe: westend::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(westend::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: westend::AuthorityDiscoveryConfig { keys: vec![] },
		vesting: westend::VestingConfig { vesting: vec![] },
		sudo: westend::SudoConfig { key: Some(root_key) },
		hrmp: Default::default(),
		configuration: westend::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		registrar: westend_runtime::RegistrarConfig {
			next_free_para_id: polkadot_primitives::LOWEST_PUBLIC_ID,
		},
		xcm_pallet: Default::default(),
		nomination_pools: Default::default(),
	}
}

/// Helper function to create rococo `GenesisConfig` for testing
#[cfg(feature = "rococo-native")]
pub fn rococo_testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> rococo_runtime::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * ROC;

	rococo_runtime::GenesisConfig {
		system: rococo_runtime::SystemConfig { code: wasm_binary.to_vec() },
		beefy: Default::default(),
		indices: rococo_runtime::IndicesConfig { indices: vec![] },
		balances: rococo_runtime::BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		session: rococo_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						rococo_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
							x.8.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		babe: rococo_runtime::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(rococo_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		phragmen_election: Default::default(),
		democracy: rococo::DemocracyConfig::default(),
		council: rococo::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: rococo::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		treasury: Default::default(),
		claims: rococo::ClaimsConfig { claims: vec![], vesting: vec![] },
		vesting: rococo::VestingConfig { vesting: vec![] },
		authority_discovery: rococo_runtime::AuthorityDiscoveryConfig { keys: vec![] },
		sudo: rococo_runtime::SudoConfig { key: Some(root_key.clone()) },
		hrmp: Default::default(),
		configuration: rococo_runtime::ConfigurationConfig {
			config: polkadot_runtime_parachains::configuration::HostConfiguration {
				max_validators_per_core: Some(1),
				..default_parachains_host_configuration()
			},
		},
		paras: rococo_runtime::ParasConfig { paras: vec![] },
		registrar: rococo_runtime::RegistrarConfig {
			next_free_para_id: polkadot_primitives::LOWEST_PUBLIC_ID,
		},
		xcm_pallet: Default::default(),
		nis_counterpart_balances: Default::default(),
	}
}

#[cfg(feature = "polkadot-native")]
fn polkadot_development_config_genesis(wasm_binary: &[u8]) -> polkadot::GenesisConfig {
	polkadot_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

#[cfg(feature = "kusama-native")]
fn kusama_development_config_genesis(wasm_binary: &[u8]) -> kusama::GenesisConfig {
	kusama_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

#[cfg(feature = "westend-native")]
fn westend_development_config_genesis(wasm_binary: &[u8]) -> westend::GenesisConfig {
	westend_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

#[cfg(feature = "rococo-native")]
fn rococo_development_config_genesis(wasm_binary: &[u8]) -> rococo_runtime::GenesisConfig {
	rococo_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Polkadot development config (single validator Alice)
#[cfg(feature = "polkadot-native")]
pub fn polkadot_development_config() -> Result<PolkadotChainSpec, String> {
	let wasm_binary = polkadot::WASM_BINARY.ok_or("Polkadot development wasm not available")?;

	Ok(PolkadotChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		move || polkadot_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		Some(polkadot_chain_spec_properties()),
		Default::default(),
	))
}

/// Kusama development config (single validator Alice)
#[cfg(feature = "kusama-native")]
pub fn kusama_development_config() -> Result<KusamaChainSpec, String> {
	let wasm_binary = kusama::WASM_BINARY.ok_or("Kusama development wasm not available")?;

	Ok(KusamaChainSpec::from_genesis(
		"Development",
		"kusama_dev",
		ChainType::Development,
		move || kusama_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// Westend development config (single validator Alice)
#[cfg(feature = "westend-native")]
pub fn westend_development_config() -> Result<WestendChainSpec, String> {
	let wasm_binary = westend::WASM_BINARY.ok_or("Westend development wasm not available")?;

	Ok(WestendChainSpec::from_genesis(
		"Development",
		"westend_dev",
		ChainType::Development,
		move || westend_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// Rococo development config (single validator Alice)
#[cfg(feature = "rococo-native")]
pub fn rococo_development_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Rococo development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Development",
		"rococo_dev",
		ChainType::Development,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_development_config_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// `Versi` development config (single validator Alice)
#[cfg(feature = "rococo-native")]
pub fn versi_development_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Versi development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Development",
		"versi_dev",
		ChainType::Development,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_development_config_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some("versi"),
		None,
		None,
		Default::default(),
	))
}

/// Wococo development config (single validator Alice)
#[cfg(feature = "rococo-native")]
pub fn wococo_development_config() -> Result<RococoChainSpec, String> {
	const WOCOCO_DEV_PROTOCOL_ID: &str = "woco";
	let wasm_binary = rococo::WASM_BINARY.ok_or("Wococo development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Development",
		"wococo_dev",
		ChainType::Development,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_development_config_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some(WOCOCO_DEV_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

#[cfg(feature = "polkadot-native")]
fn polkadot_local_testnet_genesis(wasm_binary: &[u8]) -> polkadot::GenesisConfig {
	polkadot_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Polkadot local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "polkadot-native")]
pub fn polkadot_local_testnet_config() -> Result<PolkadotChainSpec, String> {
	let wasm_binary = polkadot::WASM_BINARY.ok_or("Polkadot development wasm not available")?;

	Ok(PolkadotChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		ChainType::Local,
		move || polkadot_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		Some(polkadot_chain_spec_properties()),
		Default::default(),
	))
}

#[cfg(feature = "kusama-native")]
fn kusama_local_testnet_genesis(wasm_binary: &[u8]) -> kusama::GenesisConfig {
	kusama_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Kusama local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "kusama-native")]
pub fn kusama_local_testnet_config() -> Result<KusamaChainSpec, String> {
	let wasm_binary = kusama::WASM_BINARY.ok_or("Kusama development wasm not available")?;

	Ok(KusamaChainSpec::from_genesis(
		"Kusama Local Testnet",
		"kusama_local_testnet",
		ChainType::Local,
		move || kusama_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

#[cfg(feature = "westend-native")]
fn westend_local_testnet_genesis(wasm_binary: &[u8]) -> westend::GenesisConfig {
	westend_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Westend local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "westend-native")]
pub fn westend_local_testnet_config() -> Result<WestendChainSpec, String> {
	let wasm_binary = westend::WASM_BINARY.ok_or("Westend development wasm not available")?;

	Ok(WestendChainSpec::from_genesis(
		"Westend Local Testnet",
		"westend_local_testnet",
		ChainType::Local,
		move || westend_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

#[cfg(feature = "rococo-native")]
fn rococo_local_testnet_genesis(wasm_binary: &[u8]) -> rococo_runtime::GenesisConfig {
	rococo_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed("Alice"), get_authority_keys_from_seed("Bob")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Rococo local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "rococo-native")]
pub fn rococo_local_testnet_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Rococo development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Rococo Local Testnet",
		"rococo_local_testnet",
		ChainType::Local,
		move || RococoGenesisExt {
			runtime_genesis_config: rococo_local_testnet_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// Wococo is a temporary testnet that uses almost the same runtime as rococo.
#[cfg(feature = "rococo-native")]
fn wococo_local_testnet_genesis(wasm_binary: &[u8]) -> rococo_runtime::GenesisConfig {
	rococo_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
			get_authority_keys_from_seed("Dave"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Wococo local testnet config (multivalidator Alice + Bob + Charlie + Dave)
#[cfg(feature = "rococo-native")]
pub fn wococo_local_testnet_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Wococo development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Wococo Local Testnet",
		"wococo_local_testnet",
		ChainType::Local,
		move || RococoGenesisExt {
			runtime_genesis_config: wococo_local_testnet_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some(THXNET_DEFAULT_PROTOCOL_ID),
		None,
		None,
		Default::default(),
	))
}

/// `Versi` is a temporary testnet that uses the same runtime as rococo.
#[cfg(feature = "rococo-native")]
fn versi_local_testnet_genesis(wasm_binary: &[u8]) -> rococo_runtime::GenesisConfig {
	rococo_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Charlie"),
			get_authority_keys_from_seed("Dave"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// `Versi` local testnet config (multivalidator Alice + Bob + Charlie + Dave)
#[cfg(feature = "rococo-native")]
pub fn versi_local_testnet_config() -> Result<RococoChainSpec, String> {
	let wasm_binary = rococo::WASM_BINARY.ok_or("Versi development wasm not available")?;

	Ok(RococoChainSpec::from_genesis(
		"Versi Local Testnet",
		"versi_local_testnet",
		ChainType::Local,
		move || RococoGenesisExt {
			runtime_genesis_config: versi_local_testnet_genesis(wasm_binary),
			// Use 1 minute session length.
			session_length_in_blocks: Some(10),
		},
		vec![],
		None,
		Some("versi"),
		None,
		None,
		Default::default(),
	))
}
