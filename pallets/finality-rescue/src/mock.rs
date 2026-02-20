use frame_support::{construct_runtime, parameter_types, traits::{ConstU32, ConstU64}};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Grandpa: pallet_grandpa,
		FinalityRescue: crate,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const RescueCooldown: u64 = 10;
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_grandpa::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxAuthorities = ConstU32<100>;
	type MaxSetIdSessionEntries = ConstU64<0>;
	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RescueCooldown = RescueCooldown;
}

pub fn test_authorities() -> Vec<(pallet_grandpa::AuthorityId, u64)> {
	use sp_core::crypto::UncheckedFrom;
	vec![
		(pallet_grandpa::AuthorityId::unchecked_from([1u8; 32]), 1),
		(pallet_grandpa::AuthorityId::unchecked_from([2u8; 32]), 1),
		(pallet_grandpa::AuthorityId::unchecked_from([3u8; 32]), 1),
	]
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	use frame_support::traits::GenesisBuild;

	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	<pallet_grandpa::GenesisConfig as GenesisBuild<Test>>::assimilate_storage(
		&pallet_grandpa::GenesisConfig { authorities: test_authorities() },
		&mut t,
	)
	.unwrap();

	t.into()
}

pub fn new_test_ext_no_authorities() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
