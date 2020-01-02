use support::{debug,decl_storage, decl_module,decl_event, StorageValue, StorageMap,Parameter,
			  dispatch::Result, ensure,dispatch::Vec,traits::Currency, StorageDoubleMap};
use support::traits::{Get, ReservableCurrency};
use system::{ensure_signed};
use sp_runtime::traits::{Hash,SimpleArithmetic, Bounded, One, Member,CheckedAdd};

use codec::{Encode, Decode};
use crate::mine_linked::{PersonMineWorkForce,PersonMine,MineParm,PersonMineRecord,BLOCK_NUMS};
//use node_primitives::BlockNumber;
use crate::register::{self,MinersCount,AllMiners,Trait as RegisterTrait};
use crate::mine_power::{PowerInfo, MinerPowerInfo, TokenPowerInfo, PowerInfoStore, MinerPowerInfoStore, TokenPowerInfoStore};
use rstd::{result};
use sp_runtime::traits::{Zero};

// 继承 register 模块,方便调用register里面的 store
pub trait Trait: balances::Trait + RegisterTrait{
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type MineIndex: Parameter + Member + SimpleArithmetic + Bounded + Default + Copy;
//	type TranRuntime: RegisterTrait;
	// 算力归档时间，到达这个时间，则将`WorkforceInfo`信息写入到链上并不再修改。
	type ArchiveDuration: Get<Self::BlockNumber>;
}

type BlockNumberOf<T> = <T as system::Trait>::BlockNumber;  // u32

type OwnerMineWorkForce<T> = PersonMineWorkForce<<T as system::Trait>::BlockNumber>;

// 对应 linked_item里面的函数, 用于操作 PersonMineWorkForce 结构体
type OwnerWorkForceItem<T> = PersonMine<OwnedDayWorkForce<T>, <T as system::Trait>::AccountId,<T as system::Trait>::BlockNumber>;

// 只是结构体
type OwnerMineRecordItem<T> = PersonMineRecord<<T as timestamp::Trait>::Moment,<T as system::Trait>::BlockNumber,<T as balances::Trait>::Balance, <T as system::Trait>::AccountId>;

type PowerInfoItem<T> = PowerInfo<<T as system::Trait>::BlockNumber>;
type TokenPowerInfoItem<T> = TokenPowerInfo<<T as system::Trait>::BlockNumber>;
type MinerPowerInfoItem<T> = MinerPowerInfo<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>;
type PowerInfoStoreItem<T> = PowerInfoStore<PowerInfoList<T>, <T as system::Trait>::BlockNumber>;
type TokenPowerInfoStoreItem<T> = TokenPowerInfoStore<TokenPowerInfoList<T>, <T as system::Trait>::BlockNumber>;
type MinerPowerInfoStoreItem<T> = MinerPowerInfoStore<MinerPowerInfoDict<T>, <T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>;


decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
//		<T as Trait>::MineIndex,
		<T as system::Trait>::BlockNumber,
    {
        Created(AccountId, Hash),
        Mined(AccountId,u64),  // 挖矿成功的事件
        PowerInfoArchived(BlockNumber),
        TokenPowerInfoArchived(BlockNumber),
        MinerPowerInfoArchived(BlockNumber),
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

    	// `PowerInfoList`存储每日的全网算力信息，key为`ChainRunDays`，value为`PowerInfo`。
        // 当key为`ChainRunDays`时，表示获取当日的全网算力，key=[1..`ChainRunDays`-1]获取历史的算力信息。
        // 当每日结束时，`ChainRunDays`+1，开始存储计算下一个日期的算力信息。
        PowerInfoList get(fn power_info): map u32 => Option<PowerInfoItem<T>>;

        // `TokenPowerInfoList`存储每日的Token交易信息，与`PowerInfoList`类似。
        TokenPowerInfoList get(fn token_power_info): map u32 => Option<TokenPowerInfoItem<T>>;

		// `MinerPowerInfoDict`存储每个矿工当日与前一日的挖矿算力信息。第一个参数与MinerPowerInfoPrevPoint相关。
        MinerPowerInfoDict get(fn miner_power_info): double_map u32, twox_128(T::AccountId) => Option<MinerPowerInfoItem<T>>;

        // `MinerPowerInfoPrevPoint`用来区分存储前一天矿工算力信息的。
        // = 0，表示第一天挖矿，矿工还不存在前一日算力信息。
        // = 1，表示前一天挖矿信息保存在`MinerPowerInfoDict(1, AccountId)`中。
        // = 2，表示前一天挖矿信息保存在`MinerPowerInfoDict(2, AccountId)`中。
        MinerPowerInfoPrevPoint: u32;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        const ArchiveDuration: T::BlockNumber = T::ArchiveDuration::get();

        pub fn create_mine(origin,tx:Vec<u8>,address:Vec<u8>,to_address:Vec<u8>,symbol:Vec<u8>,amount:u64,protocol:Vec<u8>,decimal:u64,usdt_nums:u32,blockchain:Vec<u8>,memo:Vec<u8>) -> Result { // 创建挖矿
        	// 传入参数: todo: 还是 amount/10.pow(decimal)
        	// {"action":"transfer","contract":"",  // 传入一定是 transfer
        	//  "tx":"eth_xxxxxxxx",      // 币名字 + "_" + "tx hash"  的 字节码.名字为小写
        	// "address":"0x86d1DA963b381Ad4278CaD0C27e95D80777399EB",
        	// "symbol":"ETH","amount":"100",
        	// "protocol":"ScanProtocol","decimal":18,
        	// "blockchain":"ETH",
        	// "memo":"hello,octa"}  reserved_balance
        	let sender = ensure_signed(origin)?;
        	ensure!(<AllMiners<T>>::exists(sender.clone()), "account not register");
        	ensure!(T::Currency1::reserved_balance(&sender)>=T::PledgeAmount::get(),"your reservable currency is not enough");
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

		fn on_finalize(block_number: T::BlockNumber) {
            if (block_number % T::ArchiveDuration::get()).is_zero() {
                Self::archive(block_number);
            }
        }
    }
}

impl<T: Trait> Module<T> {
	fn mining(mine_parm:MineParm,sender: T::AccountId)->Result{
		ensure!(<AllMiners<T>>::exists(sender.clone()), "account not register");
//		<register::Module<T>>::add_token_info([1,2,3].to_vec(),[1,2,3].to_vec());
//		register::Call::<T>::add_token_info([1,2,3].to_vec(),[1,2,3].to_vec());
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

    // 获取存储矿工算力信息的指示
    fn miner_power_info_point() -> (u32, u32) {
        let prev_point = <MinerPowerInfoPrevPoint>::get();
        let curr_point = match prev_point {
            1 => 2,
            2 => 1,
            _ => 0,
        };
        (prev_point, curr_point)
    }


	// 将当日挖矿信息进行归档，不可更改地存储在网络中。
	fn archive(block_number: T::BlockNumber) {
		// 对算力信息和Token算力信息进行归档
		<PowerInfoStoreItem<T>>::archive(block_number.clone()).unwrap();
		Self::deposit_event(RawEvent::PowerInfoArchived(block_number.clone()));

		<TokenPowerInfoStoreItem<T>>::archive(block_number.clone()).unwrap();
		Self::deposit_event(RawEvent::TokenPowerInfoArchived(block_number.clone()));

		// 对矿工的挖矿信息进行归档
		let (prev_point, curr_point) = Self::miner_power_info_point();
		if curr_point == 0 {
			// 当日和昨日的矿工算力信息均不存在，无需归档
			return;
		}

		// 删除前一日的矿工算力数据，并将今日的算力作为前一日的矿工算力。
		<MinerPowerInfoStoreItem<T>>::archive(prev_point, block_number.clone());
		<MinerPowerInfoPrevPoint>::put(curr_point);
		Self::deposit_event(RawEvent::MinerPowerInfoArchived(block_number.clone()));

	}

	// 计算矿工一次挖矿的算力，coin_amount指本次交易以USDT计价的金额
	fn calculate_workforce(miner_id: &T::AccountId, block_number: T::BlockNumber, coin_name: &str, coin_number: f64, coin_price: f64)
		-> result::Result<u64, &'static str> {
        let (prev_point, curr_point) = Self::miner_power_info_point();
        let miner_power_info = <MinerPowerInfoStoreItem<T>>::get_miner_power_info(curr_point, miner_id, block_number.clone());
        let prev_miner_power_info = <MinerPowerInfoStoreItem<T>>::get_miner_power_info(prev_point, miner_id, block_number.clone());
        let prev_power_info = <PowerInfoStoreItem<T>>::get_prev_power(block_number.clone());
        let prev_token_power_info = <TokenPowerInfoStoreItem<T>>::get_prev_token_power(block_number.clone());
        let miner_numbers = <MinersCount>::get();

        let alpha = 0.3;
        let beta = 1.0 - alpha;
        let sr = 0.5;
        match coin_name {
            "btc" => {
                let lc_btc = 100u64;
                ensure!(miner_power_info.btc_count <= lc_btc, "BTC mining count runs out today");

                // 计算矿机P一次BTC转账的频次算力PCW btc = α * 1 / TC / PPC btc ( PC btc < LC btc )
                // 矿机P计算BTC频次算力钝化系数，PPC btc = ( (PC btc + 1 ) / AvC btc ) % 10
                let avc_btc = prev_token_power_info.btc_total_count.checked_div(miner_numbers).ok_or("Calc AvC btc causes overflow")?;
                let ppc_btc_divisor = prev_miner_power_info.btc_count.checked_add(1).ok_or("Calc PPC btc divisor causes overflow")?;
                let divisor = ppc_btc_divisor.checked_div(avc_btc).ok_or("Calc PPC btc parameter causes overflow")?;
                let ppc_btc = divisor % 10;

                let mut tc = prev_power_info.total_count;
                if tc == 0 {
                    tc = 100;
                }
                let pcw_btc:f64 = alpha * ppc_btc as f64 / tc as f64;

                let mut pa = prev_miner_power_info.total_amount;
                if pa < 10 {
                    pa = 1000;
                }

                let coin_amount = (coin_price * coin_number) as u64;
                // PPA btc	矿机P计算BTC金额算力钝化系数	PPA btc = ( (Price(BTC) * m btc +PAbtc ) / AvA btc ) % 10
                let ava_btc = prev_token_power_info.btc_total_amount.checked_div(miner_numbers).ok_or("Calc AvA btc causes overflow")?;
                let ppa_btc_divisor = coin_amount + prev_token_power_info.btc_total_amount;
                let divisor = ppa_btc_divisor.checked_div(avc_btc).ok_or("Calc PPC btc parameter causes overflow")?;
                let ppa_btc = divisor % 10;
                let paw_btc = beta * coin_number * coin_price * ppa_btc as f64 / pa as f64;
                let pw_btc:f64 = (pcw_btc + paw_btc) * sr;

                return Ok(pw_btc as u64);
            }

            _ => return Err("Unsupported token")
        }

		Ok(10u64)
	}
}

fn mining_maximum()-> u64{ //todo
	return 10;
}
