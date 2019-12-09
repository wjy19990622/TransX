use support::{debug,decl_storage, decl_module,decl_event, StorageValue, StorageMap,Parameter,
			  dispatch::Result, ensure,dispatch::Vec};
use system::{ensure_signed};
use sp_runtime::traits::{SimpleArithmetic, Bounded, One, Member,CheckedAdd};
use sp_runtime::traits::{Hash};

use codec::{Encode, Decode};

use crate::mine_linked::{PersonMineWorkForce,PersonMine,MineParm,PersonMineRecord};


pub trait Trait: balances::Trait + timestamp::Trait{
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type MineIndex: Parameter + Member + SimpleArithmetic + Bounded + Default + Copy;
}

type OwnerMineWorkForce<T> = PersonMineWorkForce<<T as system::Trait>::BlockNumber>;

// 对应 linked_item里面的函数调用
type OwnerWorkForceItem<T> = PersonMine<OwnerWorkForce<T>, <T as system::Trait>::AccountId,<T as system::Trait>::BlockNumber>;

// 只是结构体
type OwnerMineRecordItem<T> = PersonMineRecord<<T as timestamp::Trait>::Moment,<T as system::Trait>::BlockNumber,<T as balances::Trait>::Balance>;

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
    {
        Created(AccountId, Hash),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as MineStorage {
    	// 算力相关的
    	DayWorkForce get(day_workforce): map u64 => u64 ;    // 时间戳作为key,算力作为value.每天的平均算力
    	AvgWorkForce get(avg_workforce): u64;    // 以前所有天的平均算力

    	// 单个人的算力
    	OwnerMineRecord get(mine_record): map (T::AccountId, Option<T::MineIndex>) => Option<OwnerMineRecordItem<T>>;// 挖矿记录
    	OwnerWorkForce get(person_workforce): map T::AccountId => Option<OwnerMineWorkForce<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        pub fn create_mine(origin,action:Vec<u8>,address:Vec<u8>,symbol:Vec<u8>,amount:u64,precision:u64,protocol:Vec<u8>,decimal:u16,blockchain:Vec<u8>,memo:Vec<u8>) -> Result { // 创建挖矿
        	// 传入参数:
        	// {"action":"transfer","contract":"",
        	// "address":"0x86d1DA963b381Ad4278CaD0C27e95D80777399EB",
        	// "symbol":"ETH","amount":"100",
        	// "protocol":"ScanProtocol","decimal":18,
        	// "blockchain":"ETH",
        	// "memo":"hello,octa"}
        	let sender = ensure_signed(origin)?;
        	// 可能是获取 blockNum的接口
        	let block_number =<system::Module<T>>::block_number();
			let mine_parm = MineParm{action,
						address,
						symbol,
						amount,
						precision,
						protocol,
						decimal,
						blockchain,
						memo
				};
			Self::mining(mine_parm,sender)?;

			Ok(())
        }

    }
}




impl<T: Trait> Module<T> {
	fn mining(mine_parm:MineParm,sender: T::AccountId)->Result{
		let block_num = <system::Module<T>>::block_number(); // 获取区块的高度
		let now_time = <timestamp::Module<T>>::get();   // 记录到秒
		let balance = <T::Balance>::from(12);  // todo test,最后需要获取balance,从哪儿来?
		let balance1 = <T::Balance>::from(12);  // todo test
		balance.checked_add(&balance1).ok_or("balance overflow")?;
		let personMineRecord = PersonMineRecord::new(mine_parm,now_time,block_num,balance)?;
		<OwnerWorkForceItem<T>>::add(sender,0,block_num)?;
		Ok(())
	}


}