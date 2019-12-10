use rstd::prelude::*;
use support::{debug, ensure, decl_module, decl_storage, decl_event, dispatch::Result, weights::{SimpleDispatchInfo}, StorageValue, StorageMap, StorageDoubleMap, Blake2_256};
use support::traits::{Get};
use system::ensure_signed;
use timestamp;
use codec::{Encode, Decode};
use sp_runtime::offchain::http::Error;
use std::panic::resume_unwind;

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    // 算力归档时间，到达这个时间，则将`WorkforceInfo`信息写入到链上并不再修改。
    type ArchiveDuration: Get<Self::BlockNumber>;
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct WorkforceInfo<BlockNumber> {
    total_workforce: u64,                       // 24小时总算力
    total_count: u64,                           // 24小时总交易次数
    total_amount: u64,                          // 24小时总金额（以USDT计）
    miner_numbers: u64,                         // 全网总矿机数量
    block_number: BlockNumber,                  // 区块高度
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct MinerInfo<BlockNumber> {
    hardware_id: Vec<u8>,                       // 矿机的硬件ID
    total_workforce: u64,                       // 24小时累计算力
    total_count: u64,                           // 24小时累计交易次数
    total_amount: u64,                          // 24小时累计交易金额

    btc_workforce: u64,                         // 24小时BTC累计算力
    btc_count: u64,                             // 24小时BTC累计次数
    btc_amount: u64,                            // 24小时BTC累计金额

    block_number: BlockNumber,                  // 区块高度
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct GovernanceParameter {
    alpha: f64,                 // 频次算力在总算力中占比系数
    beta: f64,                  // 金额算力在总算力中占比系数
    lc_btc: u64,                // 24小时单台矿机允许的BTC交易总次数limit
    la_btc: u64,                // 24小时单台矿机允许的BTC交易总金额limit
    lc_eos: u64,
    la_eos: u64,
    lc_usdt: u64,
    la_usdt: u64,
    mla_btc: u64,               // 单次转账最大金额
    ssr: f64,                   // 推广上级分润比例，0 < SSR < 1
    osr: f64,                   // 推广上上级分润比例，0 < OSR < 1
    mr: u64,                    // 每日最低奖励数量
    msr: f64,                   // 矿工分享到手续费比例
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct BtcTradeInfo<BlockNumber> {
    total_workforce: u64,         // 24小时累计算力
    total_count: u64,             // 24小时累计交易次数
    total_amount: u64,            // 24小时累计交易金额
    block_number: BlockNumber,    // 区块高度
}

// 矿机算力相关参数计算方式：
// ρ btc：BTC算力占总算力的最高份额，0 < ρ < 1,这是一个可以治理修正的参数
// PPC btc：矿机P计算BTC频次算力钝化系数，等于((MinerInfo.btc_count + 1) / BtcTradeInfo.average_count) % 10
// PCW btc：矿机P一次BTC转账的频次算力，等于 GovernanceParameter.alpha * 1 / WorkforceInfo.total_count / PPC btc ( PC btc < LC btc )
// PPA btc：矿机P计算BTC金额算力钝化系数	PPA btc = ( (Price(BTC) * m btc +PAbtc ) / AvA btc ) % 10
// PAW btc：矿机P一次BTC转账的金额算力	PAW btc = β m price(BTC) / TW / PPA btc (PA btc < LA btc )


decl_storage! {
    trait Store for Module<T: Trait> as WorkforceStorage {
        // 运行天数
        RunningDays: u32;
        WorkforceInfoList get(workforce_info_by_numbers): map u32 => WorkforceInfo<T::BlockNumber>;
        BtcTradeInfoList get(btc_trade_info_by_numbers): map u32 => BtcTradeInfo<T::BlockNumber>;
        // 将矿机当前挖矿数据保存到数据库中
        MinerInfoDict get(miner_info_by_id): map Vec<u8> => MinerInfo;
        // 一日结束时，将当前的挖矿数据保存到数据库中
        MinerInfoPrevDict get(miner_info_prev_by_id): map Vec<u8> => MinerInfo;
    }
}

decl_event! (
    pub enum Event<T>
    where
        <T as system::Trait>::Hash,
        <T as balances::Trait>::Balance
    {
        // fields of the new inner thing
        NewInnerThing(u32, Hash, Balance),
        // fields of the super_number and the inner_thing fields
        NewSuperThingByExistingInner(u32, u32, Hash, Balance),
        // ""
        NewSuperThingByNewInner(u32, u32, Hash, Balance),
        // for testing purposes of `balances::Event`
        NullEvent(u32),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        const ArchiveDuration: T::BlockNumber = T::ArchiveDuration::get();
    }
}

impl<T: Trait> Module<T> {
    // 获取全网当前的算力信息
    fn get_workforce_info() -> WorkforceInfo<T::BlockNumber> {
        let mut workforce_info: WorkforceInfo<T::BlockNumber>;
        let mut running_days: u32 = 1;

        if !<RunningDays>::exists() {
            <RunningDays>::put(running_days);
            workforce_info = WorkforceInfo {
                total_workforce: 0,
                total_count: 0,
                total_amount: 0,
                miner_numbers: 0,
                block_number: <system::Module<T>>::block_number()
            };
            <WorkforceInfoList<T>>::insert(running_days, workforce_info.clone());
        } else {
            running_days = <RunningDays>::get();
            workforce_info = Self::workforce_info_by_numbers(running_days);
        }

        workforce_info
    }

    // 获取前一天的算力信息
    fn get_prev_workforce_info() -> WorkforceInfo<T::BlockNumber> {
        let mut workforce_info: WorkforceInfo<T::BlockNumber>;
        if !<RunningDays>::exists() {
            workforce_info = WorkforceInfo {
                total_workforce: 0,
                total_count: 0,
                total_amount: 0,
                miner_numbers: 0,
                block_number: <system::Module<T>>::block_number()
            };
        } else {
            let running_days = <RunningDays>::get();
            if running_days < 2 {
                workforce_info = WorkforceInfo {
                    total_workforce: 0,
                    total_count: 0,
                    total_amount: 0,
                    miner_numbers: 0,
                    block_number: <system::Module<T>>::block_number()
                };
            } else {
                workforce_info = Self::workforce_info_by_numbers(running_days - 1);
            }
        }

        workforce_info
    }

    // 获取全网当前BTC交易信息
    fn get_btc_trade_info() -> BtcTradeInfo<T::BlockNumber> {
        let mut btc_trade_info: BtcTradeInfo<T::BlockNumber>;
        let mut running_days: u32 = 1;

        if !<RunningDays>::exists() {
            <RunningDays>::put(running_days);
            btc_trade_info = BtcTradeInfo {
                total_workforce: 0,
                total_count: 0,
                total_amount: 0,
                block_number: <system::Module<T>>::block_number()
            };
            <BtcTradeInfoList<T>>::insert(running_days, btc_trade_info.clone());
        } else {
            running_days = <RunningDays>::get();
            btc_trade_info = Self::btc_trade_info_by_numbers(running_days);
        }

        btc_trade_info
    }

    // 获取前一天的BTC交易信息
    fn get_prev_btc_trade_info() -> BtcTradeInfo<T::BlockNumber> {
        let mut btc_trade_info: BtcTradeInfo<T::BlockNumber>;
        if !<RunningDays>::exists() {
            btc_trade_info = BtcTradeInfo {
                total_workforce: 0,
                total_count: 0,
                total_amount: 0,
                block_number: <system::Module<T>>::block_number()
            };
        } else {
            let running_days = <RunningDays>::get();
            if running_days < 2 {
                btc_trade_info = BtcTradeInfo {
                    total_workforce: 0,
                    total_count: 0,
                    total_amount: 0,
                    block_number: <system::Module<T>>::block_number()
                };
            } else {
                btc_trade_info = Self::btc_trade_info_by_numbers(running_days - 1);
            }
        }

        btc_trade_info
    }

    // 获取指定矿机当前的算力信息
    fn get_miner_info(hardware_id: Vec<u8>) -> MinerInfo<T::BlockNumber> {
        let mut miner_info:MinerInfo<T::BlockNumber>;

        if !<MinerInfoDict<T>>::exists(hardware_id.clone()) {
            miner_info = MinerInfo {
                hardware_id: hardware_id.clone(),
                total_workforce:0,
                total_count: 0,
                total_amount: 0,
                btc_workforce: 0,
                btc_count: 0,
                btc_amount: 0,
                block_number: <system::Module<T>>::block_number()
            };
            <MinerInfoDict<T>>::insert(hardware_id.clone(), miner_info.clone());
        } else {
            miner_info = Self::miner_info_by_id(hardware_id.clone())
        }

        miner_info
    }

    // 获取指定矿机昨日的算力信息
    fn get_prev_miner_info(hardware_id: Vec<u8>) -> MinerInfo<T::BlockNumber> {
        let mut miner_info:MinerInfo<T::BlockNumber>;

        if !<MinerInfoPrevDict<T>>::exists(hardware_id.clone()) {
            miner_info = MinerInfo {
                hardware_id: hardware_id.clone(),
                total_workforce:0,
                total_count: 0,
                total_amount: 0,
                btc_workforce: 0,
                btc_count: 0,
                btc_amount: 0,
                block_number: <system::Module<T>>::block_number()
            };
        } else {
            miner_info = Self::miner_info_prev_by_id(hardware_id.clone())
        }

        miner_info
    }


    // 计算一次挖矿的算力
    fn calc_workforce(hardware_id: Vec<u8>, coin_name: &str, coin_number: f64, coin_price: f64) -> Result {
        let mut workforce_info = Self::get_workforce_info();
        // let mut btc_trade_info = Self::get_btc_trade_info();
        let mut miner_info = Self::get_miner_info(hardware_id.clone());

        let prev_workforce_info = Self::get_prev_workforce_info();
        let prev_miner_info = Self::get_prev_miner_info(hardware_id.clone());
        let prev_btc_trade_info = Self::get_prev_btc_trade_info();

        if coin_name == "BTC" {
            let lc_btc: u64 = 100;
            ensure!(miner_info.btc_count < lc_btc, "BTC mining count runs out today");

            // 计算矿机P一次BTC转账的频次算力PCW btc = α * 1 / TC / PPC btc ( PC btc < LC btc )
            // 矿机P计算BTC频次算力钝化系数，PPC btc = ( (PC btc + 1 ) / AvC btc ) % 10
            let avc_btc = prev_btc_trade_info.total_count.checked_div(workforce_info.miner_numbers)
                .ok_or("Calc AvC btc causes overflow")?;
            let ppc_btc_divisor = prev_miner_info.btc_count.checked_add(1)
                .ok_or("Calc PPC btc divisor causes overflow")?;
            let divisor = ppc_btc_divisor.checked_div(avc_btc).ok_or("Calc PPC btc parameter causes overflow")?;
            let ppc_btc = divisor % 10;

            let alpha:f64 = 0.3;
            let mut tc = prev_workforce_info.total_count;
            if tc == 0 {
                tc = 100;
            }

            let pcw_btc = alpha * ppc_btc / tc;

            let beta = 1 - alpha;
            let mut pa = prev_miner_info.total_amount;
            if pa < 10 {
                pa = 1000;
            }

            // PPA btc	矿机P计算BTC金额算力钝化系数	PPA btc = ( (Price(BTC) * m btc +PAbtc ) / AvA btc ) % 10
            let ava_btc = prev_btc_trade_info.total_amount.checked_div(workforce_info.miner_numbers)
                .ok_or("Calc AvA btc causes overflow")?;
            let ppa_btc_divisor = coin_price * coin_number + prev_btc_trade_info.total_amount;
            let divisor = ppa_btc_divisor.checked_div(avc_btc).ok_or("Calc PPC btc parameter causes overflow")?;
            let ppa_btc = divisor % 10;
            let paw_btc = beta * coin_number * coin_price * ppa_btc / pa;

            let sr = 0.5;

            let pw_btc = (pcw_btc + paw_btc) * sr;

            Ok(pw_btc)
        }

        Ok(())
    }
}

