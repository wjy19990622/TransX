use support::{decl_module, decl_storage, decl_event, dispatch::Result};
use support::traits::{Currency, ReservableCurrency, OnUnbalanced, Get};

use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature, Count, USD, Workforce};
use system::ensure_signed;
use sp_runtime::Permill;

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// The module's configuration trait.
pub trait Trait: system::Trait {
	type BTCLimitCount: Get<Count>;
	type ETHLimitCount: Get<Count>;
	type EOSLimitCount: Get<Count>;
	type USDTLimitCount: Get<Count>;
	type DCEPLimitCount: Get<Count>;
	type DOTLimitCount: Get<Count>;
	type DashLimitCount: Get<Count>;
	type ADALimitCount: Get<Count>;
	type DCAPLimitCount: Get<Count>;
	type TUSDTLimitCount: Get<Count>;
	type BTCMaxPortion: Get<Permill>;
	type ETHMaxPortion: Get<Permill>;
	type EOSMaxPortion: Get<Permill>;
	type USDTMaxPortion: Get<Permill>;
	type DCEPaxPortion: Get<Permill>;
	type DOTMaxPortion: Get<Permill>;
	type DashMaxPortion: Get<Permill>;
	type ADAMaxPortion: Get<Permill>;
	type DCAPMaxPortion: Get<Permill>;
	type TUSDTMaxPortion: Get<Permill>;
	type BTCLimitAmount: Get<USD>;
	type ETHLimitAmount: Get<USD>;
	type EOSLimitAmount: Get<USD>;
	type USDTLimitAmount: Get<USD>;
	type DCEPLimitAmount: Get<USD>;
	type DOTLimitAmount: Get<USD>;
	type DashLimitAmount: Get<USD>;
	type ADALimitAmount: Get<USD>;
	type DCAPLimitAmount: Get<USD>;
	type TUSDTLimitAmount: Get<USD>;
	type BTCMaxLimitAmount: Get<USD>;
	type ETHMaxLimitAmount: Get<USD>;
	type EOSMaxLimitAmount: Get<USD>;
	type USDTMaxLimitAmount: Get<USD>;
	type DCEPMaxLimitAmount: Get<USD>;
	type DOTMaxLimitAmount: Get<USD>;
	type DashMaxLimitAmount: Get<USD>;
	type ADAMaxLimitAmount: Get<USD>;
	type DCAPMaxLimitAmount: Get<USD>;
	type TUSDTMaxLimitAmount: Get<USD>;
	type InitialTotalCount: Get<Count>;
	type InitialTotalAmount: Get<USD>;
	type InitialTotalWorkforce: Get<Permill>;
	type FrequencyWorkforceProportion: Get<Permill>; 	    // α
	type AmountWorkforceProportion: Get<Permill>;			// β	
	type SenderWorkforceProportion: Get<Permill>;			// SR
	type ReceiverWorkforceProportion: Get<Permill>;		    // RR
	type SuperiorShareRatio: Get<Permill>;      			// SSR
	type OnsuperiorShareRatio: Get<Permill>;				// OSR
	type DailyMinimumReward: Get<BalanceOf<Self>>;			// MR
	type MinerSharefeeRatio: Get<Permill>;                  // MSR

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

#[cfg_attr(feature = "std",derive(Debug))]
pub struct MinerInfo<AccountId> {
    pub accountid: AccountId,
    pub mac: Vec<u8>,
    pub superiorAddress: Vec<u8>,
    pub onsuperiorAddress: Vec<u8>,
    pub registerTime: u64,
    pub status: Vec<u8>,
    pub miner: Vec<u8>,
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as Transx {

        /// store all miner's token address,include BTC,ETH,...
        pub TokenInfo get(token_info): map(T::AccountId,Option<Vec<u8>>) => Vec<u8>;


	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		// this is needed only if you are using events in your module
		fn deposit_event() = default;

        pub fn register_miner(
            origin,
            mac: Vec<u8>,
            superiorAddress: Vec<u8>,
            onsuperiorAddress: Vec<u8>,
            miner: Vec<u8>,
        ){
            Self::deposit_event(RawEvent::RegsitedMiner(origin));
        }
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
        RegsitedMiner(AccountId),
	}
);
