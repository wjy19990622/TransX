#![cfg_attr(not(feature = "std"), no_std)]

use rstd::prelude::*;
use rstd::{cmp, result, mem, fmt::Debug};
use codec::{Codec, Encode, Decode,Input,Output};
use support::{decl_module, decl_storage, decl_event, dispatch::Result};
use support::traits::{Currency, ReservableCurrency, OnUnbalanced, Get};

use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature, Count, USD, Workforce};
use system::ensure_signed;
use sp_runtime::Permill;
use sp_runtime::RuntimeDebug;
use sp_runtime::traits::StaticLookup;

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;


/// The module's configuration trait.
pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	/// The currency trait.
	type Currency: Currency<Self::AccountId>;
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
    type PledgeAmount: Get<BalanceOf<Self>>;
	type SettlePeriodBlock: Get<BlockNumber>;
	type FoundingTeamProportion: Get<Permill>;
}

#[cfg_attr(feature = "std",derive(Debug))]
#[derive(PartialEq, Eq, Clone, Encode, Decode)]
pub struct MinerInfo<AccountId> {
    pub accountid: AccountId,
    pub mac: Vec<u8>,
    pub superior_address: AccountId,
    pub onsuperior_address: AccountId,
    pub register_time: Moment,
    pub status: Vec<u8>,
    pub miner: Vec<u8>,
}


// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as Transx {

        /// store all miner's token address,include BTC,ETH,...
        pub TokenInfo get(token_info): map(T::AccountId,Option<Vec<u8>>) => Vec<u8>;

        /// store all miner's minerinfo
        pub OwnedMinerInfo get(owned_minerinfo): map(T::AccountId) => Option<MinerInfo<AccountId>>;
	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events this is needed only if you are using events in your module
		fn deposit_event() = default;

        /// register miner
        pub fn register_miner(origin,superior_address: <T::Lookup as StaticLookup>::Source,onsuperior_address: <T::Lookup as StaticLookup>::Source) -> Result{
            let who = ensure_signed(origin)?;
            // todo register process
            Self::deposit_event(RawEvent::RegsitedMiner(who));
            Ok(())
        }

        /// miner bond other token's address,etc, BTC
        pub fn bond_address(origin, addresses: Vec<(Vec<u8>, Vec<u8>)>) -> Result {
            let who = ensure_signed(origin)?;
            // todo bonding process
            Self::deposit_event(RawEvent::BondedAddress(who));
            Ok(())
        }

        /// update bond other token's address
        // need pay a very high fee,because the system don't encourage user change bond address
        pub fn update_bond_address(origin,token_symbol: Vec<u8>,token_address: Vec<u8>) -> Result{
            let who = ensure_signed(origin)?;
            // todo update bonding process
            Self::deposit_event(RawEvent::UpdatedBondAddress(who));
            Ok(())
        }
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
        RegsitedMiner(AccountId),
        BondedAddress(AccountId),
        UpdatedBondAddress(AccountId),
	}
);
