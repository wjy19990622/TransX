use support::{debug,decl_storage, decl_module,decl_event, StorageValue, StorageMap,Parameter,
			  dispatch::Result, ensure,dispatch::Vec,traits::Currency};
use system::{ensure_signed};
use sp_runtime::traits::{Hash,SimpleArithmetic, Bounded, One, Member,CheckedAdd};
//use node_primitives::BlockNumber;

use codec::{Encode, Decode};

use crate::mine_linked::{PersonMineWorkForce,PersonMine,MineParm,PersonMineRecord,BLOCK_NUMS};
//use node_primitives::BlockNumber;


pub trait Trait: balances::Trait + timestamp::Trait{
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type MineIndex: Parameter + Member + SimpleArithmetic + Bounded + Default + Copy;
}

type BlockNumberOf<T> = <T as system::Trait>::BlockNumber;  // u32

type OwnerMineWorkForce<T> = PersonMineWorkForce<<T as system::Trait>::BlockNumber>;

// 对应 linked_item里面的函数, 用于操作 PersonMineWorkForce 结构体
type OwnerWorkForceItem<T> = PersonMine<OwnedDayWorkForce<T>, <T as system::Trait>::AccountId,<T as system::Trait>::BlockNumber>;

// 只是结构体
type OwnerMineRecordItem<T> = PersonMineRecord<<T as timestamp::Trait>::Moment,<T as system::Trait>::BlockNumber,<T as balances::Trait>::Balance, <T as system::Trait>::AccountId>;

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
//		<T as Trait>::MineIndex,
    {
        Created(AccountId, Hash),
        Mined(AccountId,u64),  // 挖矿成功的事件
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as MineStorage {
    	// 算力相关的
    	DayWorkForce get(day_workforce): map u64 => u64 ;    // 时间戳作为key,算力作为value.每天的平均算力
    	AvgWorkForce get(avg_workforce): u64;    // 以前所有天的平均算力

    	//以下针对单个用户
    	OwnerMineRecord get(mine_record): map Vec<u8> => Option<OwnerMineRecordItem<T>>;// 挖矿记录, key形如:"btc" + "_" + "tx hash"  的 字节码
    	/// linked OwnerWorkForceItem,个人数据每天汇总
    	OwnedDayWorkForce get(person_workforce): map (T::AccountId,BlockNumberOf<T>) => Option<OwnerMineWorkForce<T>>;
    	OwnedMineIndex: map (T::AccountId,BlockNumberOf<T>) => u64;        // 用户每天挖矿次数
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        pub fn create_mine(origin,tx:Vec<u8>,address:Vec<u8>,to_address:Vec<u8>,symbol:Vec<u8>,amount:u64,protocol:Vec<u8>,decimal:u64,usdt_nums:u32,blockchain:Vec<u8>,memo:Vec<u8>) -> Result { // 创建挖矿
        	// 传入参数:
        	// {"action":"transfer","contract":"",  // 传入一定是 transfer
        	//  "tx":"eth_xxxxxxxx",      // 币名字 + "_" + "tx hash"  的 字节码.名字为小写
        	// "address":"0x86d1DA963b381Ad4278CaD0C27e95D80777399EB",
        	// "symbol":"ETH","amount":"100",
        	// "protocol":"ScanProtocol","decimal":18,
        	// "blockchain":"ETH",
        	// "memo":"hello,octa"}
        	let sender = ensure_signed(origin)?;
        	ensure!(!<OwnerMineRecord<T>>::exists(tx.clone()), "tx already exists");
        	ensure!(address != to_address,"you cannot transfer  to yourself");
        	ensure!(usdt_nums<u32::max_value(),"usdt_nums is overflow");
        	ensure!(usdt_nums>5,"usdt_nums is too small");
			let action = "transfer".as_bytes().to_vec();   // 固定 为 transfer
			let mine_parm = MineParm{action,
						tx,
						address,
						to_address,
						symbol,
						amount,
						protocol,
						decimal,
						usdt_nums,
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
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);
		// let now_day = block_num.checked_div(&day_block_nums).ok_or("mining function: div causes error")?;
		let now_day = block_num/day_block_nums;
		let now_time = <timestamp::Module<T>>::get();   // 记录到秒
		// test
		let balance = <T::Balance>::from(12);  // todo test,最后需要获取balance,从哪儿来?
		let balance1 = <T::Balance>::from(12);  // todo test
		let balance2 = balance.checked_add(&balance1).ok_or("balance overflow")?;
		#[cfg(feature = "std")]{
			println!("-----------begin:{:?},{:?},now:{:?}------------",now_time,block_num,now_day);
		}

		let owned_mineindex = <OwnedMineIndex<T>>::get(&(sender.clone(),now_day));
		if owned_mineindex > mining_maximum(){  // todo: maximum 通过其他函数调用
			return Err("your mining frequency exceeds the maximum frequency");
		}
		// 将挖矿记录进去
		let person_mine_record = PersonMineRecord::new(&mine_parm, sender.clone(),now_time, block_num, balance2)?;
		<OwnerMineRecord<T>>::insert(&mine_parm.tx,person_mine_record);
		<OwnerWorkForceItem<T>>::add(&sender,mine_parm.usdt_nums,now_day,block_num)?;
		// 将用户的挖矿记录+1
		let new_owned_mineindex = owned_mineindex.checked_add(1).ok_or("mining function add overflow")?;
		<OwnedMineIndex<T>>::insert(&(sender.clone(),now_day), new_owned_mineindex);
		#[cfg(feature = "std")]{
			println!("-----------four:{:?}------------",new_owned_mineindex);
		}
		Self::deposit_event(RawEvent::Mined(sender, new_owned_mineindex));
		Ok(())
	}

}

fn mining_maximum()-> u64{ //todo
	return 10;
}