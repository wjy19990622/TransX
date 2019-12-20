
use support::{decl_module, decl_storage, decl_event, dispatch::Result, ensure, debug, StorageMap, StorageValue};
use system::ensure_signed;
use rstd::prelude::*;
use collective;
use codec::{Encode, Decode};
use sp_runtime::traits::{Hash};
use support::traits::{Get,
	Currency, ReservableCurrency
};
use sp_runtime::{Permill, ModuleId};
use sp_runtime::traits::{
	Zero, EnsureOrigin, StaticLookup, AccountIdConversion, Saturating,
};
//use register;
use crate::register::{AllMiners, BlackList, Trait as RegisterTrait};
use crate::register;

const MODULE_ID: ModuleId = ModuleId(*b"py/trsry");

//use test::parse_opts;  // 如果要用is_zero就要导入这个

// 这个是举报模块

// 这个是用来指定金额是u128类型的数据的
type BalanceOf<T> = <<T as Trait>::Currency0 as Currency<<T as system::Trait>::AccountId>>::Balance;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VoteInfo<Bo, A, Ba, H> {
	start_vote_block: Bo, // 开始投票的区块高度
	symbol: Vec<u8>,   // 币种
	tx: Vec<u8>,   // 交易tx
	tx_hash: H,   //  交易哈希
	reporter: A,  // 举报人
	report_reason: Vec<u8>,  // 举报理由
	illegal_man: A,  // 作弊者
	transaction_amount: Ba,  // 交易币额
	usdt_amount: Ba,  // usdt数额
	decimals: u32,  // 精度
	approve_mans: Vec<A>,  // 投赞成票的人
	reject_mans: Vec<A>,  // 投反对票的人
}

// 是否被惩罚
#[derive(Encode, Decode, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum IsPunished{
	YES,
	NO,
}

#[derive(Encode, Decode, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum TreasuryNeed{
	SUB,
	ADD,
}

// 投票结果
#[derive(Encode, Decode, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum VoteResult{
	PASS,
	NoPASS,
}

pub trait Trait: balances::Trait + RegisterTrait{

	// 议会成员
	type ConcilOrigin: EnsureOrigin<Self::Origin, Success=Self::AccountId>;


	type Currency0: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

	// 极端情况下多少票数算是胜出
	type Thredshould: Get<u32>;
	// 议案过期时间
	type ProposalExpire: Get<Self::BlockNumber>;

	// 每隔多久集体奖励一次
	type VoteRewardPeriod: Get<Self::BlockNumber>;

	// 举报抵押金额
	type ReportReserve: Get<BalanceOf<Self>>;

	type ReportReward: Get<BalanceOf<Self>>;

	// 对作弊者的惩罚金额
	type IllegalPunishment: Get<BalanceOf<Self>>;

	// 奖励每个投票的议员多少
	type CouncilReward: Get<BalanceOf<Self>>;

	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as ReportModule {

		// 所有还未奖励的投票的集合
		pub Votes get(fn votes): map T::Hash => VoteInfo<T::BlockNumber, T::AccountId, T::Balance, T::Hash>;

		// 所有人建立一个与自己有关的所有合法交易的tx_hash数组， 这些驻足组成一个集合
		pub Man_Txhashs get(fn mantxhashs): map T::AccountId => Vec<T::Hash>;

		// 被拉进黑名单的所有用户(现在已经移到register中）
//		pub BlackList get(fn blacklist): map T::AccountId => T::Hash;

		// 已经通过但是还没有给予奖励的投票结果
		pub RewardList get(fn rewardlist): Vec<VoteInfo<T::BlockNumber, T::AccountId, T::Balance, T::Hash>>;
	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		pub fn deposit_event() = default;

		//------------------------------------------------------------------------------------------
		// 举报
		pub fn report(origin, symbol: Vec<u8>, tx: Vec<u8>, repoter: T::AccountId, reason: Vec<u8>,
		illegalman: T::AccountId, tx_amount: T::Balance, usdt_amount: T::Balance, decimals: u32) -> Result{
			let who = ensure_signed(origin)?;

			// 如果作弊者和举报人有至少一个不在注册名单里， 则不给举报。
			// TODO 这个is_register_member方法需要进一步完善
			ensure!( !(Self::is_register_member(who.clone()) &&
			Self::is_register_member(illegalman.clone())), "someone don't exists in register_list.");

			//  计算交易tx哈希值
			let tx_hash = tx.using_encoded(<T as system::Trait>::Hashing::hash);
			// 根据tx_hash判断这笔交易是否已经存在  已经存在的话不再添加进来
			ensure!(!<Votes<T>>::exists(&tx_hash), "the tx exists in the report queue, you can't put it into again.");
			// 没有足够抵押资金，不给举报
			T::Currency0::reserve(&who, T::ReportReserve::get()).map_err(|_| "balance too low, you can't report")?;
			let start_vote_block = <system::Module<T>>::block_number();
			let mut vote_info = VoteInfo{
				start_vote_block: start_vote_block.clone(),
				symbol: symbol.clone(),
				tx: tx.clone(),
				tx_hash: tx_hash.clone(),
				reporter: who.clone(),
				report_reason: reason.clone(),
				illegal_man: illegalman.clone(),
				transaction_amount: tx_amount.clone(),
				usdt_amount: usdt_amount.clone(),
				decimals: decimals.clone(),
				approve_mans: vec![],
				reject_mans:vec![],
			};
			// 判断投票者是否是议员
			//TODO 判断是否是议员这个方法需要完善
			if Self::is_concil_member(who.clone()) {
				vote_info.approve_mans.push(who.clone());
			}
			// 添加该投票的信息
			<Votes<T>>::insert(tx_hash.clone(), vote_info);
			// 添加人与相关交易映射
			Self::add_mantxhashs(who.clone(), tx_hash.clone());
			Self::add_mantxhashs(illegalman.clone(), tx_hash.clone());
			Self::deposit_event(RawEvent::ReportEvent(start_vote_block, illegalman));
			Ok(())
		}


		//-----------------------------------------------------------------------------------------
		// 投票
		pub fn vote(origin, tx_hash: T::Hash, yes_no: bool) -> Result{
			// 如果自己不是议会成员则不给操作
			let who = T::ConcilOrigin::ensure_origin(origin)?;
			// 判断这个tx_hash是否存在于投票队列中，不存在则退出
			ensure!(!(<Votes<T>>::exists(&tx_hash)), "the tx_hash not in vote_queue.");
			let illegalman = <Votes<T>>::get(&tx_hash).illegal_man;

			// 如果这个议会成员是作弊者（被举报方），则禁止其投票。
			ensure!(!(illegalman.clone() == who.clone()), "you are being reported, can't vote.");

			// 如果举报者和作弊者有至少一个不在注册列表中， 则退出。
			ensure!( !(Self::is_register_member(who.clone()) &&
			Self::is_register_member(illegalman.clone())), "someone don't exists in register_list.");

			let now = <system::Module<T>>::block_number();
			// 过期删除相关信息  并且退出
			if now - <Votes<T>>::get(&tx_hash).start_vote_block > T::ProposalExpire::get(){
					<Votes<T>>::remove(&tx_hash);
					// 删除相关的man thhashs信息
					// TODO 添加和删除方法均已经实现， 注意查看代码是否正确
					Self::remove_mantxhashs(who.clone(), tx_hash.clone());
					Self::remove_mantxhashs(illegalman.clone(), tx_hash.clone());
					ensure!(1==2, "the vote is expire.")
			}
			let mut voting = <Votes<T>>::get(&tx_hash);
			let position_yes = voting.approve_mans.iter().position(|a| a == &who);
			let position_no = voting.reject_mans.iter().position(|a| a == &who);
			// 如果投赞成票
			if yes_no{
				if position_yes.is_none(){
					voting.approve_mans.push(who.clone());
				}
				else{
					return Err("duplicate vote ignored")
				}
				if let Some(pos) = position_no{
					voting.reject_mans.swap_remove(pos);
				}
			}
			// 如果投的是反对票
			else{
				if position_no.is_none(){
					voting.reject_mans.push(who.clone());
				}
				else{
					return Err("duplicate vote ignored")
				}
				if let Some(pos) = position_yes{
					voting.approve_mans.swap_remove(pos);
				}
			}
			<Votes<T>>::insert(tx_hash.clone(), voting.clone());
			// 判断议案是否结束
			let vote_result = Self::vote_result(voting.clone());
			// 如果议案投票已经结束
			if vote_result.0 == VoteResult::PASS{
				// 把该投票结果存储到奖励名单
				<RewardList<T>>::mutate(|a| a.push(voting.clone()));
				Self::remove_mantxhashs(who.clone(),tx_hash.clone());
				Self::remove_mantxhashs(illegalman.clone(),tx_hash.clone());
				// 如果作弊是真  把名字加入黑名单  并且从注册列表中删除
				if vote_result.1 == IsPunished::YES{
					<BlackList<T>>::insert(illegalman.clone(), tx_hash.clone());
					Self::kill_register(illegalman.clone());
				}
			}
			Self::deposit_event(RawEvent::VoteEvent(illegalman.clone()));
			Ok(())
		}


		//------------------------------------------------------------------------------------------
		// 每次出块结束都要去计算一下是否是奖励时间 如果是则奖励
		fn on_finalize(n: T::BlockNumber){
			if (n % T::VoteRewardPeriod::get()).is_zero() {  // 默认一天奖励一次
				Self::reward();  // 奖励的方法
		}
		}
	}
}


decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId,
		<T as system::Trait>::BlockNumber,
		<T as system::Trait>::Hash,
		<T as balances::Trait>::Balance,
	 {

		// 开始的区块 被举报者姓名
		ReportEvent(BlockNumber, AccountId),

		// 正在投谁的票
		VoteEvent(AccountId),

		// 谁的议案通过了
		VoteFinishEvent(AccountId),

		// 返回一个数组
		TreasuryEvent(bool, Balance),

		// 谁的票奖励结束了 tx哈希是多少
		RewardEvent(AccountId, Hash),

		SomethingStored(u32, AccountId),
	}
);

impl<T: Trait> Module<T> {


	//----------------------------------------------------------------------------------------------
	pub fn reward() -> Result {
		// 计算国库还有多少钱
		let mut useable_balance = Self::treasury_useable_balance();
		// 获取国库id
		let treasury_id = Self::get_treasury_id();

		// 这一步按照两个步骤来走
		for i in 0..2 {
			<RewardList<T>>::mutate(|v| {
			v.retain(|voteinfo| {
				let is_punish = Self::vote_result(voteinfo.clone()).1;
				let treasury_result = Self::treasury_imbalance(is_punish.clone(), voteinfo.clone());
				let sub_or_add = treasury_result.0;
				let imbalances = treasury_result.1;

				// 如果国库需要添加金额
				if sub_or_add == TreasuryNeed::ADD{
					// 给国库增加金额
					useable_balance += imbalances;
					T::Currency0::make_free_balance_be(&treasury_id, useable_balance);
					// 彻底删掉投票信息
					<Votes<T>>::remove(voteinfo.clone().tx_hash);
					Self::everyone_balance_oprate(is_punish.clone(), voteinfo.clone());
					false
				}
					// 如果国库需要减掉金额
				else{
					if useable_balance >= imbalances{
						// 给国库减掉金额
						useable_balance -= imbalances;
						T::Currency0::make_free_balance_be(&treasury_id, useable_balance);
						// 彻底删掉投票信息
						<Votes<T>>::remove(voteinfo.clone().tx_hash);
						Self::everyone_balance_oprate(is_punish.clone(), voteinfo.clone());
						false
					}
						// 金额不够 暂时不执行
					else{
						true
					}
				}
			});
		});
		}
		Ok(())
	}

	//---------------------------------------------------------------------------------------------
	// 这个方法用来判断是否是议会成员
	// TODO 是否是议员
	pub fn is_concil_member(who: T::AccountId) -> bool {
		false
	}

	// 是否在矿机的注册名单里面
	pub fn is_register_member(who: T::AccountId) -> bool {
//		true
		if <AllMiners<T>>::exists(&who){
			true
		}
		else {
			false
		}
	}

	// 把该名单从注册列表删除
	pub fn kill_register(who: T::AccountId) {
		<register::Module<T>>::kill_man(who.clone());

	}

	//--------------------------------------------------------------------------------------------
	//获取国库id
	pub fn get_treasury_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// 计算国库可用的钱
	pub fn treasury_useable_balance() -> BalanceOf<T> {
		T::Currency0::free_balance(&Self::get_treasury_id())
			// Must never be less than 0 but better be safe.
			.saturating_sub(T::Currency0::minimum_balance())
	}

	//--------------------------------------------------------------------------------------------
	// 添加man hashs映射添加相关信息
	pub fn add_mantxhashs(who: T::AccountId, tx_hash: T::Hash) {
		let mut vec_txhash = vec![];
		if <Man_Txhashs<T>>::exists(&who) {
			vec_txhash = <Man_Txhashs<T>>::get(&who);
			vec_txhash.push(tx_hash);
		} else {
			vec_txhash.push(tx_hash)
		}
		<Man_Txhashs<T>>::insert(&who, &vec_txhash);
	}

	// 删除man txhashs相关信息
	pub fn remove_mantxhashs(who: T::AccountId, tx_hash: T::Hash) {
		let mut vec_txhash = vec![];
		vec_txhash = <Man_Txhashs<T>>::get(&who);
		if let Some(pos) = vec_txhash.iter().position(|a| <Votes<T>>::exists(&tx_hash)) {
			vec_txhash.swap_remove(pos);
		};
		if vec_txhash.len() == 0 {
			<Man_Txhashs<T>>::remove(&who)
		} else {
			<Man_Txhashs<T>>::insert(&who, &vec_txhash);
		}
	}


	//----------------------------------------------------------------------------------------------
	// 这个方法用于验证投票是否结束（是否有一方胜出）
	pub fn vote_result(vote_info: VoteInfo<T::BlockNumber, T::AccountId, T::Balance, T::Hash>)
		-> (VoteResult, IsPunished) {

		let approve_len = vote_info.approve_mans.len() as u32;
		let reject_len = vote_info.reject_mans.len() as u32;
		// 胜出两票或是有一方先过半 那么就结束
		if approve_len - reject_len >= 2 || reject_len - approve_len >= 2 || approve_len
			>= T::Thredshould::get() || reject_len >= T::Thredshould::get(){
			if approve_len > reject_len {
				(VoteResult::PASS, IsPunished::YES)
			} else {
				(VoteResult::PASS, IsPunished::NO)
			}
		} else {
			(VoteResult::NoPASS, IsPunished::NO)
		}
	}


	//----------------------------------------------------------------------------------------------
	// 计算国库盈余或是亏损多少  第一个参数返回true是盈余  返回false是亏损
	pub fn treasury_imbalance(is_punish: IsPunished, vote:
	VoteInfo<T::BlockNumber, T::AccountId, T::Balance, T::Hash>) -> (TreasuryNeed, BalanceOf<T>) {

		let mut postive: BalanceOf<T> = 0.into();
		let mut negative: BalanceOf<T> = 0.into();
		// 真的作弊
		if is_punish == IsPunished::YES {
			// 惩罚作弊者的金额
			if Self::is_register_member(vote.illegal_man.clone()) {
				postive = T::IllegalPunishment::get();
			}
			// 奖励举报者的总金额
			if Self::is_register_member(vote.reporter.clone()) {
				negative = T::ReportReward::get();
			}
		}
		// 虚假举报
		else {
			// 惩罚举报者的金额
			if Self::is_register_member(vote.reporter.clone()) {
				postive += T::ReportReserve::get().clone();
			}
		}
		// 议员总奖励金额
		let mut all_mans =
			vote.reject_mans.iter().chain(vote.approve_mans.iter());

		for i in 0..all_mans.clone().count() {
			if let Some(peaple) = all_mans.next() {
				if Self::is_register_member(peaple.clone()) {
					negative += T::CouncilReward::get().clone();
				}
			};
		}
		// 国库需要减掉一些
		if postive > negative {
			(TreasuryNeed::SUB, postive - negative)
			// 国库需要册增加
		} else {
			(TreasuryNeed::ADD, negative - postive)
		}
	}

	//----------------------------------------------------------------------------------------------
	// 这个方法用来操作除了国库之外的跟此次投票有关的人员的金额
	pub fn everyone_balance_oprate(is_punish: IsPunished,
								   vote: VoteInfo<T::BlockNumber, T::AccountId, T::Balance, T::Hash>){

		let illagalman = vote.illegal_man;
		let reporter = vote.reporter;
		// 如果作弊是真的
		if is_punish == IsPunished::YES {
			// 惩罚作弊者
			if Self::is_register_member(illagalman.clone()) {
				T::Currency0::slash_reserved(&illagalman, T::IllegalPunishment::get());
			}
			// 解除抵押并奖励举报人
			if Self::is_register_member(reporter.clone()) {
				T::Currency0::unreserve(&reporter, T::ReportReserve::get());
				T::Currency0::deposit_creating(&reporter, T::ReportReward::get());
			}
		}

		// 虚假举报
		else {
			// 扣除举报者金额
			if Self::is_register_member(reporter.clone()) {
				T::Currency0::slash_reserved(&reporter, T::ReportReserve::get());
			}
		}
		// 奖励议员
		let mut all_mans = vote.reject_mans.iter()
			.chain(vote.approve_mans.iter());

		for i in 0..all_mans.clone().count() {
			if let Some(peaple) = all_mans.next() {
				if Self::is_register_member(peaple.clone()) {
					T::Currency0::deposit_creating(&peaple, T::CouncilReward::get());
				}
			};
		}
	}


}


