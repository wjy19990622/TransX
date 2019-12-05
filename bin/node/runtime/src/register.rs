
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
use support::traits::{Get,
	Currency, ReservableCurrency
};
use rstd::prelude::*;
use support::{debug, ensure, decl_module, decl_storage, decl_event, dispatch::Result, weights::{SimpleDispatchInfo}, StorageValue, StorageMap, StorageDoubleMap, Blake2_256};
use system::ensure_signed;
use timestamp;
use codec::{Encode, Decode};

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct MinerInfo<A, M> {
	hardware_id: Vec<u8>,
	father_address: A,
	grandpa_address: Option<A>,
	register_time: M,
	machine_state: Vec<u8>,  // 暂时用字符串表示
	machine_owner: A,
}


pub trait Trait: timestamp::Trait + system::Trait {

	/// The overarching event type.
	type Bond: Get<BalanceOf<Self>>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
}


decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
		// Just a dummy storage item.
		AllMiners get(fn allminers): map T::AccountId => MinerInfo<T::AccountId, T::Moment>;
		TokenInfo: double_map T::AccountId, blake2_256(Vec<u8>) => Vec<u8>;
		AllRegisters get(fn allregisters):  map Vec<u8> => T::AccountId;
		MinersCount: u64;
	}
}


decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		fn deposit_event() = default;

		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		pub fn register(origin, hardware_id: Vec<u8>, father_Address: T::AccountId, machine_state: Vec<u8>) -> Result{
			/// register the machine.
			let who = ensure_signed(origin)?;
			ensure!(!<AllMiners<T>>::exists(who.clone()), "you have been registed!");
			// 账户已经存在不需要注册！

			ensure!(!<AllRegisters<T>>::exists(hardware_id.clone()), "the hardware_id is exists!");
			// 硬件已经被注册则不能再次注册。

			let bond :BalanceOf<T> = T::Bond::get();
			debug::RuntimeLogger::init();
			debug::print!("bond---------------------------------{:?}", bond);
			T::Currency::reserve(&who, bond.clone())
				.map_err(|_| "Proposer's balance too low, you can't registe!")?;
			// 抵押不够不给注册

			let register_time = <timestamp::Module<T>>::get();
			// 添加注册时间

			ensure!(!(who.clone()==father_Address.clone()), "the father_address can't be youself!");
			// 上上级不能是自己本身。

			let mut minerinfo = MinerInfo{
				hardware_id:  hardware_id.clone(),
				father_address: father_Address.clone(),
				grandpa_address: None,  // 上上级默认是None
				register_time: register_time.clone(),
				machine_state: machine_state,
				machine_owner: who.clone(),
			};

			if <AllMiners<T>>::exists(father_Address.clone()){
				let grandpa = Self::allminers(father_Address.clone()).father_address;
				minerinfo.grandpa_address = Some(grandpa);
			}
			// 如果存在上级 则添加上上级 如果不存在则上级是None

			<AllMiners<T>>::insert(who.clone(), minerinfo.clone());
			// 添加矿机信息完毕

			<AllRegisters<T>>::insert(hardware_id.clone(), who.clone());
			// 添加映射 矿机id => 用户id

			let allminerscount = MinersCount::get();
			let new_allminerscount = allminerscount.checked_add(1).ok_or("Overflow adding a miner to total supply!")?;
			MinersCount::put(new_allminerscount);
			// 矿机数加1

			Self::deposit_event(RawEvent::RegisterEvent(allminerscount, who.clone(), register_time.clone()));
			// 触发事件

			Ok(())
		}

		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		pub fn kill_register(origin) -> Result{
			/// 注销注册的账户 并归还抵押金额
			let who = ensure_signed(origin)?;

			ensure!(<AllMiners<T>>::exists(who.clone()), "you have been not registered!");
			// 如果还没有注册， 则直接退出

			let bond :BalanceOf<T> = T::Bond::get();
			T::Currency::unreserve(&who, bond.clone());
			// 归还抵押

			let hardware_id = <AllMiners<T>>::get(who.clone()).hardware_id;
			// 获取硬件id

			<AllMiners<T>>::remove(who.clone());
			// 从矿机列表删除该账户

			<AllRegisters<T>>::remove(hardware_id.clone());
			// 从AllRegisters列表中删除记录

			let minercount = MinersCount::get();
			let new_minercount = minercount - 1;
			MinersCount::put(new_minercount);
			// 矿机数减掉1

			<TokenInfo<T>>::remove_prefix(who.clone());
			//删除掉相关的tokeninfo

			Self::deposit_event(RawEvent::KillRegisterEvent(who.clone()));

			Ok(())
		}


		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		pub fn add_token_info(origin, tokenaddress_add_symble: Vec<u8>, tokenaddress: Vec<u8>) -> Result{
			/// 给注册过的用户添加token信息
			let who = ensure_signed(origin)?;
			ensure!(<AllMiners<T>>::exists(who.clone()), "you have been not registered!");
			// 如果还没有注册， 则直接退出

			ensure!(!<TokenInfo<T>>::exists(who.clone(), tokenaddress_add_symble.clone()), "the token info have been existsting.");
			// 如果已经存在这个token信息  则不再添加。

			<TokenInfo<T>>::insert(who.clone(), tokenaddress_add_symble.clone(), tokenaddress.clone());
			Self::deposit_event(RawEvent::AddTokenInfoEvent(who, tokenaddress_add_symble));

			Ok(())

			}

		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		pub fn remove_token_info(origin, tokenaddress_add_symble: Vec<u8>) -> Result{
			let who = ensure_signed(origin)?;
			ensure!(<AllMiners<T>>::exists(who.clone()), "you have been not registered!");
			// 不是已经注册的账户，不可查。
			ensure!(<TokenInfo<T>>::exists(who.clone(), tokenaddress_add_symble.clone()), "the token info not exists.");
			// 如果本来就不存在， 则退出。

			<TokenInfo<T>>::remove(who.clone(), tokenaddress_add_symble.clone());
			// 删除该key

			Self::deposit_event(RawEvent::RemoveTokenInfoEvent(who, tokenaddress_add_symble));
			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T> where
	 <T as system::Trait>::AccountId,
	 <T as timestamp::Trait>::Moment {
		// Just a dummy event.

		RegisterEvent(u64, AccountId, Moment),
		AddTokenInfoEvent(AccountId, Vec<u8>),
		RemoveTokenInfoEvent(AccountId, Vec<u8>),
		KillRegisterEvent(AccountId),
	}
);

// tests for this module
//#[cfg(test)]
//mod tests {
//	use super::*;
//
//	use primitives::H256;
//	use support::{impl_outer_origin, assert_ok, parameter_types, weights::Weight};
//	use sp_runtime::{
//		traits::{BlakeTwo256, IdentityLookup}, testing::Header, Perbill,
//	};
//
//	impl_outer_origin! {
//		pub enum Origin for Test {}
//	}
//
//	// For testing the module, we construct most of a mock runtime. This means
//	// first constructing a configuration type (`Test`) which `impl`s each of the
//	// configuration traits of modules we want to use.
//	#[derive(Clone, Eq, PartialEq)]
//	pub struct Test;
//	parameter_types! {
//		pub const BlockHashCount: u64 = 250;
//		pub const MaximumBlockWeight: Weight = 1024;
//		pub const MaximumBlockLength: u32 = 2 * 1024;
//		pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
//	}
//	impl system::Trait for Test {
//		type Origin = Origin;
//		type Call = ();
//		type Index = u64;
//		type BlockNumber = u64;
//		type Hash = H256;
//		type Hashing = BlakeTwo256;
//		type AccountId = u64;
//		type Lookup = IdentityLookup<Self::AccountId>;
//		type Header = Header;
//		type Event = ();
//		type BlockHashCount = BlockHashCount;
//		type MaximumBlockWeight = MaximumBlockWeight;
//		type MaximumBlockLength = MaximumBlockLength;
//		type AvailableBlockRatio = AvailableBlockRatio;
//		type Version = ();
//	}
//	impl Trait for Test {
//		type Event = ();
//
//
//	}
//	type TemplateModule = Module<Test>;
//
//	// This function basically just builds a genesis storage key/value store according to
//	// our desired mockup.
//	fn new_test_ext() -> runtime_io::TestExternalities {
//		system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
//	}
//
//	#[test]
//	fn it_works_for_default_value() {
//		new_test_ext().execute_with(|| {
//			// Just a dummy test for the dummy funtion `do_something`
//			// calling the `do_something` function with a value 42
//			assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
//			// asserting that the stored value is equal to what we stored
//			assert_eq!(TemplateModule::something(), Some(42));
//		});
//	}
//}
