/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs


// We have to import a few things
//use simple_json::json::String;
use rstd::prelude::*;
use app_crypto::RuntimeAppPublic;
use support::traits::{Get};
use support::{debug,Parameter,decl_module, decl_storage, decl_event, print,StorageValue,StorageMap,StorageDoubleMap, dispatch::Result};
use system::{ensure_signed,ensure_none};
use system::offchain::{SubmitSignedTransaction, SubmitUnsignedTransaction};
use codec::{Encode, Decode,alloc::string::String};
//use simple_json::{ self, json::JsonValue };
use serde::{Serialize, Deserialize};
use core::convert::{ TryInto };
// use sp_runtime::traits::{Hash,SimpleArithmetic, Bounded, One, Member,CheckedAdd};
use sp_runtime::{
    transaction_validity::{
        TransactionValidity, TransactionLongevity, ValidTransaction, InvalidTransaction
    },
    traits::{CheckedSub,CheckedAdd,Printable,Member,Zero},
};

use primitives::{
//	crypto::KeyTypeId,
    offchain,
};

/// Our local KeyType.
///
/// For security reasons the offchain worker doesn't have direct access to the keys
/// but only to app-specific subkeys, which are defined and grouped by their `KeyTypeId`.
/// We define it here as `ofcb` (for `offchain callback`). Yours should be specific to
/// the module you are actually building.
pub const KEY_TYPE: app_crypto::KeyTypeId = app_crypto::KeyTypeId(*b"ofpf");

pub mod sr25519 {
    mod app_sr25519 {
        use crate::offchain_pricefetch::KEY_TYPE;
        use app_crypto::{app_crypto, sr25519};
        app_crypto!(sr25519, KEY_TYPE);
    }

    /// An i'm online keypair using sr25519 as its crypto.
    #[cfg(feature = "std")]
    pub type AuthorityPair = app_sr25519::Pair;

    /// An i'm online signature using sr25519 as its crypto.
    pub type AuthoritySignature = app_sr25519::Signature;

    /// An i'm online identifier using sr25519 as its crypto.
    pub type AuthorityId = app_sr25519::Public;
}

pub mod ed25519 {
    mod app_ed25519 {
        use app_crypto::{app_crypto, key_types::IM_ONLINE, ed25519};
        app_crypto!(ed25519, IM_ONLINE);
    }

    /// An i'm online keypair using ed25519 as its crypto.
    #[cfg(feature = "std")]
    pub type AuthorityPair = app_ed25519::Pair;

    /// An i'm online signature using ed25519 as its crypto.
    pub type AuthoritySignature = app_ed25519::Signature;

    /// An i'm online identifier using ed25519 as its crypto.
    pub type AuthorityId = app_ed25519::Public;
}


#[derive(Debug)]
enum OffchainErr {
    DecodeWorkerStatus,
    FailedSigning,
    NetworkState,
    SubmitTransaction,
}

impl Printable for OffchainErr {
    fn print(&self) {
        match self {
            OffchainErr::DecodeWorkerStatus => print("Offchain error: decoding WorkerStatus failed!"),
            OffchainErr::FailedSigning => print("Offchain error: signing failed!"),
            OffchainErr::NetworkState => print("Offchain error: fetching network state failed!"),
            OffchainErr::SubmitTransaction => print("Offchain error: submitting transaction failed!"),
        }
    }
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq,Serialize, Deserialize)]
pub struct Price<AccountId> {  // 存储币种价格
    dollars: u32,  // 美元
    cents: u32, // up to 4 digits
    account: AccountId, //哪个账号查询到的
    url:Vec<u8>,    // 对应的url,短写
}

#[derive(Debug, Clone, PartialEq, Eq,Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data{
    id:String,
    symbol:String,
    #[serde(rename="priceUsd")]
    price_usd:String,
    #[serde(rename="vwap24Hr")]
    vwap24hr:String,
    #[serde(rename="changePercent24Hr")]
    change_percent24hr:String,

}

#[derive(Debug,Clone, PartialEq, Eq,Serialize, Deserialize)]
pub struct SymbolFetch {  // 存储币种价格
    data:Data,
    timestamp:u64,
}


#[derive(Debug, Encode, Decode, Clone, PartialEq)]
pub struct PriceFailed<AccountId,Moment> {  // 存储币种价格
    dollars: u32,  // 美元
    cents: u32, // up to 4 digits
    account: AccountId, //哪个账号查询到的
    url:Vec<u8>,    // 对应的url
    timestamp:Moment,
    errinfo:Vec<u8>,
}

type PriceFailedOf<T> = PriceFailed<<T as system::Trait>::AccountId,<T as timestamp::Trait>::Moment>;

// This automates price fetching every certain blocks. Set to 0 disable this feature.
//   Then you need to manucally kickoff pricefetch
pub const BLOCK_FETCH_DUR: u64 = 5;

pub const FETCHED_CRYPTOS: [(&'static [u8], &'static [u8], &'static [u8]); 2] = [
    (b"BTC", b"coincap",
     b"https://api.coincap.io/v2/assets/bitcoin"),  //  (b"test",b"google",b"http://www.huobi.br.com/"),
//   (b"BTC", b"coinmarketcap",
//    b"https://sandbox-api.coinmarketcap.com/v1/cryptocurrency/quotes/latest?CMC_PRO_API_KEY=2e6d8847-bcea-4999-87b1-ad452efe4e40&symbol=BTC"),
     (b"ETH", b"coincap",
      b"https://api.coincap.io/v2/assets/ethereum"),
//     (b"ETH", b"coinmarketcap",
//      b"https://sandbox-api.coinmarketcap.com/v1/cryptocurrency/quotes/latest?CMC_PRO_API_KEY=2e6d8847-bcea-4999-87b1-ad452efe4e40&symbol=ETH"),
];

pub type StdResult<T> = core::result::Result<T, &'static str>;

/// The module's configuration trait.
pub trait Trait: timestamp::Trait + system::Trait+ authority_discovery::Trait{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// A dispatchable call type. We need to define it for the offchain worker to
    /// reference the `pong` function it wants to call.
    type Call: From<Call<Self>>;

    /// Let's define the helper we use to create signed transactions with
    type SubmitTransaction: SubmitSignedTransaction<Self, <Self as Trait>::Call>;
    type SubmitUnsignedTransaction: SubmitUnsignedTransaction<Self, <Self as Trait>::Call>;

    /// The local AuthorityId
    type AuthorityId: Member + Parameter + RuntimeAppPublic + Default + Ord;

    type TwoHour: Get<Self::BlockNumber>;
    type Day: Get<Self::BlockNumber>;
}

/// The type of requests we can send to the offchain worker
#[cfg_attr(feature = "std", derive(PartialEq, Debug))]
#[derive(Encode, Decode)]
pub enum OffchainRequest {
    /// If an authorised offchain worker sees this, will kick off to work
    PriceFetch(Vec<u8>, Vec<u8>, Vec<u8>)
}

decl_event!(
  pub enum Event<T> where
    Moment = <T as timestamp::Trait>::Moment,
    AccountId = <T as system::Trait>::AccountId,
    {

    PriceFetched(Vec<u8>, Vec<u8>, Moment, Price<AccountId>),
    AggregatedPrice(Vec<u8>, Moment, Price<AccountId>),
  }
);

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as PriceFetch {
    // storage about offchain worker tasks
    pub OcRequests get(oc_requests): Vec<OffchainRequest>;

    // 页面添加网址 ,形如 FETCHED_CRYPTOS
    pub FetchUrlList get(add_url): Vec<(Vec<u8>,Vec<u8>,Vec<u8>)>;

    // storage about source price points
    // key:4小时的区块个数+币名字(名字统一小写), value:时间与币价格 .列表存放
    pub PricePoints get(price_pts): double_map T::BlockNumber, blake2_256(Vec<u8>) => Vec<(T::Moment, Price<T::AccountId>)>;

    // 记录 哪个节点ccountId,时间,哪个 url 没有查询到数据.保存一天数据.key: 币名字, value:时间 , url
    pub SrcPriceFailed get(src_price_failed): map Vec<u8> => Vec<(PriceFailedOf<T>)>;

    ValidatorList get(get_validators): Vec<T::AccountId>;

    pub UpdateAggPP get(update_agg_pp): bool;
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
    fn on_initialize(block: T::BlockNumber) {
        <Self as Store>::OcRequests::kill();
        if (block % T::TwoHour::get()).is_zero() {
            // 删除某个价格列表 double_map
            let _ = Self::enque_pricefetch_tasks();  // 每5个区块执行一次该函数,添加到队列中
            }
    }

    fn add_urls(origin,symbol:Vec<u8>,short_domain:Vec<u8>,url:Vec<u8>) {
        let author = ensure_signed(origin)?; //todo symbol是否该大小写转换
        let symbol = rstd::str::from_utf8(&symbol).map_err(|e|"symbol from utf8 to str failed")?.to_lowercase();
        if Self::is_authority(&author) {   // 高级节点才有权限添加
            <FetchUrlList>::mutate(|v| v.push((symbol.as_bytes().to_vec(),short_domain,url)));
        }
        Ok(())
    }

    pub fn record_price(
        _origin,
        crypto_info: (Vec<u8>, Vec<u8>, Vec<u8>),
        price: Price<T::AccountId>,
        _signature: <T::AuthorityId as RuntimeAppPublic>::Signature  // 需要验证
    )  {
      let (symbol, remote_src) = (crypto_info.0, crypto_info.1); // coinName,
      let now = <timestamp::Module<T>>::get();

      // Debug printout
      runtime_io::misc::print_utf8(b"record_price: called");
      runtime_io::misc::print_utf8(&symbol);
      runtime_io::misc::print_utf8(&remote_src);
      runtime_io::misc::print_num(price.dollars.into());
      runtime_io::misc::print_num(price.cents.into());

      // Spit out an event and Add to storage
      Self::deposit_event(RawEvent::PriceFetched(
        symbol.clone(), remote_src.clone(), now.clone(), price.clone()));

      let price_pt = (now, price);
      let block_num = <system::Module<T>>::block_number();
      let twohours = block_num / T::TwoHour::get();
      <PricePoints<T>>::mutate(
        &twohours,&crypto_info.0,
        |vec| vec.push(price_pt),
        );

      // The index serves as the ID
      // set the flag to kick off update aggregated pricing
      <UpdateAggPP>::mutate(|flag| *flag = true);

      Ok(())
    }

    fn record_fail_fetchprice( _origin,symbol:Vec<u8>,price_failed:PriceFailedOf<T>){
        // 记录获取price失败的信息
        <SrcPriceFailed<T>>::mutate(symbol, |fetch_failed| fetch_failed.push(price_failed));
         Ok(())
    }

//    pub fn record_agg_pp(_origin, sym: Vec<u8>, price: Price) -> Result {
//      // Debug printout
//      runtime_io::misc::print_utf8(b"record_agg_pp: called");
//      runtime_io::misc::print_utf8(&sym);
//      runtime_io::misc::print_num(price.dollars.into());
//      runtime_io::misc::print_num(price.cents.into());
//
//      let now = <timestamp::Module<T>>::get();
//      // Turn off the flag for request has been handled
//      <UpdateAggPP>::mutate(|flag| *flag = false);
//
//      // Spit the event
//      Self::deposit_event(RawEvent::AggregatedPrice(
//        sym.clone(), now.clone(), price.clone()));
//
//      // Record in the storage
//      let price_pt = (now.clone(), price.clone());
//      <AggPricePoints<T>>::mutate(|vec| vec.push(price_pt));
//      let pp_id: u64 = Self::agg_price_pts().len().try_into().unwrap();
//      <TokenAggPPMap>::mutate(sym, |vec| vec.push(pp_id));
//
//      Ok(())
//    }

    fn offchain_worker(_block: T::BlockNumber) {
        debug::RuntimeLogger::init();
        if runtime_io::offchain::is_validator() { // 是否是验证人的模式启动
             if let Some(key) = Self::authority_id() {
                Self::offchain(&key);
            }

        }
    } // end of `fn offchain_worker`


       fn on_finalize(n: T::BlockNumber) {
            if (n % T::TwoHour::get()).is_zero() {
                // 删除某个价格列表 double_map

            }
        }
    }
}

impl<T: Trait> Module<T> {
    fn enque_pricefetch_tasks() -> Result { // 写进队列
        for crypto_info in FETCHED_CRYPTOS.iter() {
            <OcRequests>::mutate(|v|
                v.push(OffchainRequest::PriceFetch(crypto_info.0.to_vec(),
                                                   crypto_info.1.to_vec(), crypto_info.2.to_vec()))
            );
        }

        for crypto_info in <Self as Store>::FetchUrlList::get().iter() {
            <OcRequests>::mutate(|v|
                v.push(OffchainRequest::PriceFetch(crypto_info.0.to_vec(),
                                                   crypto_info.1.to_vec(), crypto_info.2.to_vec()))
            );
        }
        let a = Self::oc_requests().len();
        #[cfg(feature = "std")]{
            println!("-----------len oc_requests {:?}------------",a);
        }
        Ok(())
    }

    pub fn offchain(key: &T::AccountId) {
        #[cfg(feature = "std")] {
            let now = <timestamp::Module<T>>::get();
            println!("---offchain_worker time:{:?}------", now);
        }
        // 验证是否
        // Type I task: fetch_price
        for fetch_info in Self::oc_requests() {
            let remote_url_clone;
            let sym_clone;
            let remote_src_clone;
            let res = match fetch_info {
                OffchainRequest::PriceFetch(sym, remote_src, remote_url) =>{
                // runtime_io::misc::print_utf8(&remote_url);
                    remote_url_clone = remote_url.clone();
                    sym_clone = sym.clone();
                    remote_src_clone = remote_src.clone();
                    Self::fetch_price(key,sym, remote_src, remote_url)
                }
            };
            if let Err(err_msg) = res {
                print(err_msg); // res的值可能是Err
                let price_failed = PriceFailed {
                    dollars: 0,
                    cents: 0,
                    account: key.clone(),
                    url: remote_url_clone,
                    timestamp: <timestamp::Module<T>>::get(),
                    errinfo: err_msg.as_bytes().to_vec(),
                };
                // 上报 todo: 实现签名
                let call = Call::record_fail_fetchprice((sym_clone, remote_src_clone, remote_url_clone), price_failed);
                T::SubmitUnsignedTransaction::submit_unsigned(call)
                    .map_err(|_| "fetch_price: submit_unsigned_call error")
            };
        }

        // Type II task: aggregate price
        if Self::update_agg_pp() {
            if let Err(err_msg) = Self::aggregate_pp() { print(err_msg); }
        }
    }

    fn is_authority(who: &T::AccountId) -> bool {
        // Vec<T::AccountId> 遍历出来,与who 对比,如果 AccountId==who,然后判断该值:如果此对象包含数据，则为真。
        let authorities = <authority_discovery::Module<T>>::authorities().iter().map(
            |i| (*i).clone().into()
        ).collect::<Vec<T::AccountId>>();
        Self::ValidatorList().into_iter().find(|i| i == who).is_some() ||
            authorities.into_iter().find(|i| i == who).is_some() // todo 待验证
    }


    /// Find a local `AccountId` we can sign with, that is allowed to offchainwork
    fn authority_id() -> Option<T::AccountId> {
        //通过本地化的密钥类型查找此应用程序可访问的所有本地密钥。
        // 然后遍历当前存储在chain上的所有ValidatorList，并根据本地键列表检查它们，直到找到一个匹配，否则返回None。
        let authorities = <authority_discovery::Module<T>>::authorities().iter().map(
            |i| (*i).clone().into()
        ).collect::<Vec<T::AccountId>>();
        let local_keys = T::AuthorityId::all().iter().map(
            |i| (*i).clone().into()
        ).collect::<Vec<T::AccountId>>();
        #[cfg(feature = "std")]{
            println!("----authority_id------{:?}",local_keys);}
        let authorities = <authority_discovery::Module<T>>::authorities().iter().map(
            |i| (*i).clone().into()
        ).collect::<Vec<T::AccountId>>();
        authorities().into_iter().find_map(|authority| {
            if local_keys.contains(&authority) {
                Some(authority)
            } else {
                None
            }
        })
    }

    fn fetch_json(remote_url: &str) -> StdResult<Vec<u8>> {
        #[cfg(feature = "std")]{
            println!("-----------fetch_json {:?}------------",remote_url);
        }

        let now = <timestamp::Module<T>>::get();  // 将时间转换为 u64
//        let wait_millis:u32 = 1000;
//        let deadline = now.checked_add(&T::Moment::from(wait_millis)).ok_or("checked_add overflow...")?; // 暂时放弃

        let deadline:u64 = now.try_into().
            map_err(|_|"An error occurred when moment was converted to usize")?  // usize类型
            .try_into().map_err(|_|"An error occurred when usize was converted to u64")?;

//    let deadline = now.checked_add(&T::Moment::from((10000 as u32).try_into().unwrap())).ok_or("mining function add overflow")?;
//    let deadline = deadline.try_into().map_err(|_e| "you have err")?.try_into().unwrap();
        let id = runtime_io::offchain::http_request_start("GET", remote_url, &[]).map_err(|_| "http_request start error")?;
        let _ = runtime_io::offchain::http_response_wait(&[id], Some(offchain::Timestamp::from_unix_millis(deadline+20000)));
        #[cfg(feature = "std")]{
            println!("-----------wait end {:?}------------",remote_url);
        }

        let mut json_result: Vec<u8> = vec![];
        loop {
            let mut buffer = vec![0; 1024];
            let _read = runtime_io::offchain::http_response_read_body(id, &mut buffer, Some(offchain::Timestamp::from_unix_millis(deadline+20000)))
                .map_err(|_e|  _e);
            json_result = [&json_result[..], &buffer[..]].concat();
//            let c = &json_result[..];

            match _read {
                Ok(0)=>{
                    #[cfg(feature = "std")] {
                        println!("break");}
                    break
                },
                Err(_read)=>{
                    #[cfg(feature = "std")] {
                        println!("break:_read size {:?}", _read);}
                    break
                },
                _ => {}
            }
        }

        #[cfg(feature = "std")]{
            println!("-----------fetch_json over{:?}------------",remote_url);
        }
        // Print out the whole JSON blob
        runtime_io::misc::print_utf8(&json_result);

//        let json_val: JsonValue = simple_json::parse_json(
//            &rstd::str::from_utf8(&json_result).unwrap())
//            .map_err(|_| "JSON parsing error")?;

        Ok(json_result)
    }

    fn fetch_price(key: &T::AccountId,sym: Vec<u8>, remote_src: Vec<u8>, remote_url: Vec<u8>) -> Result {
        runtime_io::misc::print_utf8(&sym);
        runtime_io::misc::print_utf8(&remote_src);
        runtime_io::misc::print_utf8(b"--fetch_json begin--");
        let fetch_res = Self::fetch_json(rstd::str::from_utf8(&remote_url).unwrap())?;
        runtime_io::misc::print_utf8(b"--fetch_json over--");

        let price = match remote_src.as_slice() { // 解析json
            src if src == b"coincap" => Self::fetch_price_from_coincap(key,sym,remote_src,&fetch_res)
                .map_err(|_| "fetch_price_from_coincap error")?,
            _ => return Err("Unknown remote source"),
        };


        let signature = key.sign(&price.encode()).ok_or(OffchainErr::FailedSigning)?;
        // let call = Call::heartbeat(heartbeat_data, signature); // 打包部分
        let call = Call::record_price((sym, remote_src, remote_url), price);
        T::SubmitUnsignedTransaction::submit_unsigned(call)
            .map_err(|_| "fetch_price: submit_unsigned_call error")
    }

    fn fetch_price_from_coincap(key:&T::AccountId,sym: Vec<u8>,remote_src:Vec<u8>,fetch: &[u8]) -> StdResult<Price<T::AccountId>> {
        runtime_io::misc::print_utf8(b"-- fetch_price_from_coincap");
        let symbol_fetch: SymbolFetch = serde_json::from_slice(fetch).map_err(|_|"utf-8 to price struct failed")?;
        let symbol_info = symbol_fetch.data;
        let price_usd = symbol_info.price_usd;
        let mut v:Vec<u64> = vec![];
        for i in price_usd.split("."){
            v.push(i.parse::<u64>().unwrap());
        }
        let remote_src = String::from_utf8(&remote_src).map_err(|e|"symbol from utf8 to str failed")?;
        let p = Price{
            dollars:v.0,
            cents:v.1,
            account:key,
            url:remote_src,
        };
        Ok(p)
    }

    fn aggregate_pp() -> Result {
//        let mut pp_map = BTreeMap::new();

        // TODO: calculate the map of sym -> pp
//        pp_map.insert(b"BTC".to_vec(), Price::new(100, 3500, None));
//
//        pp_map.iter().for_each(|(sym, price)| {
//            let call = Call::record_agg_pp(sym.clone(), price.clone());
//            if let Err(_) = T::SubmitUnsignedTransaction::submit_unsigned(call) { // 提交没有签名
//                print("aggregate_pp: submit_unsigned_call error");
//            }
//        });
        Ok(())
    }
}

impl<T: Trait> support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(call: &Self::Call) -> TransactionValidity {
        let now = <timestamp::Module<T>>::get();
        match call {
            Call::record_price(..) => {
                runtime_io::misc::print_utf8(b"############## record_price ##############");
                Ok(ValidTransaction {
                    priority: 0,
                    requires: vec![],
                    provides: vec![(now).encode()],
                    longevity: TransactionLongevity::max_value(),
                    propagate: true,
                })},
            Call::record_fail_fetchprice(..) => {
                runtime_io::misc::print_utf8(b"############## record_fail_fetchprice ##############");
                Ok(ValidTransaction {
                    priority: 0,
                    requires: vec![],
                    provides: vec![(now).encode()],
                    longevity: TransactionLongevity::max_value(), // 永远有效
                    propagate: true,
                })
            },
            _ => InvalidTransaction::Call.into()
        }
    }
}

// tests for this module

