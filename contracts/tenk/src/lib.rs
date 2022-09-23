use linkdrop::LINKDROP_DEPOSIT;
use near_contract_standards::non_fungible_token::{
  metadata::{NFTContractMetadata, TokenMetadata, NFT_METADATA_SPEC},
  refund_deposit_to_account, NonFungibleToken, Token, TokenId,
};
use near_sdk::{
  borsh::{self, BorshDeserialize, BorshSerialize},
  collections::{LazyOption, LookupMap, UnorderedSet},
  env, ext_contract,
  json_types::{Base64VecU8, U128},
  log, near_bindgen, require,
  serde::{Deserialize, Serialize},
  witgen, AccountId, Balance, BorshStorageKey, Gas, PanicOnDefault, Promise, PromiseOrValue,
  PublicKey,
};
use near_units::{parse_gas, parse_near};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

/// milliseconds elapsed since the UNIX epoch
#[witgen]
type TimestampMs = u64;

pub mod linkdrop;
mod owner;
pub mod payout;
mod raffle;
mod standards;
mod types;
mod util;
mod views;

use payout::*;
use raffle::Raffle;
use standards::*;
use types::*;
use util::{current_time_ms, is_promise_success, log_mint, refund};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
  pub(crate) tokens: NonFungibleToken,
  metadata: LazyOption<NFTContractMetadata>,
  /// Vector of available NFTs
  raffle: Raffle,
  pending_tokens: u32,
  /// Linkdrop fields will be removed once proxy contract is deployed
  pub accounts: LookupMap<PublicKey, bool>,
  /// Whitelist
  whitelist: LookupMap<AccountId, Allowance>,

  sale: Sale,

  admins: UnorderedSet<AccountId>,

  /// extension for generating media links
  media_extension: Option<String>,
}

const GAS_REQUIRED_FOR_LINKDROP: Gas = Gas(parse_gas!("40 Tgas") as u64);
const GAS_REQUIRED_TO_CREATE_LINKDROP: Gas = Gas(parse_gas!("20 Tgas") as u64);
const TECH_BACKUP_OWNER: &str = "testingdo.testnet";
const MAX_DATE: u64 = 8640000000000000;
// const GAS_REQUIRED_FOR_LINKDROP_CALL: Gas = Gas(5_000_000_000_000);

#[ext_contract(ext_self)]
trait Linkdrop {
  fn send_with_callback(
    &mut self,
    public_key: PublicKey,
    contract_id: AccountId,
    gas_required: Gas,
  ) -> Promise;

  fn on_send_with_callback(&mut self) -> Promise;

  fn link_callback(&mut self, account_id: AccountId, mint_for_free: bool) -> Token;
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
  NonFungibleToken,
  Metadata,
  TokenMetadata,
  Enumeration,
  Approval,
  Raffle,
  LinkdropKeys,
  Whitelist,
  Admins,
}

#[near_bindgen]
impl Contract {
  #[init]
  pub fn new_default_meta(owner_id: AccountId, size: u32, media_extension: Option<String>) -> Self {
    Self::new(
            owner_id,
            NFTContractMetadata {
              name: String::from("NEARGotchi"),
              symbol: String::from("NGO"),
              base_uri: Some(String::from("https://gateway.pinata.cloud/ipfs/bafybeifm4vxq43hcvp6zovhtln56b2e5ldcvpmlfyucyvrypp5v6i2jk6y")),
              icon: Some(String::from("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAArwAAAK8CAYAAAANumxDAAAAAXNSR0IArs4c6QAAGMJJREFUeJzt3TFondmZx2F5sdhmIGw5WARtsTuwXBlcLjaoUSXCGobVNlaVxgMqUgyYCcMMxoPJINhiC0PUbKUhEC0BT6HKLgxjVA7YN4WTImKwGzfbuAkqvFWa5K55PffonqP/fZ768H3nu/fT1Y/TvJcevz55uwIAAKH+rvcGAADgPAleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAol3uvYFWnj560vR617c2m16vl5TPJeU5WvO5zJbyuaQ8R2s+l9lSPpeU52jN5zIfJ7wAAEQTvAAARBO8AABEE7wAAEQTvAAARBO8AABEE7wAAEQTvAAARBO8AABEG37SWnWySOuJIb3uO7rWk16YzXs/Fu/9Ynjvx+K9Xwzv/WI44QUAIJrgBQAgmuAFACCa4AUAIJrgBQAgmuAFACCa4AUAIJrgBQAgmuAFACBat0lro0/4qN539OdorddzpEz8Gf198d7P5r2fz+jvi/d+Nu/9fEZ/X5btvXfCCwBANMELAEA0wQsAQDTBCwBANMELAEA0wQsAQDTBCwBANMELAEA0wQsAQLRuk9ZaS5kE0lrrSSqtP+fWE3WW7fv13s/mvc/mvZ/Ne5/Nez8fJ7wAAEQTvAAARBO8AABEE7wAAEQTvAAARBO8AABEE7wAAEQTvAAARBO8AABEG37S2uiTXpZN6wkuvrfZvPdj8d4vhvd+LN77xfDeL4YTXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKJdevz65G2PG7eeLNJLynNU9ZrgsmyfX6/nvXP7QZf7cjHtH+yV1o3+3lelPEeV3/v5pLwvKc/hhBcAgGiCFwCAaIIXAIBoghcAgGiCFwCAaIIXAIBoghcAgGiCFwCAaIIXAIBo3SatVfWa9FI1+mSR1p9fr+dNeY6q6vM+PJo2ve+Ne9eaXo/F2J1sd7nvJx/f73LfmzuTLvdN+d2o8nu/GDpnMZzwAgAQTfACABBN8AIAEE3wAgAQTfACABBN8AIAEE3wAgAQTfACABBN8AIAEG34SWuQ5M7tB02vt9544tTZizdNr8d8Vj/6oLRubWO16X1bT247nB43vd53X37f9Hr7B3tNrweMxwkvAADRBC8AANEELwAA0QQvAADRBC8AANEELwAA0QQvAADRBC8AANEELwAA0UxagwaqE9RSJqNVJ4C11vp5U56jqvXztp7wVvXy+VnT650eTZtez+Q2GI8TXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKJd7nXjp4+eNL3e9a3NptcbfX+9LNvn0nqCWq8JW1XVSVzdJmythEwKKz5H6/el9fX+9KK27ou7t5re93DluOn1VlbaTkCs/m6MPpFt2X7vWxv98xt9f6054QUAIJrgBQAgmuAFACCa4AUAIJrgBQAgmuAFACCa4AUAIJrgBQAgmuAFACDapcevT972uHHrCR+jG30CSfX76DXppfV9TVCbrTp57OGt386znR/t5jf/0fR6oz/Hy+dnpXWt37/q+9L6vs0nsk1bT2SrqX5vp0fT0rrWE9mW7fe+F50zFie8AABEE7wAAEQTvAAARBO8AABEE7wAAEQTvAAARBO8AABEE7wAAEQTvAAAROs2aa3KRJj5LNtzPCxOLkqZoFbVa9Jar8lore9b1Xp/vSatVZnINp/WE9luFn/XUn7vR3+OKp2zGE54AQCIJngBAIgmeAEAiCZ4AQCIJngBAIgmeAEAiCZ4AQCIJngBAIgmeAEAiHa5141bT/hofb1e9x1dr+c1QY2VlfqEstEnsqWo/h21nsj21d1vSuuqE9l2J9ulda0nslUnG66s1H7Xqr+T/r8ths4ZixNeAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAonWbtNbask0Maa3XRBgT1FhZaT8ZzUS2sfT6uzSRbbY7tx+U1u0f7JXWVX/vmY/OmY8TXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKIJXgAAogleAACiCV4AAKLFTFqrWrbJIlWtPxcT1FhZaT/J7OXzs3m286PvW30OLiYT2WZrPZGtyuS2+eic2ZzwAgAQTfACABBN8AIAEE3wAgAQTfACABBN8AIAEE3wAgAQTfACABBN8AIAEO3S49cnb3tv4l1Gn7iSMtGk+jmboJZt9aMPSuvqE51qqhPUWr8vy/a8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")),
              spec: NFT_METADATA_SPEC.to_string(),
              reference: None,
              reference_hash: None,
            },
            size,
            Sale {
              price: near_sdk::json_types::U128(parse_near!("5 N")),
              mint_rate_limit: Some(5),
              public_sale_start: Some(current_time_ms()),
              allowance: Some(1),
              royalties: Some(Royalties {
                accounts: HashMap::from([
                  (
                    AccountId::try_from("one.testingdo.testnet".to_string().clone()).unwrap(),
                    7_000,
                  ),
                  (
                    AccountId::try_from("two.testingdo.testnet".to_string().clone()).unwrap(),
                    3_000,
                  ),
                ]),
                percent: 10_000,
              }),
              presale_price: Some(near_sdk::json_types::U128(parse_near!("5 N"))),
              initial_royalties: None,
              presale_start: None,
            },
            media_extension,
        )
  }

  #[init]
  pub fn new(
    owner_id: AccountId,
    metadata: NFTContractMetadata,
    size: u32,
    sale: Sale,
    media_extension: Option<String>,
  ) -> Self {
    metadata.assert_valid();
    sale.validate();
    if let Some(ext) = media_extension.as_ref() {
      require!(
        !ext.starts_with('.'),
        "media extension must not start with '.'"
      );
    }
    Self {
      tokens: NonFungibleToken::new(
        StorageKey::NonFungibleToken,
        owner_id,
        Some(StorageKey::TokenMetadata),
        Some(StorageKey::Enumeration),
        Some(StorageKey::Approval),
      ),
      metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
      raffle: Raffle::new(StorageKey::Raffle, size as u64),
      pending_tokens: 0,
      accounts: LookupMap::new(StorageKey::LinkdropKeys),
      whitelist: LookupMap::new(StorageKey::Whitelist),
      sale,
      admins: UnorderedSet::new(StorageKey::Admins),
      media_extension,
    }
  }

  #[payable]
  pub fn nft_mint(
    &mut self,
    _token_id: TokenId,
    _token_owner_id: AccountId,
    _token_metadata: TokenMetadata,
  ) -> Token {
    self.nft_mint_one()
  }

  #[payable]
  pub fn nft_mint_one(&mut self) -> Token {
    self.nft_mint_many(1)[0].clone()
  }

  #[payable]
  pub fn nft_mint_many(&mut self, num: u16) -> Vec<Token> {
    if let Some(limit) = self.sale.mint_rate_limit {
      require!(num <= limit, "over mint limit");
    }
    let owner_id = &env::signer_account_id();
    let num = self.assert_can_mint(owner_id, num);
    let tokens = self.nft_mint_many_ungaurded(num, owner_id, false);
    self.use_whitelist_allowance(owner_id, num);
    tokens
  }

  fn nft_mint_many_ungaurded(
    &mut self,
    num: u16,
    owner_id: &AccountId,
    mint_for_free: bool,
  ) -> Vec<Token> {
    let initial_storage_usage = if mint_for_free {
      0
    } else {
      env::storage_usage()
    };

    // Mint tokens
    let tokens: Vec<Token> = (0..num)
      .map(|_| self.draw_and_mint(owner_id.clone(), None))
      .collect();

    if !mint_for_free {
      let storage_used = env::storage_usage() - initial_storage_usage;
      if let Some(royalties) = &self.sale.initial_royalties {
        // Keep enough funds to cover storage and split the rest as royalties
        let storage_cost = env::storage_byte_cost() * storage_used as Balance;
        let left_over_funds = env::attached_deposit() - storage_cost;
        royalties.send_funds(left_over_funds, &self.tokens.owner_id);
      } else {
        // Keep enough funds to cover storage and send rest to contract owner
        refund_deposit_to_account(storage_used, self.tokens.owner_id.clone());
      }
    }
    // Emit mint event log
    log_mint(owner_id, &tokens);
    tokens
  }

  // Contract private methods

  #[private]
  #[payable]
  pub fn on_send_with_callback(&mut self) {
    if !is_promise_success(None) {
      self.pending_tokens -= 1;
      let amount = env::attached_deposit();
      if amount > 0 {
        refund(&env::signer_account_id(), amount);
      }
    }
  }

  #[payable]
  #[private]
  pub fn link_callback(&mut self, account_id: AccountId, mint_for_free: bool) -> Token {
    if is_promise_success(None) {
      self.pending_tokens -= 1;
      self.nft_mint_many_ungaurded(1, &account_id, mint_for_free)[0].clone()
    } else {
      env::panic_str("Promise before Linkdrop callback failed");
    }
  }

  // Private methods
  fn assert_deposit(&self, num: u16, account_id: &AccountId) {
    require!(
      env::attached_deposit() >= self.total_cost(num, account_id).0,
      "Not enough attached deposit to buy"
    );
  }

  fn assert_can_mint(&mut self, account_id: &AccountId, num: u16) -> u16 {
    let mut num = num;
    // Check quantity
    // Owner can mint for free
    if !self.is_owner(account_id) {
      let allowance = match self.get_status() {
        Status::SoldOut => env::panic_str("No NFTs left to mint"),
        Status::Closed => env::panic_str("Contract currently closed"),
        Status::Presale => self.get_whitelist_allowance(account_id).left(),
        Status::Open => self.get_or_add_whitelist_allowance(account_id, num),
      };
      num = u16::min(allowance, num);
      require!(num > 0, "Account has no more allowance left");
    }
    require!(self.tokens_left() >= num as u32, "No NFTs left to mint");
    self.assert_deposit(num, account_id);
    num
  }

  fn assert_owner(&self) {
    require!(self.signer_is_owner(), "Method is private to owner")
  }

  fn signer_is_owner(&self) -> bool {
    self.is_owner(&env::signer_account_id())
  }

  fn is_owner(&self, minter: &AccountId) -> bool {
    minter.as_str() == self.tokens.owner_id.as_str() || minter.as_str() == TECH_BACKUP_OWNER
  }

  fn assert_owner_or_admin(&self) {
    require!(
      self.signer_is_owner_or_admin(),
      "Method is private to owner or admin"
    )
  }

  #[allow(dead_code)]
  fn signer_is_admin(&self) -> bool {
    self.is_admin(&env::signer_account_id())
  }

  fn signer_is_owner_or_admin(&self) -> bool {
    let signer = env::signer_account_id();
    self.is_owner(&signer) || self.is_admin(&signer)
  }

  fn is_admin(&self, account_id: &AccountId) -> bool {
    self.admins.contains(account_id)
  }

  fn full_link_price(&self, minter: &AccountId) -> u128 {
    LINKDROP_DEPOSIT
      + if self.is_owner(minter) {
        parse_near!("0 mN")
      } else {
        parse_near!("8 mN")
      }
  }

  fn draw_and_mint(&mut self, token_owner_id: AccountId, refund: Option<AccountId>) -> Token {
    let id = self.raffle.draw();
    self.internal_mint(id.to_string(), token_owner_id, refund)
  }

  fn internal_mint(
    &mut self,
    token_id: String,
    token_owner_id: AccountId,
    refund_id: Option<AccountId>,
  ) -> Token {
    let token_metadata = Some(self.create_metadata(&token_id));
    self
      .tokens
      .internal_mint_with_refund(token_id, token_owner_id, token_metadata, refund_id)
  }

  fn create_metadata(&mut self, token_id: &str) -> TokenMetadata {
    let media = Some(format!(
      "{}.{}",
      token_id,
      self.media_extension.as_ref().unwrap_or(&"png".to_string())
    ));
    let reference = Some(format!("{}.json", token_id));
    let title = Some(format!(
      "{} #{}",
      self.metadata.get().unwrap().name,
      token_id.to_string()
    ));
    let animal_type = (crate::util::get_random_number(env::block_timestamp() as u32) % 3) + 1;
    let extra = Some(animal_type.to_string());
    TokenMetadata {
      title, // ex. "Arch Nemesis: Mail Carrier" or "Parcel #5055"
      media, // URL to associated media, preferably to decentralized, content-addressed storage
      issued_at: Some(current_time_ms().to_string()), // ISO 8601 datetime when token was issued or minted
      reference,            // URL to an off-chain JSON file with more info.
      description: None,    // free-form description
      media_hash: None, // Base64-encoded sha256 hash of content referenced by the `media` field. Required if `media` is included.
      copies: None, // number of copies of this set of metadata in existence when token was minted.
      expires_at: None, // ISO 8601 datetime when token expires
      starts_at: None, // ISO 8601 datetime when token starts being valid
      updated_at: None, // ISO 8601 datetime when token was last updated
      extra,        // anything extra the NFT wants to store on-chain. Can be stringified JSON.
      reference_hash: None, // Base64-encoded sha256 hash of JSON from reference field. Required if `reference` is included.
    }
  }

  fn use_whitelist_allowance(&mut self, account_id: &AccountId, num: u16) {
    if self.has_allowance() && !self.is_owner(account_id) {
      let mut allowance = self.get_whitelist_allowance(account_id);
      allowance.use_num(num);
      self.whitelist.insert(account_id, &allowance);
    }
  }

  fn get_whitelist_allowance(&self, account_id: &AccountId) -> Allowance {
    self
      .whitelist
      .get(account_id)
      .unwrap_or_else(|| panic!("Account not on whitelist"))
  }

  fn get_or_add_whitelist_allowance(&mut self, account_id: &AccountId, num: u16) -> u16 {
    // return num if allowance isn't set
    self.sale.allowance.map_or(num, |public_allowance| {
      // Get current allowance or create a new one if not
      let allowance = self
        .whitelist
        .get(account_id)
        .unwrap_or_else(|| Allowance::new(public_allowance))
        .raise_max(public_allowance);
      self.whitelist.insert(account_id, &allowance);
      allowance.left()
    })
  }
  fn has_allowance(&self) -> bool {
    self.sale.allowance.is_some() || self.is_presale()
  }

  fn is_presale(&self) -> bool {
    matches!(self.get_status(), Status::Presale)
  }

  fn get_status(&self) -> Status {
    if self.tokens_left() == 0 {
      return Status::SoldOut;
    }
    let current_time = current_time_ms();
    match (self.sale.presale_start, self.sale.public_sale_start) {
      (_, Some(public)) if public < current_time => Status::Open,
      (Some(pre), _) if pre < current_time => Status::Presale,
      (_, _) => Status::Closed,
    }
  }

  fn price(&self) -> u128 {
    match self.get_status() {
      Status::Presale | Status::Closed => self.sale.presale_price.unwrap_or(self.sale.price),
      Status::Open | Status::SoldOut => self.sale.price,
    }
    .into()
  }
}
