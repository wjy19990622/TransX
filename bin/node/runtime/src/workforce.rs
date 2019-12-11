use rstd::prelude::*;
use support::{debug, ensure, decl_module, decl_storage, decl_event, dispatch::Result,
              weights::{SimpleDispatchInfo}, StorageValue, StorageMap, StorageDoubleMap, Blake2_256};
use support::traits::{Get};
use runtime_primitives::traits::{As, Hash, Zero};
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

/// `WorkforceInfo`存储全网的算力信息，每日都会归档一次，并新建一个供当时使用。
/// `RunningDays`会存储区块链运行天数，可以根据`RunningDays`获取当前`WorkforceInfo`。
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct WorkforceInfo<BlockNumber> {
    total_workforce: u64,                       // 24小时总算力
    total_count: u64,                           // 24小时总交易次数
    total_amount: f64,                          // 24小时总金额（以USDT计）
    miner_numbers: u64,                         // 全网总矿机数量
    block_number: BlockNumber,                  // 区块高度
}

/// `MinerInfo`保存矿机的挖矿信息。由于每个矿机都要保存一个这样的结构，并且计算矿机的挖矿算力需要
/// 使用前一天的算力，所以需要保持两个该结构，`MinerInfoMapOne`和`MinerInfoMapTwo`，并通过额外
/// 变量`MinerInfoPrevPoint`来区分前一天算力的存储。当`MinerInfoPrevPoint`为1,则`MinerInfoMapOne`
/// 表示是前一天的算力。
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct MinerInfo<BlockNumber> {
    hardware_id: Vec<u8>,                       // 矿机的硬件ID
    total_workforce: u64,                       // 24小时累计算力
    total_count: u64,                           // 24小时累计交易次数
    total_amount: f64,                          // 24小时累计交易金额

    btc_workforce: u64,                         // 24小时BTC累计算力
    btc_count: u64,                             // 24小时BTC累计次数
    btc_amount: f64,                            // 24小时BTC累计金额

    eth_workforce: u64,                         // 24小时ETH累计算力
    eth_count: u64,                             // 24小时ETH累计次数
    eth_amount: f64,                            // 24小时ETH累计金额

    eos_workforce: u64,                         // 24小时EOS累计算力
    eos_count: u64,                             // 24小时EOS累计次数
    eos_amount: f64,                            // 24小时EOS累计金额

    usdt_workforce: u64,                         // 24小时USDT累计算力
    usdt_count: u64,                             // 24小时USDT累计次数
    usdt_amount: f64,                            // 24小时USDT累计金额

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

/// `TokenTradeInfo`记录每日的交易信息，和`WorkforceInfo`一样，通过`RunningDays`来获取。
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TokenTradeInfo<BlockNumber> {
    btc_total_workforce: u64,         // 24小时BTC累计算力
    btc_total_count: u64,             // 24小时BTC累计交易次数
    btc_total_amount: f64,            // 24小时BTC累计交易金额

    eth_total_workforce: u64,         // 24小时ETH累计算力
    eth_total_count: u64,             // 24小时ETH累计交易次数
    eth_total_amount: f64,            // 24小时ETH累计交易金额

    eos_total_workforce: u64,         // 24小时EOS累计算力
    eos_total_count: u64,             // 24小时EOS累计交易次数
    eos_total_amount: f64,            // 24小时EOS累计交易金额

    usdt_total_workforce: u64,         // 24小时USDT累计算力
    usdt_total_count: u64,             // 24小时USDT累计交易次数
    usdt_total_amount: f64,            // 24小时USDT累计交易金额

    block_number: BlockNumber,        // 区块高度
}


decl_storage! {
    trait Store for Module<T: Trait> as WorkforceStorage {
        // `RunningDays`：区块链运行天数
        RunningDays: u32;

        // `WorkforceInfoList`存储每日的全网算力信息，key为`RunningDays`，value为`WorkforceInfo`。
        // 当key为`RunningDays`时，表示获取当日的全网算力，key=[1..`RunningDays`-1]获取历史的算力信息。
        // 当每日结束时，`RunningDays`+1，开始存储计算下一个日期的算力信息。
        WorkforceInfoList get(workforce_info_by_numbers): map u32 => WorkforceInfo<T::BlockNumber>;

        // `TokenTradeInfoList`存储每日的Token交易信息，与`WorkforceInfoList`类似。
        TokenTradeInfoList get(token_trade_info_by_numbers): map u32 => TokenTradeInfo<T::BlockNumber>;

        // `MinerInfoMapOne`存储所有矿机的挖矿信息，key为矿机的硬件ID，通过`MinerInfoPrevPoint`来区分是否
        // 存储的是当前挖矿信息还是前一天挖矿信息。
        MinerInfoMapOne get(miner_info_by_id_one): map Vec<u8> => MinerInfo;

        // `MinerInfoMapTwo`存储所有矿机的挖矿信息，key为矿机的硬件ID，通过`MinerInfoPrevPoint`来区分是否
        // 存储的是当前挖矿信息还是前一天挖矿信息。
        MinerInfoMapTwo get(miner_info_by_id_two): map Vec<u8> => MinerInfo;

        // `MinerInfoPrevPoint`用来区分存储前一天挖矿信息的Map，`MinerInfoMapOne`还是`MinerInfoMapTwo`。
        // = 0，表示第一天挖矿，还不存在前一日挖矿信息
        // = 1，表示前一天挖矿信息保存在`MinerInfoMapOne`中
        // = 2，表示前一天挖矿信息保存在`MinerInfoMapTwo`中
        MinerInfoPrevPoint: u32;
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

        fn on_finalize(n: T::BlockNumber) {
            if (n % T::ArchiveDuration::get()).is_zero() {
                ensure!(<WorkforceInfoList>::exists(), "workforce info list store does not exist");
                Self::archive_workforce_info();
            }
        }
    }
}

impl<T: Trait> Module<T> {
    fn new_workforce_info(miner_numbers: u64) -> WorkforceInfo<T::BlockNumber> {
        let workforce_info = WorkforceInfo {
            total_workforce: 0,
            total_count: 0,
            total_amount: 0.0,
            miner_numbers,
            block_number: <system::Module<T>>::block_number()
        };
        workforce_info
    }

    // 获取全网当前的算力信息，如果不存在，则插入一个默认算力信息并返回。
    fn get_workforce_info_or_insert() -> WorkforceInfo<T::BlockNumber> {
        let mut workforce_info: WorkforceInfo<T::BlockNumber>;

        if !<RunningDays>::exists() {
            // Todo: 获取已注册的矿机数量，这里假设为0
            let miner_numbers: u64 = 0;
            workforce_info = Self::new_workforce_info(miner_numbers);

            let running_days = 1u64;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info.clone());
            <RunningDays>::put(running_days);
        } else {
            let running_days = <RunningDays>::get();
            workforce_info = Self::workforce_info_by_numbers(running_days);
        }

        workforce_info
    }

    // 获取前一天的算力信息，如果不存在，则返回一个默认的算力信息。
    fn get_prev_workforce_info_or_default() -> WorkforceInfo<T::BlockNumber> {
        let mut workforce_info: WorkforceInfo<T::BlockNumber>;
        if !<RunningDays>::exists() {
            // Todo: 获取已注册的矿机数量，这里假设为0
            let miner_numbers: u64 = 0;
            workforce_info = Self::new_workforce_info(miner_numbers);
        } else {
            let running_days = <RunningDays>::get();
            if running_days < 2 {
                // Todo: 获取已注册的矿机数量，这里假设为0
                let miner_numbers: u64 = 0;
                workforce_info = Self::new_workforce_info(miner_numbers);
            } else {
                workforce_info = Self::workforce_info_by_numbers(running_days - 1);
            }
        }

        workforce_info
    }

    // 设置全网的矿机数量
    fn set_miner_numbers(miner_numbers: u64) {
        if !<RunningDays>::exists() {
            let workforce_info = Self::new_workforce_info(miner_numbers);

            let running_days = 1u64;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info.clone());
            <RunningDays>::put(running_days);
        } else {
            let running_days = <RunningDays>::get();
            let mut workforce_info = Self::workforce_info_by_numbers(running_days);
            workforce_info.miner_numbers = miner_numbers;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info);
        }
    }

    // 增加全网算力
    fn add_workforce(workforce: u64, count: u64, amount: f64) {
        if !<RunningDays>::exists() {
            let mut workforce_info = Self::new_workforce_info(0);
            workforce_info.total_workforce += workforce;
            workforce_info.total_count += count;
            workforce_info.total_amount += amount;
            let running_days = 1u64;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info.clone());
            <RunningDays>::put(running_days);
        } else {
            let running_days = <RunningDays>::get();
            let mut workforce_info = Self::workforce_info_by_numbers(running_days);
            workforce_info.total_workforce += workforce;
            workforce_info.total_count += count;
            workforce_info.total_amount += amount;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info);
        }
    }

    // 归档算力
    fn archive_workforce_info() {
        if !<RunningDays>::exists() {
            let workforce_info = Self::new_workforce_info(0);
            let running_days = 1u64;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info.clone());
            <RunningDays>::put(running_days);
        } else {
            let running_days = <RunningDays>::get();
            let mut workforce_info = Self::workforce_info_by_numbers(running_days);
            workforce_info.block_number = <system::Module<T>>::block_number();
            let miner_numbers = workforce_info.miner_numbers;
            <WorkforceInfoList<T>>::insert(running_days, workforce_info);

            let new_workforce_info = Self::new_workforce_info(miner_numbers);
            <WorkforceInfoList<T>>::insert(running_days+1, new_workforce_info);
            <RunningDays>::put(running_days+1);
        }
    }

    fn new_token_trade_info() -> TokenTradeInfo<T::BlockNumber> {
        let token_trade_info = TokenTradeInfo {
            btc_total_workforce: 0u64,
            btc_total_count: 0u64,
            btc_total_amount: 0.0,

            eth_total_workforce: 0u64,
            eth_total_count: 0u64,
            eth_total_amount: 0.0,

            eos_total_workforce: 0u64,
            eos_total_count: 0u64,
            eos_total_amount: 0.0,

            usdt_total_workforce: 0u64,
            usdt_total_count: 0u64,
            usdt_total_amount: 0.0,

            block_number: <system::Module<T>>::block_number()
        };

        token_trade_info
    }

    // 获取全网当前Token交易信息，如果不存在，则插入一个默认交易信息并返回。
    fn get_token_trade_info_or_insert() -> TokenTradeInfo<T::BlockNumber> {
        let mut token_trade_info: TokenTradeInfo<T::BlockNumber>;

        if !<RunningDays>::exists() {
            let running_days = 1u64;
            token_trade_info = Self::new_token_trade_info();
            <TokenTradeInfoList<T>>::insert(running_days, token_trade_info.clone());
            <RunningDays>::put(running_days);
        } else {
            let running_days = <RunningDays>::get();
            token_trade_info = Self::token_trade_info_by_numbers(running_days);
        }

        token_trade_info
    }

    // 获取前一天的Token交易信息，如果不存在，则返回一个默认的交易信息。
    fn get_prev_token_trade_info_or_default() -> TokenTradeInfo<T::BlockNumber> {
        let mut token_trade_info: TokenTradeInfo<T::BlockNumber>;
        if !<RunningDays>::exists() {
            token_trade_info = Self::new_token_trade_info();
        } else {
            let running_days = <RunningDays>::get();
            if running_days < 2 {
                token_trade_info = Self::new_token_trade_info();
            } else {
                token_trade_info = Self::token_trade_info_by_numbers(running_days - 1);
            }
        }

        token_trade_info
    }

    fn new_miner_info(hardware_id: Vec<u8>) -> MinerInfo<T::BlockNumber> {
        let miner_info = MinerInfo {
            hardware_id,
            total_workforce:0,
            total_count: 0,
            total_amount: 0.0,
            btc_workforce: 0,
            btc_count: 0,
            btc_amount: 0.0,
            eth_workforce: 0,
            eth_count: 0,
            eth_amount: 0.0,
            eos_workforce: 0,
            eos_count: 0,
            eos_amount: 0.0,
            usdt_workforce: 0,
            usdt_count: 0,
            usdt_amount: 0.0,
            block_number: <system::Module<T>>::block_number()
        };
        miner_info
    }

    // 获取指定矿机当前的算力信息
    fn get_miner_info_by_id(hardware_id: Vec<u8>) -> MinerInfo<T::BlockNumber> {
        // 判断`MinerInfoPrevPoint`是否存在，如果存在的话，需要根据其值判断
        if !<MinerInfoPrevPoint>::exists() {
            Self::new_miner_info(hardware_id)
        } else {
            let prev = <MinerInfoPrevPoint>::get();
            if prev == 1 {
                Self::miner_info_by_id_two(hardware_id)
            } else if prev == 2{
                Self::miner_info_by_id_one(hardware_id)
            } else {
                Self::new_miner_info(hardware_id)
            }
        }
    }

    // 获取指定矿机前一日的算力信息
    fn get_prev_miner_info_by_id(hardware_id: Vec<u8>) -> MinerInfo<T::BlockNumber> {
        // 判断`MinerInfoPrevPoint`是否存在，如果存在的话，需要根据其值判断
        if !<MinerInfoPrevPoint>::exists() {
            Self::new_miner_info(hardware_id)
        } else {
            let prev = <MinerInfoPrevPoint>::get();
            if prev == 1 {
                Self::miner_info_by_id_one(hardware_id)
            } else if prev == 2{
                Self::miner_info_by_id_two(hardware_id)
            } else {
                Self::new_miner_info(hardware_id)
            }
        }
    }

    // 计算一次挖矿的算力
    fn calc_workforce(hardware_id: Vec<u8>, coin_name: &str, coin_number: f64, coin_price: f64) -> Result {
        let mut workforce_info = Self::get_workforce_info_or_insert();
        let mut miner_info = Self::get_miner_info_by_id(hardware_id.clone());

        let prev_workforce_info = Self::get_prev_workforce_info_or_default();
        let prev_miner_info = Self::get_prev_miner_info_by_id(hardware_id.clone());
        let prev_token_trade_info = Self::get_prev_token_trade_info_or_default();

        if coin_name == "BTC" {
            let lc_btc: u64 = 100;
            ensure!(miner_info.btc_count <= lc_btc, "BTC mining count runs out today");

            // 计算矿机P一次BTC转账的频次算力PCW btc = α * 1 / TC / PPC btc ( PC btc < LC btc )
            // 矿机P计算BTC频次算力钝化系数，PPC btc = ( (PC btc + 1 ) / AvC btc ) % 10
            let avc_btc = prev_token_trade_info.btc_total_count.checked_div(workforce_info.miner_numbers)
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
            let ava_btc = prev_token_trade_info.btc_total_amount.checked_div(workforce_info.miner_numbers)
                .ok_or("Calc AvA btc causes overflow")?;
            let ppa_btc_divisor = coin_price * coin_number + prev_token_trade_info.btc_total_amount;
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

