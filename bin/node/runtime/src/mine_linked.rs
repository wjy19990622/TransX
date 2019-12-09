use support::{decl_storage, decl_module,decl_event, Parameter,StorageValue, StorageMap,
              dispatch::Result, ensure,dispatch::Vec};
use system::{ensure_signed};
use sp_runtime::traits::{ Hash,Member,SimpleArithmetic,Bounded,MaybeDisplay};
use codec::{Encode, Decode};
use rstd::{result};
//use core::{f32, f64};
const DAY_SECONDS :u32 = 86400;
const BLOCK_TIME:u32 = 3;   //  3s出一个块
const BLOCK_NUMS: u32 = DAY_SECONDS/BLOCK_TIME;

#[cfg_attr(feature = "std", derive(Debug, PartialEq, Eq))]
#[derive(Encode, Decode)]
pub struct MineParm {
    pub action:Vec<u8>,
    pub address:Vec<u8>,
    pub symbol:Vec<u8>,
    pub amount:u64,  // eth 等需要是整数
    pub precision:u64, // 精度
    pub protocol:Vec<u8>,
    pub decimal:u16,
    pub blockchain:Vec<u8>,
    pub memo:Vec<u8>
}

// 个人算力 汇总表
#[cfg_attr(feature = "std", derive(Debug, PartialEq, Eq))]
#[derive(Encode, Decode)]
pub struct PersonMineWorkForce<BlockNumber>{
    mine_cnt: u64, // 当天的挖矿次数
    usdt_nums: u64,  // 完成的金额
    work_force: u64,  // 当天的算力
    settle_blocknumber:BlockNumber, // 上一次结算时的区块高度,用于区分是否是第二天了
}

// 为了来存储 PersonMineWorkForce
// 仅仅是为了让编译器通过, Storage:PersonMineWorkForce Key:T::AccountId
pub struct PersonMine<Storage, Key,BlockNumber>(rstd::marker::PhantomData<(Storage, Key,BlockNumber)>);

impl<Storage, Key,BlockNumber> PersonMine<Storage, Key,BlockNumber> where
    Key: Parameter, // Key 表示个人的hash值
    BlockNumber:Parameter + Member + MaybeDisplay + SimpleArithmetic + Default + Bounded + Copy,
    Storage: StorageMap<Key,PersonMineWorkForce<BlockNumber>, Query = Option<PersonMineWorkForce<BlockNumber>>>,
{
    fn write(key: &Key, personmine_work_force: PersonMineWorkForce<BlockNumber>) {
        Storage::insert(key,personmine_work_force);
    }

    fn read(key: &Key, block_num:BlockNumber) ->PersonMineWorkForce<BlockNumber>{
        // let SettleBlocknumber =<system::Module<T>>::block_number();
        Storage::get(key).unwrap_or_else(|| PersonMineWorkForce {
            mine_cnt: 0,
            usdt_nums: 0,
            work_force:0,
            settle_blocknumber: block_num
        })
    }

    fn calculate_workforce()->u64{
        // 伪代码
        10
    }

    pub fn add(key: Key,usdt_nums:u64,block_num:BlockNumber)-> Result{
        // 获取上次的算力
        let mut personmine_work_force = Self::read(&key, block_num);
        let block_nums = BlockNumber::from(BLOCK_NUMS);
        let last_day = personmine_work_force.settle_blocknumber.checked_div(&block_nums)
                        .ok_or("add function: div causes error of last_day")?;
        let now_day = block_num.checked_div(&block_nums)
                        .ok_or("user add function: div causes error of now_day")?;

        let now_workforce = Self::calculate_workforce();
        personmine_work_force.settle_blocknumber = block_num;
        if last_day==now_day{
            // 相当于是同一天
            personmine_work_force.mine_cnt =  personmine_work_force.mine_cnt.checked_add(1)
                                .ok_or("add function: add causes overflow of mine_cnt")?;
            personmine_work_force.usdt_nums =  personmine_work_force.usdt_nums.checked_add(usdt_nums)
                                .ok_or("add function: add causes overflow of usdt_nums")?;
            personmine_work_force.work_force = personmine_work_force.work_force.checked_add(now_workforce)
                                .ok_or("add function: add causes overflow of work_force")?;

        }else{
            //第二天
            personmine_work_force.mine_cnt =  1;
            personmine_work_force.usdt_nums =  usdt_nums;
            personmine_work_force.work_force = now_workforce;
        }
        Self::write(&key,personmine_work_force);
        Ok(())
    }
}

// 个人算力 单次挖矿表
#[cfg_attr(feature = "std", derive(Debug, PartialEq, Eq))]
#[derive(Encode, Decode)]
pub struct PersonMineRecord<Moment,BlockNumber,Balance>{
    timestamp:Moment,         // 挖矿时间
    blocknum:BlockNumber,
    miner_address:Vec<u8>,   //矿工地址
    from_address:Vec<u8>,    // 不为空，钱包发起支付挖矿地址
    to_address:Vec<u8>,      // 不为空，接收客户端挖矿地址
    symbol:Vec<u8>,          // 币种
    amount:Balance,              // 支付的金额
    blockchain:Vec<u8>,       // 哪条链
    tx:Vec<u8>,              // 交易的hash
    usdt_amount:u64,         // usdt 总价格
    pcount_workforce:u64,     // 这次交易频次算力
    pamount_workforce:u64,     //这次交易金额算力
    reward:Balance,                 // 奖励的token
    superior_reward:Balance,        // 上级奖励的token
    on_reward:Balance           // 上上级奖励的token
}

impl <Moment,BlockNumber,Balance>PersonMineRecord<Moment,BlockNumber,Balance>
    where Balance:Copy,
{
    pub fn new(mine_parm:MineParm,moment:Moment,block_number:BlockNumber,balances:Balance)
        ->  result::Result<PersonMineRecord<Moment,BlockNumber,Balance>, &'static str> {
        if mine_parm.amount < u64::max_value(){ // 逻辑或
            return Err("overflow f64");
        }
        let s = [1,2].to_vec();

        let res =  PersonMineRecord{
            timestamp:moment,
            blocknum: block_number,
            miner_address: mine_parm.address,  // 是否该字段就是挖矿地址?
            from_address: s.clone(),
            to_address: s.clone(),
            symbol: s.clone(),
            amount: balances,
            blockchain: s.clone(),
            tx: s,
            usdt_amount: 1,
            pcount_workforce: 1,
            pamount_workforce: 1,
            reward: balances,
            superior_reward: balances,
            on_reward: balances
        };
        Ok(res)
    }

    pub fn record(){

    }

}
