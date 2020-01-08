use support::{debug,decl_storage, decl_module,decl_event, StorageValue, StorageMap,Parameter,
			  dispatch::Result, Blake2_256, ensure,dispatch::Vec,traits::Currency, StorageDoubleMap};
use support::traits::{Get, ReservableCurrency};
use system::{ensure_signed};
use sp_runtime::traits::{Hash,SimpleArithmetic, Bounded, One, Member,CheckedAdd, Zero};
use sp_runtime::{Permill};
use codec::{Encode, Decode};
use crate::mine_linked::{PersonMineWorkForce, PersonMine, MineParm, PersonMineRecord, BLOCK_NUMS, MineTag};
//use node_primitives::BlockNumber;
use crate::register::{self,MinersCount,AllMiners,Trait as RegisterTrait};
use crate::mine_power::{PowerInfo, MinerPowerInfo, TokenPowerInfo, PowerInfoStore, MinerPowerInfoStore, TokenPowerInfoStore};
use node_primitives::{Count, USD, PercentU64};
use rstd::{result};

use rstd::prelude::*;
use rstd::convert::TryInto;


// 继承 register 模块,方便调用register里面的 store
pub trait Trait: balances::Trait + RegisterTrait{
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type MineIndex: Parameter + Member + SimpleArithmetic + Bounded + Default + Copy;
//	type TranRuntime: RegisterTrait;
	// 算力归档时间，到达这个时间，则将`WorkforceInfo`信息写入到链上并不再修改。
	type ArchiveDuration: Get<Self::BlockNumber>;
	type RemovePersonRecordDuration: Get<Self::BlockNumber>;

	// 第一年挖矿每天奖励token数
	type FirstYearPerDayMineRewardToken: Get<BalanceOf<Self>>;

	type BTCLimitCount: Get<Count>;
	type BTCLimitAmount: Get<USD>;
	type MiningMaximum: Get<Count>;

	type BTCMaxPortion: Get<Permill>;
	type ETHMaxPortion: Get<Permill>;
	type EOSMaxPortion: Get<Permill>;
	type USDTMaxPortion: Get<Permill>;

	type SuperiorShareRatio: Get<PercentU64>;
	type OnsuperiorShareRatio: Get<PercentU64>;





}

type StdResult<T> = core::result::Result<T, &'static str>;
type BalanceOf<T> = <T as balances::Trait>::Balance;

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

		// id与交易天数的映射
		MinerDAYS get(fn minertxdays): map T::AccountId => Vec<T::BlockNumber>;

		// 个人所有天数的交易hash（未清除）
		MinerAllDaysTx get(fn mineralldaystx): double_map hasher(twox_64_concat) T::AccountId, blake2_256(T::BlockNumber) => Vec<Vec<u8>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        const ArchiveDuration: T::BlockNumber = T::ArchiveDuration::get();

        pub fn create_mine(origin,mine_tag: MineTag, tx: Vec<u8>, address: Vec<u8>,to_address:Vec<u8>,symbol:Vec<u8>,amount:u64,protocol:Vec<u8>,decimal:u64,usdt_nums:u32,blockchain:Vec<u8>,memo:Vec<u8>) -> Result { // 创建挖矿
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

        	// 有两种挖矿方式  所以一比交易最多能够进行两次挖矿
        	ensure!(!(<OwnerMineRecord<T>>::exists(tx.clone())
        	&& (<OwnerMineRecord<T>>::get(tx.clone()).unwrap().mine_tag == mine_tag.clone()  ||  (<OwnerMineRecord<T>>::get(tx.clone()).unwrap().mine_count >= 2u16))), "tx already exists");
        	// TODO 这里说明了一个挖矿tx只能一次被挖（哪里有数据期限呢）  这个数据要永久保存？？？？？？

			// 该币的全网挖矿算力大于一定的比例  则不再挖矿
			ensure!(!Self::is_token_power_more_than_portion(symbol.clone()), "this token is more than max portion today.");

        	let block_num = <system::Module<T>>::block_number();
        	let now_tokenpowerinfo = <TokenPowerInfoStoreItem<T>>::get_curr_token_power(block_num);
        	let PCbtc = now_tokenpowerinfo.btc_total_count;
        	let PAbtc =  now_tokenpowerinfo.btc_total_amount;
        	ensure!(!(T::BTCLimitCount::get() <= PCbtc || T::BTCLimitAmount::get() <= PAbtc), "btc count or amount allreadey enough.");


			Self::remove_expire_record(sender.clone(), false);

			let mut mine_count = 1u16;
			// 如果交易已经进入队列，说明正在进行第二次挖矿，挖矿次数加1
        	if <OwnerMineRecord<T>>::exists(tx.clone()){
        		mine_count += 1;
        	}
        	// 如果是第一次添加该比交易 则去添加今天的日期进队列   如果已经存在不需要添加
        	else{
				 // 获取区块的高度
				let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);  // wjy 一天出多少块
				let now_day = block_num/day_block_nums;
				// 获取本人的所有有记录的天数
				let all_days = <MinerDAYS<T>>::get(sender.clone());
				if all_days.is_empty(){
					let days = vec![now_day];
					<MinerDAYS<T>>::insert(sender.clone(), days);
				}
				else{
					if !all_days.contains(&now_day){

						let mut days = all_days.clone();
						days.push(now_day);
						// 先删除再增加
						<MinerDAYS<T>>::remove(sender.clone());
						<MinerDAYS<T>>::insert(sender.clone(), days);
					}
				}
        	}

			let action = "transfer".as_bytes().to_vec();   // 固定 为 transfer
			let mine_parm = MineParm{
						mine_tag,
						mine_count,
						action,  // action是字符串  这里先定义下来（substrate是Vec<u8>)
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
		if owned_mineindex > T::MiningMaximum::get(){  // todo: maximum 通过其他函数调用
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


	fn remove_expire_record(who: T::AccountId, is_remove_all: bool) {
		/// 删除过期记录
		let block_num = <system::Module<T>>::block_number(); // 获取区块的高度
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);  // wjy 一天出多少块
		let now = block_num / day_block_nums;
		if <MinerDAYS<T>>::exists(&who) {
			// 如果里面包含数据
			let all_days = <MinerDAYS<T>>::get(&who);
//			let mut all_new_days = all_days.clone();
			if !all_days.is_empty() {
				// 如果是删除全部（提供给外部模块， 这个模块不使用）
				if is_remove_all{
					for day in all_days.iter() {
						Self::remove_per_day_record(day.clone(), who.clone());
						}
				}
					// 正常删除
				else{
					for day in all_days.iter() {
						if now.clone() - day.clone() >= T::RemovePersonRecordDuration::get(){
						Self::remove_per_day_record(day.clone(), who.clone());
					}
					}
				}
			}
		}
	}

	// 删除被选中的那天的记录
	fn remove_per_day_record(day: T::BlockNumber, who: T::AccountId) {
		let mut all_days = <MinerDAYS<T>>::get(&who);
		let all_tx = <MinerAllDaysTx<T>>::get(who.clone(), day.clone());
		//如果当天交易存在 那么就删除掉
		if !all_tx.is_empty() {
			for tx in all_tx.iter() {
				<OwnerMineRecord<T>>::remove(tx.clone());  // tx不能直接用remove方法来删除？？？？？？？？
			}
			// 把过期的交易清除
		}
		<MinerAllDaysTx<T>>::remove(who.clone(), day.clone());
		if let Some(pos) = all_days.iter().position(|a| a == &day) {
			all_days.swap_remove(pos);
			// 更新本人的未删除记录
			<MinerDAYS<T>>::insert(who.clone(), all_days.clone())
		}
	}

	fn is_token_power_more_than_portion(symbol: Vec<u8>) -> bool{  //
		/// 判断该token在全网算力是否超额
		// 小写传进来

		let mut is_too_large: bool = false;
		let mut max_portion: Permill = Permill::from_percent(0);
		let block_num = <system::Module<T>>::block_number();

		let now_tokenpower_info =  <TokenPowerInfoStoreItem<T>>::get_prev_token_power(block_num.clone());
		let power_info = <PowerInfoStoreItem<T>>::get_prev_power(block_num.clone());
		let all_token_power_total = power_info.total_power;

		if power_info.total_count >= 1000 || power_info.total_amount >= 100_0000{

			if symbol == "btc".as_bytes().to_vec(){
					max_portion = T::BTCMaxPortion::get();
					if (now_tokenpower_info.btc_total_power*100u64/all_token_power_total) as u32 > max_portion*100{
						is_too_large = true;
					}
			}

			else if symbol == "eth".as_bytes().to_vec(){
				max_portion = T::ETHMaxPortion::get();
				if (now_tokenpower_info.eth_total_power*100u64/all_token_power_total) as u32 > max_portion*100{
						is_too_large = true;
					}
			}

			else if symbol == "eos".as_bytes().to_vec(){
				max_portion = T::EOSMaxPortion::get();
				if (now_tokenpower_info.eos_total_power*100u64/all_token_power_total) as u32 > max_portion*100{
						is_too_large = true;
					}
				}

			else if  symbol == "usdt".as_bytes().to_vec(){
				max_portion = T::USDTMaxPortion::get();
				if (now_tokenpower_info.usdt_total_power/all_token_power_total) as u32 > max_portion*100{
						is_too_large = true;
					}
				}
			}

		is_too_large
		}


	fn per_day_mine_reward_token() -> Option<T::Balance>{
		/// 计算每一天的挖矿奖励
		let block_num = <system::Module<T>>::block_number(); // 获取区块的高度
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);  // wjy 一天出多少块
		let now:u32  = (block_num / day_block_nums).try_into().ok()?.try_into().ok()?;

		let e = now/(36525*4/100);  //一年365.25天来进行计算
		if e > 32{
			Some(<BalanceOf<T>>::from(0))  // 128年之后的挖矿奖励基本为0 所以这时候可以终止了 继续没必要
		}
		else{
			let num = 2_u32.pow(e);  // 意味着e最大值是32  运行32*4 = 128年
			let per_day_tokens = T::FirstYearPerDayMineRewardToken::get()/<BalanceOf<T>>::from(num); // 2的n次方怎么形容
			Some(per_day_tokens)
		}

	}

	fn inflate_power(who: T::AccountId, mine_power: u64) -> u64{
		/// 计算膨胀算力
		let mut grandpa = 0;
		let mut father = 0;
		if let Some(father_address) = <AllMiners<T>>::get(who.clone()).father_address{
			father = T::OnsuperiorShareRatio::get();
		};
		if let Some(grandpa_address) = <AllMiners<T>>::get(who.clone()).grandpa_address{
			grandpa = T::SuperiorShareRatio::get();
		};
		let flate_power = mine_power + mine_power*father/100 + mine_power*grandpa/100;
		flate_power
	}



}


