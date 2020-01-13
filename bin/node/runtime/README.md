## settings/developer
配置json格式
```json
{
  
  "MineIndex": "u64",
  "BlockNumberOf": "u32",
  "MineTag": " ",
  "OwnerMineRecordItem": {
    "mine_tag": "MineTag",
    "mine_count": "u16",
    "timestamp": "Moment",
    "blocknum": "u32",
    "miner_address": "AccountId",
    "from_address": "Vec<u8>",
    "to_address": "Vec<u8>",
    "symbol": "Vec<u8>",
    "amount": "Balance",
    "blockchain": "Vec<u8>",
    "tx": "Vec<u8>",
    "usdt_amount": "u64",
    "pcount_workforce": "u64",
    "pamount_workforce": "u64",
    "reward": "Balance",
    "superior_reward": "Balance",
    "on_reward": "Balance"
  },
  "OwnerMineWorkForce": {
    "mine_cnt": "u64",
    "usdt_nums": "u64",
    "work_force": "u64",
    "settle_blocknumber": "u32"
  },
  "PriceInfo": {
    "dollars": "u64",
    "account": "AccountId",
    "url": "Vec<u8>"
  },
  "PriceFailed":{
    "account": "AccountId",
    "sym": "Vec<u8>",
    "errinfo": "Vec<u8>"
  },
  "PriceFailedOf": "PriceFailed",
  
  "MinerInfo": {
      "hardware_id": "Vec<u8>",
      "father_address": "Option<AccountId",
      "grandpa_address": "Option<AccountId>",
      "register_time": "Moment",
      "machine_state": "Vec<u8>",
      "machine_owner": "AccountId"
  },
  
  "VoteInfo": {
        "start_vote_block": "BlockNumber",
        "symbol": "Vec<u8>",
        "tx": "Vec<u8>",
        "tx_hash": "Hash",
        "reporter": "AccountId",
        "report_reason": "Vec<u8>",
        "illegal_man": "AccountId",
        "transaction_amount": "Balance",
        "usdt_amount": "Balance",
        "decimals": "u32",
        "approve_mans": "Vec<AccountId>",
        "reject_mans": "Vec<AccountId>"
  }
  
}

```
