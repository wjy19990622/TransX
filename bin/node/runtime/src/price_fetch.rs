/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references

/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

// We have to import a few things
use rstd::{prelude::*, convert::TryInto};
use primitives::{crypto::AccountId32 as AccountId};
use primitives::{crypto::KeyTypeId,offchain::Timestamp};

use support::{Parameter,decl_module, decl_storage, decl_event, dispatch, debug, traits::Get,StorageLinkedMap};
use system::{ ensure_signed,ensure_none, offchain,
              offchain::SubmitSignedTransaction,
              offchain::SubmitUnsignedTransaction,
               };
use simple_json::{ self, json::JsonValue };

use runtime_io::{ self, misc::print_utf8 as print_bytes };
use codec::{ Encode,Decode };
use num_traits::float::FloatCore;
use sp_runtime::{
    AnySignature,MultiSignature,MultiSigner,
    offchain::http, transaction_validity::{
    TransactionValidity, TransactionLongevity, ValidTransaction, InvalidTransaction},
    traits::{CheckedSub,CheckedAdd,Printable,Member,Zero,IdentifyAccount},
    RuntimeAppPublic};
use app_crypto::{sr25519};

type BlockNumberOf<T> = <T as system::Trait>::BlockNumber;  // u32
type StdResult<T> = core::result::Result<T, &'static str>;

/// Our local KeyType.
///
/// For security reasons the offchain worker doesn't have direct access to the keys
/// but only to app-specific subkeys, which are defined and grouped by their `KeyTypeId`.
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ofpf");

// REVIEW-CHECK: is it necessary to wrap-around storage vector at `MAX_VEC_LEN`?
pub const MAX_VEC_LEN: usize = 1000;

pub trait AccountIdPublicConver{
    type AccountId;
    fn into_account32(self)->Self::AccountId; // 转化为accountId
}


pub type Signature = AnySignature;
pub mod crypto {
    pub use super::{KEY_TYPE,AccountIdPublicConver,Signature};
    pub mod app_sr25519 {
        pub use super::{KEY_TYPE,AccountIdPublicConver};
//        use app_crypto::{app_crypto, sr25519};
//        use node_primitives::{AccountId};
        use sp_runtime::{MultiSignature,MultiSigner};
        use sp_runtime::traits::{IdentifyAccount};  // AccountIdConversion,
        use primitives::{crypto::AccountId32 as AccountId};
        use sp_runtime::app_crypto::{app_crypto, sr25519};
        app_crypto!(sr25519, KEY_TYPE);
//        use primitives::sr25519;
//        app_crypto::app_crypto!(sr25519, KEY_TYPE);

        impl From<Signature> for super::Signature {
            fn from(a: Signature) -> Self {
                sr25519::Signature::from(a).into()
            }
        }

        impl From<AccountId> for Public {
            fn from(inner: AccountId) -> Self {
                let s = <[u8; 32]>::from(inner);
                let sr_public = sr25519::Public(s);
                Self::from(sr_public)
            }
        }
        impl From<Public> for AccountId {
            fn from(outer: Public) -> Self {
                let s: sr25519::Public = outer.into();
                MultiSigner::from(s).into_account()
            }
        }


        impl AccountIdPublicConver for Public{
            type AccountId = AccountId;
            fn into_account32(self) -> AccountId{
                let s: sr25519::Public = self.into();
                MultiSigner::from(s).into_account()
            }
        }


        impl IdentifyAccount for Public {
            type AccountId = AccountId;
            fn into_account(self) -> AccountId {
                let s: sr25519::Public = self.into();
                <[u8; 32]>::from(s).into()
            }
        }

    }

    pub type AuthorityId = app_sr25519::Public;
    #[cfg(feature = "std")]
    pub type AuthorityPair = app_sr25519::Pair;
}


// todo 仅仅测试用,后改为链表,方便链上添加与修改
pub const FETCHED_CRYPTOS: [(&[u8], &[u8], &[u8]); 4] = [
    (b"btc", b"coincap",
     b"https://api.coincap.io/v2/assets/bitcoin"),
    (b"btc", b"cryptocompare",
     b"https://min-api.cryptocompare.com/data/price?fsym=BTC&tsyms=USD"),
    (b"eth", b"coincap",
     b"https://api.coincap.io/v2/assets/ethereum"),
//    (b"eth", b"cryptocompare",
//     b"https://min-api.cryptocompare.com/data/price?fsym=ETH&tsyms=USD"),
//    (b"dai", b"coincap",
//     b"https://api.coincap.io/v2/assets/dai"),
    (b"dai", b"cryptocompare",
     b"https://min-api.cryptocompare.com/data/price?fsym=DAI&tsyms=USD"),
];

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub struct PriceInfo<AccountId> {  // 存储币种价格
    dollars: u64,  // 0.0001美元为单位
    account: AccountId, //哪个账号查询到的
    url:Vec<u8>,    // 对应的url,短写
}

#[derive(Debug, Encode, Decode, Clone, PartialEq)]
pub struct PriceFailed<AccountId> {  // 存储币种价格
    // dollars: u64,   // 0.0001美元为单位
    account: AccountId, //哪个账号查询到的
    sym:Vec<u8>,    // 对应的币名字
    errinfo:Vec<u8>,
}

type PriceFailedOf<T> = PriceFailed<<T as system::Trait>::AccountId>;

/// The module's configuration trait.
pub trait Trait: timestamp::Trait + system::Trait + authority_discovery::Trait{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Call: From<Call<Self>>;

    type SubmitSignedTransaction: offchain::SubmitSignedTransaction<Self, <Self as Trait>::Call>;
    type SubmitUnsignedTransaction: offchain::SubmitUnsignedTransaction<Self, <Self as Trait>::Call>;

    /// The local AuthorityId
    type AuthorityId: RuntimeAppPublic + Clone + Parameter+ Into<sr25519::Public> + From<sr25519::Public>+ AccountIdPublicConver<AccountId=Self::AccountId>; // From<Self::AccountId> + Into<Self::AccountId> +

    type TwoHour: Get<Self::BlockNumber>;
    type Hour: Get<Self::BlockNumber>;

    // Wait period between automated fetches. Set to 0 disable this feature.
    //   Then you need to manucally kickoff pricefetch
    // type BlockFetchDur: Get<Self::BlockNumber>;


}

decl_event!(
  pub enum Event<T> where
    Moment = <T as timestamp::Trait>::Moment {

    FetchedPrice(Vec<u8>, Vec<u8>, Moment, u64),
    AggregatedPrice(Vec<u8>, Moment, u64),
  }
);

// This module's storage items.
decl_storage! {
  trait Store for Module<T: Trait> as PriceFetch {
    UpdateAggPP get(update_agg_pp): linked_map Vec<u8> => u32 = 0;

    // storage about source price points
    // mapping of ind -> (timestamp, price)
    //   price has been inflated by 10,000, and in USD.
    //   When used, it should be divided by 10,000.

    //
    // key:币名字(名字统一小写)+4小时的区块个数, value:时间与币价格 .列表存放 .每个周期删除一次
    PricePoints get(price_pts): double_map Vec<u8>, blake2_256(T::BlockNumber) => Vec<(T::Moment, PriceInfo<T::AccountId>)>;
    // 待删除的  key:币名字  value:4小时的区块周期数
    DeletePricePoints get(del_price_pts): linked_map Vec<u8> => Vec<T::BlockNumber>;


    // 记录 哪个节点ccountId,时间,哪个 url 没有查询到数据.保存一天数据.key: 币名字, value:时间 , url .每天删除一次
    pub SrcPriceFailed get(src_price_failed): linked_map Vec<u8> => Vec<(PriceFailedOf<T>)>;
    pub SrcPriceFailedCnt get(pricefailed_cnt): linked_map Vec<u8> => u64;


    // todo 以下多余
    SrcPricePoints get(src_price_pts): Vec<(T::Moment, u64)>;
    // mapping of token sym -> pp_ind
    // Using linked map for easy traversal from offchain worker or UI
    TokenSrcPPMap: linked_map Vec<u8> => Vec<u32>;

    // mapping of remote_src -> pp_ind
    RemoteSrcPPMap: linked_map Vec<u8> => Vec<u32>;

    // storage about aggregated price points (calculated with our logic)
    AggPricePoints get(agg_price_pts): Vec<(T::Moment, u64)>;
    TokenAggPPMap: linked_map Vec<u8> => Vec<u32>;
  }
}

impl<T: Trait> Module<T> {
    fn offchain(block_num:T::BlockNumber,key: &T::AccountId) -> dispatch::Result{
        for (sym, remote_src, remote_url) in FETCHED_CRYPTOS.iter() {
            let current_time = <timestamp::Module<T>>::get();
            if let Err(e) = Self::fetch_price(block_num,key,*sym, *remote_src, *remote_url,current_time) {
                debug::error!("------Error fetching------: {:?}, {:?}: {:?},{:?}",
                    core::str::from_utf8(sym).unwrap(),
                    core::str::from_utf8(remote_src).unwrap(),
                    e,
                    current_time
                    );
                // 处理错误信息
                let price_failed = PriceFailed {
                    account: key.clone(),
                    sym: (*sym).to_vec(),
                    errinfo: e.as_bytes().to_vec(),
                };
                // 实现错误信息上报记录
                let call = Call::record_fail_fetchprice(block_num,sym.to_vec(), price_failed);
                T::SubmitUnsignedTransaction::submit_unsigned(call)
                    .map_err(|_| {
                        debug::info!("===record_fail_fetchprice: submit_unsigned_call error===");
                        "===record_fail_fetchprice: submit_unsigned_call error==="
                    })?;
                debug::info!("+++++++record_fail_fetchprice suc++++++++++++++");
            }
        }
        Ok(())
    }

    /// Find a local `AccountId` we can sign with, that is allowed to offchainwork
    fn authority_id() -> Option<T::AccountId> { // 返回值 T::AccountId 改为 AccountId32
        //通过本地化的密钥类型查找此应用程序可访问的所有本地密钥。
        // 然后遍历当前存储在chain上的所有ValidatorList，并根据本地键列表检查它们，直到找到一个匹配，否则返回None。
        let authorities = <authority_discovery::Module<T>>::authorities().iter().map(
            |i| { // (*i).clone().into()
                (*i).clone().into()
//                MultiSigner::from(s).into_account()
//                <[u8; 32]>::from(s).into()
            }
        ).collect::<Vec<sr25519::Public>>();
        debug::info!("本地key: {:?}",authorities);
        for i in T::AuthorityId::all().iter(){
            let authority:T::AuthorityId = (*i).clone();
            let  authority_sr25519: sr25519::Public = authority.clone().into();
            if authorities.contains(&authority_sr25519) {
                let s:T::AccountId= authority.into_account32();
                debug::info!("找到了账号: {:?}",s);
                return Some(s);
//                return Some(T::AccountId::from((*i).clone()));
            }
        }
        return None;
    }

    fn fetch_json<'a>(remote_url: &'a [u8]) -> StdResult<JsonValue> {
        let remote_url_str = core::str::from_utf8(remote_url)
            .map_err(|_| "Error in converting remote_url to string")?;

        let now = <timestamp::Module<T>>::get();
        let deadline:u64 = now.try_into().
            map_err(|_|"An error occurred when moment was converted to usize")?  // usize类型
            .try_into().map_err(|_|"An error occurred when usize was converted to u64")?;
        let deadline = Timestamp::from_unix_millis(deadline+20000); // 等待最多10s

        let mut new_reuest = http::Request::get(remote_url_str);
        new_reuest.deadline = Some(deadline);
        let pending = new_reuest.send()
            .map_err(|_| "Error in sending http GET request")?;

        let http_result = pending.try_wait(deadline)
            .map_err(|_| "Error in waiting http response back")?;
        let response = http_result.map_err(|_| "Error in waiting http_result convert response" )?;

        if response.code != 200 {
            debug::warn!("Unexpected status code: {}", response.code);
            return Err("Non-200 status code returned from http request");
        }

        let json_result: Vec<u8> = response.body().collect::<Vec<u8>>();

        // Print out the whole JSON blob
        print_bytes(&json_result);

        let json_val: JsonValue = simple_json::parse_json(
            &core::str::from_utf8(&json_result).unwrap())
            .map_err(|_| "JSON parsing error")?;

        Ok(json_val)
    }

    fn fetch_price<'a>(
        block_num:T::BlockNumber,
        account_id:&T::AccountId,
        sym: &'a [u8],
        remote_src: &'a [u8],
        remote_url: &'a [u8],
        current_time:T::Moment
    ) -> dispatch::Result {
        debug::info!("***fetch price***: {:?}:{:?},{:?}",
            core::str::from_utf8(sym).unwrap(),
            core::str::from_utf8(remote_src).unwrap(),
            current_time,
        );

        let json = Self::fetch_json(remote_url)?;
        let price = match remote_src {
            src if src == b"coincap" => Self::fetch_price_from_coincap(json)
                .map_err(|_| "fetch_price_from_coincap error"),
            src if src == b"cryptocompare" => Self::fetch_price_from_cryptocompare(json)
                .map_err(|_| "fetch_price_from_cryptocompare error"),
            _ => Err("Unknown remote source"),
        }?;

        debug::info!("当前的区块为:{:?}",block_num);
        let call = Call::record_price(
            block_num,
            (sym.to_vec(), remote_src.to_vec(), remote_url.to_vec()),
            price,account_id.clone());

        // Unsigned tx
        T::SubmitUnsignedTransaction::submit_unsigned(call)
            .map_err(|e| {
                debug::info!("{:?}",e);
                "============fetch_price: submit_signed(call) error=================="})?;

        debug::info!("***fetch price over ^_^***: {:?}:{:?},{:?}",
            core::str::from_utf8(sym).unwrap(),
            core::str::from_utf8(remote_src).unwrap(),
            current_time,
        );
        // Signed tx
        // let local_accts = T::SubmitTransaction::find_local_keys(None);
        // let (local_acct, local_key) = local_accts[0];
        // debug::info!("acct: {:?}", local_acct);
        // T::SignAndSubmitTransaction::sign_and_submit(call, local_key);

        // T::SubmitSignedTransaction::submit_signed(call);
         Ok(())
    }

    fn vecchars_to_vecbytes<I: IntoIterator<Item = char> + Clone>(it: &I) -> Vec<u8> {
        it.clone().into_iter().map(|c| c as u8).collect::<_>()
    }

    fn fetch_price_from_cryptocompare(json_val: JsonValue) -> StdResult<u64> {
        // Expected JSON shape:
        //   r#"{"USD": 7064.16}"#;

        let val_f64: f64 = json_val.get_object()[0].1.get_number_f64();
        let val_u64: u64 = (val_f64 * 10000.).round() as u64;
        Ok(val_u64)
    }

    fn fetch_price_from_coincap(json_val: JsonValue) -> StdResult<u64> {
        // Expected JSON shape:
        //   r#"{"data":{"priceUsd":"8172.2628346190447316"}}"#;

        const PRICE_KEY: &[u8] = b"priceUsd";
        let data = json_val.get_object()[0].1.get_object();

        let (_, v) = data.iter()
            .filter(|(k, _)| PRICE_KEY.to_vec() == Self::vecchars_to_vecbytes(k))
            .nth(0)
            .ok_or("fetch_price_from_coincap: JSON does not conform to expectation")?;

        // `val` contains the price, such as "222.333" in bytes form
        let val_u8: Vec<u8> = v.get_bytes();

        // Convert to number
        let val_f64: f64 = core::str::from_utf8(&val_u8)
            .map_err(|_| "fetch_price_from_coincap: val_f64 convert to string error")?
            .parse::<f64>()
            .map_err(|_| "fetch_price_from_coincap: val_u8 parsing to f64 error")?;
        let val_u64 = (val_f64 * 10000.).round() as u64;
        Ok(val_u64)
    }

    fn aggregate_pp<'a>(block: T::BlockNumber, sym: &'a [u8], freq: usize) -> dispatch::Result {
        let ts_pp_vec = <TokenSrcPPMap>::get(sym);

        // use the last `freq` number of prices and average them
        let amt: usize = if ts_pp_vec.len() > freq { freq } else { ts_pp_vec.len() };
        let pp_inds: &[u32] = ts_pp_vec.get((ts_pp_vec.len() - amt)..ts_pp_vec.len())
            .ok_or("aggregate_pp: extracting TokenSrcPPMap error")?;

        let src_pp_vec: Vec<_> = Self::src_price_pts();
        let price_sum: u64 = pp_inds.iter().fold(0, |mem, ind| mem + src_pp_vec[*ind as usize].1);
        let price_avg: u64 = (price_sum as f64 / amt as f64).round() as u64;

        // submit onchain call for aggregating the price
        let call = Call::record_agg_pp(block, sym.to_vec(), price_avg);

        // Unsigned tx
        T::SubmitUnsignedTransaction::submit_unsigned(call)
            .map_err(|_| "aggregate_pp: submit_signed(call) error")

        // Signed tx
        // T::SubmitSignedTransaction::submit_signed(call);
        // Ok(())
    }
}


// The module's dispatchable functions.
decl_module! {
  /// The module declaration.
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    // Initializing events
    // this is needed only if you are using events in your module
    fn deposit_event() = default;

    // Clean the state on initialization of the block
    fn on_initialize(block_num: T::BlockNumber) {
        // 每个时间周期清理 PricePoints
         if (block_num % T::TwoHour::get()).is_zero() {
            let duration = block_num / T::TwoHour::get();
            if duration >  <BlockNumberOf<T>>::from(2){  // 当前的时间周期
                 for key_value in <DeletePricePoints<T>>::enumerate().into_iter(){ // sym,vec<>, linked_map的作用
                    let (sym,blocknum_list) = key_value;
                    let index_len = blocknum_list.len();
                    debug::info!("------清理工作------------");
                    debug::info!("key_value: {:?}, {:?},and len={:?}",core::str::from_utf8(&sym).unwrap(),blocknum_list,index_len);
                    if index_len == 1{ // 只有1个就不删除
                        continue;
                    }
                    for block_num in &blocknum_list[..index_len-1]{   // vec<>
                       <PricePoints<T>>::remove(&sym,block_num); // i32
                    }
                    // DeletePricePoints 也只保留一个
                    <DeletePricePoints<T>>::insert(&sym,&blocknum_list[index_len-1..]);
                }
            }
        }

        // 每天清理错误的列表 SrcPriceFailed todo:TwoHour 改为 DAY
        if (block_num % T::Hour::get()).is_zero() {
            // delete
            for key_value in <DeletePricePoints<T>>::enumerate().into_iter(){
                 let (sym,_) = key_value;
                 <SrcPriceFailed<T>>::remove(&sym);
                 SrcPriceFailedCnt::remove(&sym);
            }
        }
    }


    pub fn record_price(
      origin,
      _block_num:T::BlockNumber,
      crypto_info: (Vec<u8>, Vec<u8>, Vec<u8>),
      price: u64,account_id: T::AccountId
    ) ->dispatch::Result {
      ensure_none(origin)?;
      let (sym, remote_src) = (&crypto_info.0, &crypto_info.1);
      let now = <timestamp::Module<T>>::get();
      // 转化为小写的字节码
      let sym_string = core::str::from_utf8(sym).map_err(|e|"symbol from utf8 to str failed")?.to_lowercase();
      let sym = &sym_string.as_bytes().to_vec();
      // Debug printout
      debug::info!("----上链: record_price-----: {:?}, {:?}, {:?}",
        core::str::from_utf8(sym).unwrap(),
        core::str::from_utf8(remote_src).unwrap(),
        price
      );

      // Spit out an event and Add to storage
      let price_info = PriceInfo{dollars:price.clone(),account:account_id,url:crypto_info.1.clone()};
      let price_pt = (now, price_info);
      let block_num = <system::Module<T>>::block_number();
      let duration = block_num / T::TwoHour::get();
      // 添加到队列
      <PricePoints<T>>::mutate(
        &crypto_info.0, &duration,
        |vec| vec.push(price_pt),
        );
        let delete_ppoints = <DeletePricePoints<T>>::get(&crypto_info.0);
        let length = delete_ppoints.len();
        if length !=0{
            let last_index = delete_ppoints[length-1];
            if last_index < duration{  // duration 表示当前的4小时区块周期数
                <DeletePricePoints<T>>::mutate(
                &crypto_info.0,
                |vec| vec.push(duration),
                );
            }
        }else{
            <DeletePricePoints<T>>::mutate(
                &crypto_info.0,
                |vec| vec.push(duration),
                );
        }

      // Spit out an event and Add to storage
      Self::deposit_event(RawEvent::FetchedPrice(
        sym.clone(), remote_src.clone(), now.clone(), price));

       // todo : 多余
      let price_pt = (now, price);
      // The index serves as the ID
      let pp_id: u32 = Self::src_price_pts().len().try_into().unwrap();
      <SrcPricePoints<T>>::mutate(|vec| vec.push(price_pt));
      <TokenSrcPPMap>::mutate(sym.clone(), |token_vec| token_vec.push(pp_id));
      <RemoteSrcPPMap>::mutate(remote_src, |rs_vec| rs_vec.push(pp_id));

      // set the flag to kick off update aggregated pricing in offchain call
      <UpdateAggPP>::mutate(sym.clone(), |freq| *freq += 1);
      debug::info!("----上链成功: record_price-----: {:?}, {:?}, {:?}",
            core::str::from_utf8(sym).unwrap(),
            core::str::from_utf8(remote_src).unwrap(),
            price
      );
      Ok(())
    }


    fn record_fail_fetchprice(_origin,block:T::BlockNumber,symbol:Vec<u8>,price_failed:PriceFailedOf<T>)->dispatch::Result{
        // 记录获取price失败的信息
        ensure_none(_origin)?;
        <SrcPriceFailed<T>>::mutate(&symbol, |fetch_failed| fetch_failed.push(price_failed));
        SrcPriceFailedCnt::mutate(&symbol,|cnt|*cnt += 1);
        debug::info!("------上链成功:record_fail_fetchprice--------");
         Ok(())
    }


    pub fn record_agg_pp(
      origin,
      _block: T::BlockNumber,
      sym: Vec<u8>,
      price: u64
    ) -> dispatch::Result {
      // Debug printout
      debug::info!("record_agg_pp: {:?}: {:?}",
        core::str::from_utf8(&sym).unwrap(),
        price
      );

      let now = <timestamp::Module<T>>::get();

      // Spit the event
      Self::deposit_event(RawEvent::AggregatedPrice(
        sym.clone(), now.clone(), price.clone()));

      // Record in the storage
      let price_pt = (now.clone(), price.clone());
      let pp_id: u32 = Self::agg_price_pts().len().try_into().unwrap();
      <AggPricePoints<T>>::mutate(|vec| vec.push(price_pt));
      <TokenAggPPMap>::mutate(sym.clone(), |vec| vec.push(pp_id));

      // Turn off the flag as the request has been handled
      <UpdateAggPP>::mutate(sym.clone(), |freq| *freq = 0);

      Ok(())
    }


    fn offchain_worker(block: T::BlockNumber) {
      let duration = T::TwoHour::get();
      // Type I task: fetch_price
      if duration > 0.into() && block % duration == 0.into() {
        if runtime_io::offchain::is_validator() { // 是否是验证人的模式启动
             if let Some(key) = Self::authority_id() {
                Self::offchain(block,&key);
            }
        }
      }
    } // end of `fn offchain_worker()`

  }
}

//fn public_to_accountid()->AccountId{
//
//}

#[allow(deprecated)]
impl<T: Trait> support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(call: &Self::Call) -> TransactionValidity {
        let now = <timestamp::Module<T>>::get();
        match call {
            Call::record_price(block,(sym, remote_src, ..),price,account_id) => {
                debug::info!("############## record_price :{:?}##############",now);
                Ok(ValidTransaction {
                priority: 0,
                requires: vec![],
                provides: vec![(block, sym, remote_src, account_id).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
                })
            },
            Call::record_fail_fetchprice(block,sym,price_failed) => {
                debug::info!("############## record_fail_fetchprice :{:?}##############",now);
                Ok(ValidTransaction {
                priority: 1,
                requires: vec![],
                provides: vec![(block,sym,price_failed).encode()], // vec![(now).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            })},
            Call::record_agg_pp(..) => Ok(ValidTransaction {
                priority: 2,
                requires: vec![],
                provides: vec![(now).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            }),
            _ => InvalidTransaction::Call.into()
        }
    }
}

/// tests for this module
#[cfg(test)]
mod tests {
    use super::*;

    use primitives::H256;
    use support::{impl_outer_origin, assert_ok, parameter_types};
    use sr_primitives::{
        traits::{BlakeTwo256, IdentityLookup}, testing::Header, weights::Weight, Perbill,
    };

    impl_outer_origin! {
    pub enum Origin for Test {}
  }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
  }
    impl system::Trait for Test {
        type Origin = Origin;
        type Call = ();
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type BlockHashCount = BlockHashCount;
        type MaximumBlockWeight = MaximumBlockWeight;
        type MaximumBlockLength = MaximumBlockLength;
        type AvailableBlockRatio = AvailableBlockRatio;
        type Version = ();
    }
    impl Trait for Test {
        type Event = ();
    }
    type TemplateModule = Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities {
        system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
    }

    #[test]
    fn it_works_for_default_value() {
        new_test_ext().execute_with(|| {
            // Just a dummy test for the dummy funtion `do_something`
            // calling the `do_something` function with a value 42
            assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
            // asserting that the stored value is equal to what we stored
            assert_eq!(TemplateModule::something(), Some(42));
        });
    }
}
