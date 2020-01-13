
//! ## Genesis config
use support::{debug,decl_storage, decl_module,decl_event, StorageValue, StorageMap,Parameter,
			  dispatch::Result, weights::{SimpleDispatchInfo},Blake2_256, ensure,dispatch::Vec,traits::Currency, StorageDoubleMap};
use support::traits::{Get, ReservableCurrency};
use system::{ensure_signed};
use rstd::convert::{TryInto,TryFrom};
use sp_runtime::traits::{Hash,SimpleArithmetic, Bounded, One, Member,CheckedAdd, Zero};
use sp_runtime::{Permill};
use codec::{Encode, Decode};
use crate::mine_linked::{PersonMineWorkForce,PersonMine,MineParm,PersonMineRecord,BLOCK_NUMS, MineTag};
//use node_primitives::BlockNumber;
use crate::register::{self,MinersCount,AllMiners,Trait as RegisterTrait};
use crate::mine_power::{PowerInfo, MinerPowerInfo, TokenPowerInfo, PowerInfoStore, MinerPowerInfoStore, TokenPowerInfoStore};
use node_primitives::{Count, USD, PermilllChangeIntoF64, Duration};
use rstd::{result};

use rstd::prelude::*;



// 继承 register 模块,方便调用register里面的 store
pub trait Trait: balances::Trait + RegisterTrait{
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency3: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
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

	type SuperiorShareRatio: Get<PermilllChangeIntoF64>;
	type OnsuperiorShareRatio: Get<PermilllChangeIntoF64>;

	type SubHalfDuration: Get<Duration>;  // 减半周期

}

type StdResult<T> = core::result::Result<T, &'static str>;
type BalanceOf<T> = <<T as Trait>::Currency3 as Currency<<T as system::Trait>::AccountId>>::Balance;

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
		MinerDays get(fn minertxdays): map T::AccountId => Vec<T::BlockNumber>;

		// 个人所有天数的交易hash（未清除）
		MinerAllDaysTx get(fn mineralldaystx): double_map hasher(twox_64_concat) T::AccountId, blake2_256(T::BlockNumber) => Vec<Vec<u8>>;

		Founders get(fn founders) config(): Vec<T::AccountId>;
    }

}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        const ArchiveDuration: T::BlockNumber = T::ArchiveDuration::get();


        pub fn create_mine(origin,mine_tag: MineTag, tx: Vec<u8>, address: Vec<u8>,to_address:Vec<u8>,symbol:Vec<u8>,amount:u64,protocol:Vec<u8>,decimal:u64,usdt_nums:u64,blockchain:Vec<u8>,memo:Vec<u8>) -> Result { // 创建挖矿
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
        	ensure!(address != to_address,"you cannot transfer  to yourself");
        	ensure!(usdt_nums<u64::max_value(),"usdt_nums is overflow");
        	ensure!(usdt_nums>5,"usdt_nums is too small");

        	// 挖矿类型不能相同 并且挖矿次数不能大于2
        	ensure!(!(<OwnerMineRecord<T>>::exists(tx.clone())
        	&& (<OwnerMineRecord<T>>::get(tx.clone()).unwrap().mine_tag == mine_tag.clone()  ||  (<OwnerMineRecord<T>>::get(tx.clone()).unwrap().mine_count >= 2u16))), "tx already exists");

			// 该币的全网挖矿算力大于一定的比例  则不再挖矿
			ensure!(!Self::is_token_power_more_than_portion(symbol.clone()), "this token is more than max portion today.");

        	let block_num = <system::Module<T>>::block_number();
        	let now_tokenpowerinfo = <TokenPowerInfoStoreItem<T>>::get_curr_token_power(block_num);
        	let PCbtc = now_tokenpowerinfo.btc_total_count;
        	let PAbtc =  now_tokenpowerinfo.btc_total_amount;

        	// btc挖矿次数或是金额大于一定数目 则停止挖矿
        	ensure!(!(T::BTCLimitCount::get() <= PCbtc || T::BTCLimitAmount::get() <= PAbtc), "btc count or amount allreadey enough.");

			// 删除过期的交易tx（为了减轻存储负担）
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

				if <MinerAllDaysTx<T>>::exists(sender.clone(), now_day.clone()){
					let mut all_tx = <MinerAllDaysTx<T>>::get(sender.clone(), now_day.clone());
					all_tx.push(tx.clone());
					<MinerAllDaysTx<T>>::insert(sender.clone(), now_day.clone(), all_tx.clone());
				}
				else{
					<MinerAllDaysTx<T>>::insert(sender.clone(), now_day.clone(), vec![tx.clone()]);
				}

				// 获取本人的所有有记录的天数
				let all_days = <MinerDays<T>>::get(sender.clone());
				if all_days.is_empty(){
					let days = vec![now_day];
					<MinerDays<T>>::insert(sender.clone(), days);
				}
				else{
					if !all_days.contains(&now_day){

						let mut days = all_days.clone();
						days.push(now_day);
						// 先删除再增加
						<MinerDays<T>>::remove(sender.clone());
						<MinerDays<T>>::insert(sender.clone(), days);
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
			// 打印创始团队成员的AccountId
			debug::RuntimeLogger::init();
			debug::print!("mine module fouders:---------------------------------{:?}", Self::founders());

            if (block_number % T::ArchiveDuration::get()).is_zero() {
                Self::archive(block_number);
            }
        }
    }
}

impl<T: Trait> Module<T> {
	fn mining(mine_parm:MineParm,sender: T::AccountId)->Result{
		ensure!(<AllMiners<T>>::exists(sender.clone()), "account not register");
		// 获取日期
		let block_num = <system::Module<T>>::block_number();
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);
		let now_day = block_num/day_block_nums;


		let now_time = <timestamp::Module<T>>::get();   // 记录到秒
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
		//--------------------------------------------------------------------------------------------
		// ***以下跟算力相关

		// 把金额与次数均放大100倍  获得更大的算力精度
		let enlarge_usdt_nums: u64 = mine_parm.usdt_nums.clone();
		// 计算金额算力
		let mut amount_workforce = Self::calculate_count_or_amount_workforce(&sender, block_num, mine_parm.symbol.clone(), enlarge_usdt_nums, true)?;
		// 获取膨胀金额算力（真实算力）
		amount_workforce = Self::inflate_power(sender.clone(), amount_workforce);
		// 获取次数算力
		let mut count_workforce = Self::calculate_count_or_amount_workforce(&sender, block_num, mine_parm.symbol.clone(), 100 as u64, false)?;
		// 获取膨胀后的次数算力（真实算力）
		count_workforce = Self::inflate_power(sender.clone(), count_workforce);

		// 获取昨天的总金额
		let mut prev_total_amount = <PowerInfoStoreItem<T>>::get_prev_power(block_num.clone()).total_amount;
		// 获取昨天的总次数
		let mut prev_total_count = <PowerInfoStoreItem<T>>::get_prev_power(block_num.clone()).total_count;

		// 计算总算力占比
		let workforce_ratio = Self::caculate_workforce_ratio(amount_workforce.clone(), count_workforce.clone(), prev_total_amount.clone(), prev_total_count.clone());

		// 获取该日期挖矿奖励的总token
		let mut today_reward = <BalanceOf<T>>::from(0u32);
		match Self::per_day_mine_reward_token() {
			Some(a) => today_reward = a,
			None => return Err("tdday reward err")
		}
		// 把算力占比变成balance类型  这里是初始化 下面才是真的赋值
		let mut workforce_ratio_change_into_balance = <BalanceOf<T>>::from(0u32);

		// 精度  这里采用10位 因为u64不能用 所以用两个u32代替
		let mut decimals1 = <BalanceOf<T>>::from(1_0000_0000u32);
		let mut decimal2 = <BalanceOf<T>>::from(10u32);
		let decimal = decimals1*decimal2;

		// 把算力占比变成balance类型
		match <BalanceOf<T>>::try_from(workforce_ratio as usize).ok(){
			Some(b) => workforce_ratio_change_into_balance = b,
			None => return Err("fenzi err")
		}

		// 计算这一次的总挖矿奖励
		let thistime_reward = today_reward * workforce_ratio_change_into_balance/decimal;
		// 矿工奖励
		let miner_reward = thistime_reward*<BalanceOf<T>>::from(8)/<BalanceOf<T>>::from(10);
		// 每一个创始团队成员的奖励
		let per_founder_reward = thistime_reward/<BalanceOf<T>>::from(10);

		T::Currency3::deposit_into_existing(&sender, miner_reward)?;

		let fouders = Self::founders();
		for i in fouders.iter(){
			T::Currency3::deposit_into_existing(&i, per_founder_reward);
		}
		// 奖励上级与上上级
		Self::reward_parent_or_super(sender.clone(), thistime_reward);

		// 全网算力存储
		<PowerInfoStoreItem<T>>::add_power(workforce_ratio.clone(), 1u64, count_workforce.clone(), mine_parm.usdt_nums.clone(),
		amount_workforce.clone(), block_num.clone());
		// 全网token信息存储
		<TokenPowerInfoStoreItem<T>>::add_token_power(mine_parm.symbol.clone(), workforce_ratio, 1u64, count_workforce, mine_parm.usdt_nums.clone(),
		amount_workforce, block_num);
		// 矿工个人算力存储
		let curr_point = Self::miner_power_info_point().1;
		<MinerPowerInfoStoreItem<T>>::add_miner_power(&sender, curr_point.clone(), mine_parm.symbol.clone(), workforce_ratio,
		1u64, count_workforce, mine_parm.usdt_nums.clone(), amount_workforce, block_num);

		//--------------------------------------------------------------------------------------------
		let person_mine_record = PersonMineRecord::new(&mine_parm, sender.clone(),now_time, block_num, balance2)?;

		if <OwnerMineRecord<T>>::exists(&mine_parm.tx){
			<OwnerMineRecord<T>>::remove(&mine_parm.tx)
		}

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
	fn calculate_count_or_amount_workforce(miner_id: &T::AccountId, block_number: T::BlockNumber, coin_name: Vec<u8>, usdt_nums: u64, is_amount_power: bool)
		-> result::Result<u64, &'static str> {
        let (prev_point, curr_point) = Self::miner_power_info_point();
        let miner_power_info = <MinerPowerInfoStoreItem<T>>::get_miner_power_info(curr_point, miner_id, block_number.clone());
        let prev_miner_power_info = <MinerPowerInfoStoreItem<T>>::get_miner_power_info(prev_point, miner_id, block_number.clone());
        let prev_power_info = <PowerInfoStoreItem<T>>::get_prev_power(block_number.clone());
        let prev_token_power_info = <TokenPowerInfoStoreItem<T>>::get_prev_token_power(block_number.clone());
        let miner_numbers = <MinersCount>::get();

        let alpha = 0.3;
        let beta = 1.0 - alpha;

		let mut work_power = 0 as u64;

        if  coin_name == "btc".as_bytes().to_vec() {
			let lc_btc = 100u64;
			ensure!(miner_power_info.btc_count <= lc_btc, "BTC mining count runs out today");

			// 计算矿机P一次BTC转账的频次算力PCW btc = α * 1 / TC / PPC btc ( PC btc < LC btc )
			// 矿机P计算BTC频次算力钝化系数，PPC btc = ( (PC btc + 1 ) / AvC btc ) % 10
			if is_amount_power{
				//                let coin_amount = (coin_price * coin_number) as u64;
				// PPA btc	矿机P计算BTC金额算力钝化系数	PPA btc = ( (Price(BTC) * m btc +PAbtc ) / AvA btc ) % 10
				// todo 以下是计算金额算力
				let mut pa = prev_miner_power_info.total_amount;
				if pa < 10 {
					pa = 1000;
				}
				let ava_btc = prev_token_power_info.btc_total_amount.checked_div(miner_numbers).ok_or("Calc AvA btc causes overflow")?;  // 平均每个区块多少钱
				let ppa_btc_divisor = usdt_nums + prev_token_power_info.btc_total_amount;
				let divisor = ppa_btc_divisor.checked_div(ava_btc).ok_or("Calc PPC btc parameter causes overflow")?;
				let ppa_btc = divisor % 10;
				let paw_btc = (beta * (usdt_nums as f64) * ppa_btc as f64 / pa as f64) as u64;
				work_power = paw_btc;

			}
			else{
				let avc_btc = prev_token_power_info.btc_total_count.checked_div(miner_numbers).ok_or("Calc AvC btc causes overflow")?;  // todo 平均一个区块产生多少btc
				let ppc_btc_divisor = prev_miner_power_info.btc_count.checked_add(1).ok_or("Calc PPC btc divisor causes overflow")?;
				let divisor = ppc_btc_divisor.checked_div(avc_btc).ok_or("Calc PPC btc parameter causes overflow")?;
				let ppc_btc = divisor % 10;

				let mut tc = prev_power_info.total_count;
				if tc == 0 {
					tc = 100;
				}
				let pcw_btc = (alpha * ppc_btc as f64 / tc as f64) as u64;
				work_power = pcw_btc;
			}

				return Ok(work_power); // todo 算力应该是个浮点数


        }
		else {
			return Err("Unsupported token")
		}

		Ok(10 as u64)
	}

	fn caculate_workforce_ratio(amount_workforce: u64, count_workforce: u64, mut pre_amount_workfore: u64, mut pre_count_workforce: u64) -> u64{
		let a_sr = 0.5;  // 金额算力占比
		let c_sr= 1.0-a_sr;  // 次数算力占比

		if pre_count_workforce == 0_u64{
			pre_count_workforce = 1000u64; // 初始化1000笔
		}
		if pre_amount_workfore == 0_u64{
			pre_amount_workfore = 10_0000u64;  // 初始化1000万金额
		}

		let decimal = 100_0000_0000u64 as f64;
		let workforce_ratio = (amount_workforce as f64 * decimal  / pre_amount_workfore as f64  * a_sr + count_workforce as f64 * decimal / pre_count_workforce as f64 * c_sr) as u64;
		workforce_ratio
	}

	fn remove_expire_record(who: T::AccountId, is_remove_all: bool) {
		/// 删除过期记录
		let block_num = <system::Module<T>>::block_number(); // 获取区块的高度
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);  // wjy 一天出多少块
		let now = block_num / day_block_nums;

		if <MinerDays<T>>::exists(&who) {
			let all_days = <MinerDays<T>>::get(&who);
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
		let mut all_days = <MinerDays<T>>::get(&who);
		let all_tx = <MinerAllDaysTx<T>>::get(who.clone(), day.clone());
		//如果当天交易存在 那么就删除掉
		if !all_tx.is_empty() {
			for tx in all_tx.iter() {
				<OwnerMineRecord<T>>::remove(tx.clone());  // tx不能直接用remove方法来删除？？？？？？？？
			}
		}

		<MinerAllDaysTx<T>>::remove(who.clone(), day.clone());

		if let Some(pos) = all_days.iter().position(|a| a == &day) {
			all_days.swap_remove(pos);

			// 更新本人的未删除记录
			<MinerDays<T>>::insert(who.clone(), all_days.clone())
		}
	}

	fn is_token_power_more_than_portion(symbol: Vec<u8>) -> bool{// 参数要小写
		/// 判断该token在全网算力是否超额
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


	fn per_day_mine_reward_token() -> Option<BalanceOf<T>>{
		/// 计算每一天的挖矿奖励
		let block_num = <system::Module<T>>::block_number(); // 获取区块的高度
		let day_block_nums = <BlockNumberOf<T>>::from(BLOCK_NUMS);  // wjy 一天出多少块
		let now:u64  = (block_num / day_block_nums).try_into().ok()?.try_into().ok()?;

		let e = (now/(36525*T::SubHalfDuration::get()/100)) as u32;  //一年365.25天来进行计算
		if e > 32{
			Some(<BalanceOf<T>>::from(0))  // 128年之后的挖矿奖励基本为0 所以这时候可以终止了 继续没必要
		}
		else{
			let num = 2_u32.pow(e);  // 意味着e最大值是32  运行32*4 = 128年
			let per_day_tokens = T::FirstYearPerDayMineRewardToken::get()/<BalanceOf<T>>::from(num); // 2的n次方怎么形容
			Some(per_day_tokens)
		}

	}

	// 把这个usdt金额数值再放大到100倍  这样计算数值的时候才能最大限度的准确
	fn inflate_power(who: T::AccountId, mine_power: u64) -> u64{  // todo 膨胀算力在计算算力之后  把膨胀算力加入到累计算力里面
		/// 计算膨胀算力
		let mut grandpa = 0.0;
		let mut father = 0.0;
		if let Some(father_address) = <AllMiners<T>>::get(who.clone()).father_address{
			father = T::OnsuperiorShareRatio::get();
		};
		if let Some(grandpa_address) = <AllMiners<T>>::get(who.clone()).grandpa_address{
			grandpa = T::SuperiorShareRatio::get();
		};
		let inflate_power = (mine_power as f64 + mine_power as f64 *father as f64/100 as f64 + mine_power as f64 *grandpa as f64/100 as f64) as u64;
		inflate_power
	}

	fn reward_parent_or_super(who: T::AccountId, thistime_reward_token: BalanceOf<T>){
		if let Some(father_address) = <AllMiners<T>>::get(who.clone()).father_address{
			let fa_reward = thistime_reward_token * <BalanceOf<T>>::from(2u32)/<BalanceOf<T>>::from(10u32);
			T::Currency3::deposit_creating(&father_address, fa_reward);
		};
		if let Some(grandpa_address) = <AllMiners<T>>::get(who.clone()).grandpa_address{
			let gr_reward = thistime_reward_token * <BalanceOf<T>>::from(1u32)/<BalanceOf<T>>::from(10u32);
			T::Currency3::deposit_creating(&grandpa_address, gr_reward);
		};
	}
}


