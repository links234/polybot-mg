//! Comprehensive type definitions for Gamma API data structures
//! 
//! This module defines strongly-typed structs for all Gamma API endpoints,
//! replacing tuples and primitives with meaningful domain types.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json;

/// Unique identifier for a market (outcome)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct MarketId(pub u64);

impl<'de> Deserialize<'de> for MarketId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MarketIdVisitor;
        
        impl<'de> serde::de::Visitor<'de> for MarketIdVisitor {
            type Value = MarketId;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a market ID")
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(MarketId(value))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u64>()
                    .map(MarketId)
                    .map_err(serde::de::Error::custom)
            }
        }
        
        deserializer.deserialize_any(MarketIdVisitor)
    }
}

/// Unique identifier for an event (question)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct EventId(pub u64);

impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EventIdVisitor;
        
        impl<'de> serde::de::Visitor<'de> for EventIdVisitor {
            type Value = EventId;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing an event ID")
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(EventId(value))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u64>()
                    .map(EventId)
                    .map_err(serde::de::Error::custom)
            }
        }
        
        deserializer.deserialize_any(EventIdVisitor)
    }
}

/// Condition ID (blockchain identifier for the prediction condition)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConditionId(pub String);

/// CLOB token identifier for order book trading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClobTokenId(pub String);

/// User address (proxy wallet)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct UserAddress(pub String);

/// Transaction hash on blockchain
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionHash(pub String);

/// Tag identifier for market categorization
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TagId(pub u64);

impl<'de> Deserialize<'de> for TagId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TagIdVisitor;
        
        impl<'de> serde::de::Visitor<'de> for TagIdVisitor {
            type Value = TagId;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a tag ID")
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TagId(value))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u64>()
                    .map(TagId)
                    .map_err(serde::de::Error::custom)
            }
        }
        
        deserializer.deserialize_any(TagIdVisitor)
    }
}

/// Market slug (URL-friendly identifier)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketSlug(pub String);

/// Event slug (URL-friendly identifier)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventSlug(pub String);

/// Tag slug (URL-friendly identifier)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TagSlug(pub String);

/// Trade side (from taker's perspective)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

/// Market type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketType {
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "categorical")]
    Categorical,
}

/// Filter type for trade size filtering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    #[serde(rename = "CASH")]
    Cash,
    #[serde(rename = "TOKENS")]
    Tokens,
}

/// Price level in order book
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub size: Decimal,
}

/// Market pricing information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketPricing {
    pub outcome_prices: Vec<Decimal>,
    pub last_trade_price: Option<Decimal>,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub spread: Option<Decimal>,
    pub one_day_price_change: Option<Decimal>,
}

/// Market volume and liquidity metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub volume: Decimal,
    pub volume_24hr: Decimal,
    pub liquidity: Decimal,
    pub liquidity_clob: Option<Decimal>,
}

/// Market status flags
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketStatus {
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub restricted: bool,
    pub featured: bool,
    pub new: bool,
    pub enable_order_book: bool,
    pub fpmm_live: bool,
}

/// Market tag information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketTag {
    pub id: TagId,
    pub label: String,
    pub slug: TagSlug,
}

/// CLOB reward structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClobReward {
    #[serde(rename = "assetAddress")]
    pub asset_address: String,
    #[serde(rename = "conditionId")]
    pub condition_id: ConditionId,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub id: String,
    #[serde(rename = "rewardsAmount", deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub rewards_amount: Decimal,
    #[serde(rename = "rewardsDailyRate", deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub rewards_daily_rate: Decimal,
    #[serde(rename = "startDate")]
    pub start_date: String,
}

/// Event info embedded in market
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketEventInfo {
    pub id: EventId,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub archived: Option<bool>,
    #[serde(rename = "automaticallyResolved")]
    pub automatically_resolved: Option<bool>,
    #[serde(rename = "automaticallyActive")]
    pub automatically_active: Option<bool>,
    pub closed: bool,
    #[serde(rename = "closedTime", default, deserialize_with = "serde_helpers::deserialize_optional_datetime_flexible")]
    pub closed_time: Option<DateTime<Utc>>,
    #[serde(rename = "commentCount")]
    pub comment_count: Option<u32>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "creationDate")]
    pub creation_date: Option<DateTime<Utc>>,
    pub cyom: bool,
    pub deploying: bool,
    #[serde(rename = "enableNegRisk")]
    pub enable_neg_risk: bool,
    #[serde(rename = "enableOrderBook", default)]
    pub enable_order_book: Option<bool>,
    #[serde(rename = "endDate")]
    pub end_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub featured: bool,
    pub icon: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "negRisk", default)]
    pub neg_risk: Option<bool>,
    #[serde(rename = "negRiskAugmented", default)]
    pub neg_risk_augmented: Option<bool>,
    #[serde(default)]
    pub new: bool,
    #[serde(rename = "pendingDeployment")]
    pub pending_deployment: bool,
    #[serde(default)]
    pub restricted: bool,
    #[serde(rename = "showAllOutcomes")]
    pub show_all_outcomes: bool,
    #[serde(rename = "showMarketImages")]
    pub show_market_images: bool,
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    pub ticker: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume: Option<Decimal>,
    #[serde(rename = "volume1mo", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1mo: Option<Decimal>,
    #[serde(rename = "volume1wk", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1wk: Option<Decimal>,
    #[serde(rename = "volume1yr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1yr: Option<Decimal>,
    // Additional fields from newer API responses
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub competitive: Option<Decimal>,
    #[serde(rename = "deployingTimestamp")]
    pub deploying_timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "gmpChartMode")]
    pub gmp_chart_mode: Option<String>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity: Option<Decimal>,
    #[serde(rename = "liquidityClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_clob: Option<Decimal>,
    #[serde(rename = "negRiskMarketID")]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "openInterest", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub open_interest: Option<Decimal>,
    #[serde(rename = "resolutionSource")]
    pub resolution_source: Option<String>,
    #[serde(rename = "sortBy")]
    pub sort_by: Option<String>,
    #[serde(rename = "volume24hr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_24hr: Option<Decimal>,
}

/// Complete market data structure from Gamma API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GammaMarket {
    pub id: MarketId,
    #[serde(rename = "conditionId")]
    pub condition_id: ConditionId,
    pub slug: String,
    pub question: String,
    pub description: Option<String>,
    #[serde(deserialize_with = "serde_helpers::deserialize_json_string_array")]
    pub outcomes: Vec<String>,
    #[serde(rename = "outcomePrices", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_json_string_array")]
    pub outcome_prices: Option<Vec<Decimal>>,
    #[serde(rename = "clobTokenIds", default, deserialize_with = "serde_helpers::deserialize_clob_token_ids_optional")]
    pub clob_token_ids: Vec<ClobTokenId>,
    pub category: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<DateTime<Utc>>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "closedTime", default, deserialize_with = "serde_helpers::deserialize_optional_datetime_flexible")]
    pub closed_time: Option<DateTime<Utc>>,
    pub image: Option<String>,
    pub icon: Option<String>,
    #[serde(rename = "twitterCardImage")]
    pub twitter_card_image: Option<String>,
    #[serde(rename = "marketMakerAddress")]
    pub market_maker_address: Option<String>,
    #[serde(rename = "volumeNum", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_num: Option<Decimal>,
    // Alternative volume field - some markets use "volume" instead of "volumeNum"
    #[serde(rename = "volume", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string", skip_serializing_if = "Option::is_none")]
    pub volume_alt: Option<Decimal>,
    #[serde(rename = "liquidityNum", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity: Option<Decimal>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub restricted: bool,
    pub cyom: bool,
    pub approved: bool,
    #[serde(rename = "volume24hr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_24hr: Option<Decimal>,
    #[serde(rename = "volume1wk", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1wk: Option<Decimal>,
    #[serde(rename = "volume1mo", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1mo: Option<Decimal>,
    #[serde(rename = "volume1yr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1yr: Option<Decimal>,
    #[serde(rename = "bestBid", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub best_bid: Option<Decimal>,
    #[serde(rename = "bestAsk", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub best_ask: Option<Decimal>,
    #[serde(rename = "lastTradePrice", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub last_trade_price: Option<Decimal>,
    // Additional fields from API response
    #[serde(rename = "acceptingOrders", default)]
    pub accepting_orders: bool,
    #[serde(rename = "acceptingOrdersTimestamp")]
    pub accepting_orders_timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "automaticallyResolved")]
    pub automatically_resolved: Option<bool>,
    #[serde(rename = "automaticallyActive")]
    pub automatically_active: Option<bool>,
    #[serde(rename = "clearBookOnStart")]
    pub clear_book_on_start: Option<bool>,
    #[serde(rename = "clobRewards", default)]
    pub clob_rewards: Vec<ClobReward>,
    pub deploying: Option<bool>,
    #[serde(rename = "enableOrderBook", default)]
    pub enable_order_book: bool,
    #[serde(rename = "endDateIso")]
    pub end_date_iso: Option<String>,
    pub events: Option<Vec<MarketEventInfo>>,
    #[serde(default)]
    pub featured: bool,
    pub funded: Option<bool>,
    #[serde(rename = "groupItemThreshold")]
    pub group_item_threshold: Option<String>,
    #[serde(rename = "groupItemTitle")]
    pub group_item_title: Option<String>,
    #[serde(rename = "hasReviewedDates")]
    pub has_reviewed_dates: Option<bool>,
    #[serde(rename = "manualActivation")]
    pub manual_activation: Option<bool>,
    #[serde(rename = "negRisk", default)]
    pub neg_risk: bool,
    #[serde(rename = "negRiskOther")]
    pub neg_risk_other: Option<bool>,
    #[serde(default)]
    pub new: bool,
    #[serde(rename = "oneDayPriceChange", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub one_day_price_change: Option<Decimal>,
    #[serde(rename = "oneHourPriceChange", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub one_hour_price_change: Option<Decimal>,
    #[serde(rename = "oneMonthPriceChange", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub one_month_price_change: Option<Decimal>,
    #[serde(rename = "oneWeekPriceChange", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub one_week_price_change: Option<Decimal>,
    #[serde(rename = "oneYearPriceChange", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub one_year_price_change: Option<Decimal>,
    #[serde(rename = "orderMinSize", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub order_min_size: Option<Decimal>,
    #[serde(rename = "orderPriceMinTickSize", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub order_price_min_tick_size: Option<Decimal>,
    #[serde(rename = "pagerDutyNotificationEnabled")]
    pub pager_duty_notification_enabled: Option<bool>,
    #[serde(rename = "pendingDeployment")]
    pub pending_deployment: Option<bool>,
    #[serde(rename = "questionID")]
    pub question_id: Option<String>,
    pub ready: Option<bool>,
    #[serde(rename = "resolutionSource")]
    pub resolution_source: Option<String>,
    #[serde(rename = "resolvedBy")]
    pub resolved_by: Option<String>,
    #[serde(rename = "rewardsMaxSpread", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub rewards_max_spread: Option<Decimal>,
    #[serde(rename = "rewardsMinSize", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub rewards_min_size: Option<Decimal>,
    #[serde(rename = "rfqEnabled")]
    pub rfq_enabled: Option<bool>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub spread: Option<Decimal>,
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(rename = "startDateIso")]
    pub start_date_iso: Option<String>,
    #[serde(rename = "submitted_by")]
    pub submitted_by: Option<String>,
    #[serde(rename = "umaBond")]
    pub uma_bond: Option<String>,
    #[serde(rename = "umaEndDate", default, deserialize_with = "serde_helpers::deserialize_optional_datetime_flexible")]
    pub uma_end_date: Option<DateTime<Utc>>,
    #[serde(rename = "umaResolutionStatus")]
    pub uma_resolution_status: Option<String>,
    #[serde(rename = "umaResolutionStatuses")]
    pub uma_resolution_statuses: Option<String>,
    #[serde(rename = "umaReward")]
    pub uma_reward: Option<String>,
    #[serde(rename = "volume1moAmm", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1mo_amm: Option<Decimal>,
    #[serde(rename = "volume1moClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1mo_clob: Option<Decimal>,
    #[serde(rename = "volume1wkAmm", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1wk_amm: Option<Decimal>,
    #[serde(rename = "volume1wkClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1wk_clob: Option<Decimal>,
    #[serde(rename = "volume1yrAmm", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1yr_amm: Option<Decimal>,
    #[serde(rename = "volume1yrClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1yr_clob: Option<Decimal>,
    #[serde(rename = "volumeClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_clob: Option<Decimal>,
    // Additional fields from newer API responses
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub competitive: Option<Decimal>,
    #[serde(rename = "deployingTimestamp")]
    pub deploying_timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "negRiskMarketID")]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "negRiskRequestID")]
    pub neg_risk_request_id: Option<String>,
    #[serde(rename = "seriesColor")]
    pub series_color: Option<String>,
    #[serde(rename = "showGmpOutcome")]
    pub show_gmp_outcome: Option<bool>,
    #[serde(rename = "showGmpSeries")]
    pub show_gmp_series: Option<bool>,
    // Additional liquidity fields that appear as strings
    // Commented out to avoid duplicate field mapping conflicts
    // #[serde(rename = "liquidity", deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    // pub _liquidity_string: Option<Decimal>,
    // #[serde(rename = "liquidityClob", deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    // pub _liquidity_clob_alt: Option<Decimal>,
    
    // Additional fields found in API responses
    #[serde(rename = "mailchimpTag")]
    pub mailchimp_tag: Option<String>,
    #[serde(rename = "marketType")]
    pub market_type: Option<String>,
    #[serde(rename = "readyForCron")]
    pub ready_for_cron: Option<bool>,
    #[serde(rename = "updatedBy")]
    pub updated_by: Option<u32>,
    pub creator: Option<String>,
    #[serde(rename = "wideFormat")]
    pub wide_format: Option<bool>,
    #[serde(rename = "gameStartTime")]
    pub game_start_time: Option<String>,
    #[serde(rename = "secondsDelay")]
    pub seconds_delay: Option<u32>,
    #[serde(rename = "sentDiscord")]
    pub sent_discord: Option<bool>,
    #[serde(rename = "notificationsEnabled")]
    pub notifications_enabled: Option<bool>,
    pub fee: Option<String>,
    #[serde(rename = "fpmmLive")]
    pub fpmm_live: Option<bool>,
    // volume24hr is already defined above as required field
    #[serde(rename = "volume24hrClob", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_24hr_clob: Option<Decimal>,
    #[serde(rename = "volumeAmm", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_amm: Option<Decimal>,
    #[serde(rename = "liquidityAmm", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_amm: Option<Decimal>,
    #[serde(rename = "commentsEnabled")]
    pub comments_enabled: Option<bool>,
    // Optional ticker field that appears in some markets
    pub ticker: Option<String>,
}

impl GammaMarket {
    /// Get the volume value from either volumeNum or volume field
    pub fn volume(&self) -> Decimal {
        self.volume_num
            .or(self.volume_alt)
            .unwrap_or(Decimal::ZERO)
    }
}

/// Event-level data structure from Gamma API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GammaEvent {
    pub id: EventId,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(rename = "endDate")]
    pub end_date: Option<DateTime<Utc>>,
    #[serde(rename = "negRisk", default)]
    pub neg_risk: bool,
    #[serde(rename = "negRiskMarketID")]
    pub neg_risk_market_id: Option<String>,
    #[serde(rename = "enableOrderBook", default)]
    pub enable_order_book: bool,
    #[serde(rename = "createdAt")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<DateTime<Utc>>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub featured: Option<bool>,
    pub restricted: Option<bool>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume: Option<Decimal>,
    #[serde(rename = "volumeNum", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_num: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity: Option<Decimal>,
    #[serde(rename = "liquidityNum", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_num: Option<Decimal>,
    #[serde(rename = "openInterest", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub open_interest: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub competitive: Option<Decimal>,
    pub tags: Option<Vec<MarketTag>>,
    #[serde(rename = "volume1mo", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1mo: Option<Decimal>,
    #[serde(rename = "volume1wk", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1wk: Option<Decimal>,
    #[serde(rename = "volume1yr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_1yr: Option<Decimal>,
    #[serde(rename = "volume24hr", default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_24hr: Option<Decimal>,
    // Markets are not directly included in event responses
    #[serde(skip)]
    pub markets: Vec<GammaMarket>,
}

/// Trade record from Data API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GammaTrade {
    #[serde(deserialize_with = "serde_helpers::deserialize_timestamp")]
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "proxyWallet")]
    pub proxy_wallet: UserAddress,
    pub side: TradeSide,
    #[serde(rename = "conditionId")]
    pub condition_id: ConditionId,
    pub asset: ClobTokenId,
    pub outcome: String,
    #[serde(rename = "outcomeIndex")]
    pub outcome_index: u32,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub price: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub size: Decimal,
    pub title: String,
    pub slug: MarketSlug,
    #[serde(rename = "eventSlug")]
    pub event_slug: EventSlug,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: TransactionHash,
    pub name: Option<String>,
    pub pseudonym: Option<String>,
    #[serde(rename = "profileImage")]
    pub profile_image: Option<String>,
    #[serde(rename = "profileImageOptimized")]
    pub profile_image_optimized: Option<String>,
    pub icon: Option<String>,
    pub bio: Option<String>,
}

/// User position from Data API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GammaPosition {
    pub proxy_wallet: UserAddress,
    pub asset: ClobTokenId,
    pub condition_id: ConditionId,
    pub outcome: String,
    pub outcome_index: u32,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub size: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub avg_price: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub initial_value: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub current_value: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub cash_pnl: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub percent_pnl: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub total_bought: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub realized_pnl: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub percent_realized_pnl: Decimal,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub cur_price: Decimal,
    pub redeemable: bool,
    pub opposite_outcome: Option<String>,
    pub opposite_asset: Option<ClobTokenId>,
    pub end_date: DateTime<Utc>,
    pub negative_risk: bool,
    pub title: String,
    pub slug: MarketSlug,
    pub icon: Option<String>,
    pub event_slug: EventSlug,
}

/// Price history point from CLOB API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    #[serde(deserialize_with = "serde_helpers::deserialize_decimal_from_string")]
    pub price: Decimal,
}

/// Historical price data for a token
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceHistory {
    pub token_id: ClobTokenId,
    pub history: Vec<PricePoint>,
}

/// Query parameters for market fetching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order: Option<String>,
    pub ascending: Option<bool>,
    pub ids: Vec<MarketId>,
    pub slugs: Vec<MarketSlug>,
    pub archived: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub clob_token_ids: Vec<ClobTokenId>,
    pub condition_ids: Vec<ConditionId>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_min: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_max: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_min: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_max: Option<Decimal>,
    pub start_date_min: Option<DateTime<Utc>>,
    pub start_date_max: Option<DateTime<Utc>>,
    pub end_date_min: Option<DateTime<Utc>>,
    pub end_date_max: Option<DateTime<Utc>>,
    pub tag_ids: Vec<TagId>,
    pub tags: Vec<String>,
    pub tag_slugs: Vec<TagSlug>,
    pub related_tags: Option<bool>,
}

/// Query parameters for event fetching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order: Option<String>,
    pub ascending: Option<bool>,
    pub ids: Vec<EventId>,
    pub slugs: Vec<EventSlug>,
    pub archived: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_min: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub liquidity_max: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_min: Option<Decimal>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub volume_max: Option<Decimal>,
    pub start_date_min: Option<DateTime<Utc>>,
    pub start_date_max: Option<DateTime<Utc>>,
    pub end_date_min: Option<DateTime<Utc>>,
    pub end_date_max: Option<DateTime<Utc>>,
    pub tag_ids: Vec<TagId>,
    pub tags: Vec<String>,
    pub tag_slugs: Vec<TagSlug>,
    pub related_tags: Option<bool>,
}

/// Query parameters for trade fetching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeQuery {
    pub user: Option<UserAddress>,
    pub market: Option<ConditionId>,
    pub markets: Vec<ConditionId>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub taker_only: Option<bool>,
    pub filter_type: Option<FilterType>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub filter_amount: Option<Decimal>,
    pub side: Option<TradeSide>,
}

/// Query parameters for position fetching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PositionQuery {
    pub user: UserAddress,
    pub markets: Vec<ConditionId>,
    pub event_id: Option<EventId>,
    #[serde(default, deserialize_with = "serde_helpers::deserialize_optional_decimal_from_string")]
    pub size_threshold: Option<Decimal>,
    pub redeemable: Option<bool>,
    pub mergeable: Option<bool>,
    pub title: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
}

/// Query parameters for price history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistoryQuery {
    pub market: ClobTokenId,
    pub start_ts: Option<DateTime<Utc>>,
    pub end_ts: Option<DateTime<Utc>>,
    pub interval: Option<String>,
    pub fidelity: Option<u32>,
}

/// Search filters for local data querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub keyword: Option<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub min_volume: Option<Decimal>,
    pub max_volume: Option<Decimal>,
    pub min_liquidity: Option<Decimal>,
    pub max_liquidity: Option<Decimal>,
    pub active_only: bool,
    pub closed_only: bool,
    pub archived_only: bool,
    pub market_type: Option<MarketType>,
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

/// Aggregated market statistics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_markets: u64,
    pub active_markets: u64,
    pub closed_markets: u64,
    pub archived_markets: u64,
    pub total_volume: Decimal,
    pub total_liquidity: Decimal,
    pub avg_volume: Decimal,
    pub avg_liquidity: Decimal,
    pub top_categories: Vec<(String, u64)>,
    pub top_tags: Vec<(String, u64)>,
}

/// Response wrapper for paginated results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: Option<u64>,
    pub offset: u32,
    pub limit: u32,
    pub has_more: bool,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, offset: u32, limit: u32, total: Option<u64>) -> Self {
        let has_more = data.len() as u32 == limit;
        Self {
            data,
            total,
            offset,
            limit,
            has_more,
        }
    }
}

// Helper functions for deserializing JSON strings
mod serde_helpers {
    use super::*;
    use serde::{de, Deserializer};
    
    pub fn deserialize_json_string_array<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        serde_json::from_str(&s).map_err(de::Error::custom)
    }
    
    #[allow(dead_code)]
    pub fn deserialize_decimal_json_string_array<'de, D>(deserializer: D) -> Result<Vec<Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let str_array: Vec<String> = serde_json::from_str(&s).map_err(de::Error::custom)?;
        str_array.into_iter()
            .map(|s| s.parse::<Decimal>().map_err(de::Error::custom))
            .collect()
    }
    
    pub fn deserialize_clob_token_ids_optional<'de, D>(deserializer: D) -> Result<Vec<ClobTokenId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                let str_array: Vec<String> = serde_json::from_str(&s).map_err(de::Error::custom)?;
                Ok(str_array.into_iter().map(ClobTokenId).collect())
            }
            None => Ok(Vec::new()),
        }
    }
    
    pub fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimestampVisitor;
        
        impl<'de> de::Visitor<'de> for TimestampVisitor {
            type Value = DateTime<Utc>;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a timestamp as integer seconds or datetime string")
            }
            
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                DateTime::from_timestamp(value, 0)
                    .ok_or_else(|| de::Error::custom("invalid timestamp"))
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                DateTime::from_timestamp(value as i64, 0)
                    .ok_or_else(|| de::Error::custom("invalid timestamp"))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                parse_flexible_datetime(value).map_err(de::Error::custom)
            }
        }
        
        deserializer.deserialize_any(TimestampVisitor)
    }
    
    pub fn deserialize_decimal_from_string<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DecimalVisitor;
        
        impl<'de> de::Visitor<'de> for DecimalVisitor {
            type Value = Decimal;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a decimal value")
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value.parse::<Decimal>().map_err(de::Error::custom)
            }
            
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Decimal::try_from(value).map_err(de::Error::custom)
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Decimal::from(value))
            }
            
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Decimal::from(value))
            }
        }
        
        deserializer.deserialize_any(DecimalVisitor)
    }
    
    pub fn deserialize_optional_decimal_from_string<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionalDecimalVisitor;
        
        impl<'de> de::Visitor<'de> for OptionalDecimalVisitor {
            type Value = Option<Decimal>;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an optional string or number representing a decimal value")
            }
            
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }
            
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserialize_decimal_from_string(deserializer).map(Some)
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.is_empty() {
                    Ok(None)
                } else {
                    value.parse::<Decimal>().map(Some).map_err(de::Error::custom)
                }
            }
            
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Decimal::try_from(value).map(Some).map_err(de::Error::custom)
            }
        }
        
        deserializer.deserialize_option(OptionalDecimalVisitor)
    }
    
    pub fn deserialize_optional_datetime_flexible<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionalDateTimeVisitor;
        
        impl<'de> de::Visitor<'de> for OptionalDateTimeVisitor {
            type Value = Option<DateTime<Utc>>;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an optional datetime string in various formats")
            }
            
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }
            
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                parse_flexible_datetime(&s).map(Some).map_err(de::Error::custom)
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.is_empty() {
                    Ok(None)
                } else {
                    parse_flexible_datetime(value).map(Some).map_err(de::Error::custom)
                }
            }
        }
        
        deserializer.deserialize_option(OptionalDateTimeVisitor)
    }
    
    fn parse_flexible_datetime(s: &str) -> Result<DateTime<Utc>, String> {
        // Handle special cases first - these are non-datetime values used by the API
        if s == "NOW*()" || s == "NOW()" || s.is_empty() {
            // Return current time for NOW*(), NOW() and empty strings
            return Ok(chrono::Utc::now());
        }
        
        // Try standard ISO 8601 format first
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Ok(dt.with_timezone(&Utc));
        }
        
        // Try format with space instead of T and timezone variations
        // Handle formats like "2024-04-09 17:03:46+00", "2024-11-08 03:38:50.376398+00"
        let normalized = if s.ends_with("+00") {
            format!("{}:00", s)
        } else if s.ends_with("-00") {
            format!("{}:00", s)
        } else {
            s.to_string()
        };
        
        // Try with timezone offset
        if let Ok(dt) = DateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%:z") {
            return Ok(dt.with_timezone(&Utc));
        }
        
        // Try with microseconds and timezone
        if let Ok(dt) = DateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%.f%:z") {
            return Ok(dt.with_timezone(&Utc));
        }
        
        // Try original timezone format variations
        if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%z") {
            return Ok(dt.with_timezone(&Utc));
        }
        
        if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f%z") {
            return Ok(dt.with_timezone(&Utc));
        }
        
        // Try with Z suffix variations
        if s.ends_with('Z') {
            let without_z = &s[..s.len()-1];
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(without_z, "%Y-%m-%dT%H:%M:%S") {
                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(without_z, "%Y-%m-%dT%H:%M:%S%.f") {
                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(without_z, "%Y-%m-%d %H:%M:%S") {
                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(without_z, "%Y-%m-%d %H:%M:%S%.f") {
                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
            }
        }
        
        // Try without timezone (assume UTC) - various formats
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        // Try with fractional seconds (microseconds)
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        // Try shorter date-time formats without seconds (e.g., "2023-03-03T15:00")
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
        
        // Try human-readable format (e.g., "April 11, 2022")
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%B %d, %Y") {
            return Ok(DateTime::from_naive_utc_and_offset(dt.and_hms_opt(0, 0, 0).unwrap(), Utc));
        }
        
        // Try date-only formats
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Ok(DateTime::from_naive_utc_and_offset(dt.and_hms_opt(0, 0, 0).unwrap(), Utc));
        }
        
        Err(format!("Could not parse datetime from string: '{}'", s))
    }
    
    pub fn deserialize_optional_decimal_json_string_array<'de, D>(deserializer: D) -> Result<Option<Vec<Decimal>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                let str_array: Vec<String> = serde_json::from_str(&s).map_err(de::Error::custom)?;
                let decimals: Result<Vec<Decimal>, _> = str_array.into_iter()
                    .map(|s| s.parse::<Decimal>().map_err(de::Error::custom))
                    .collect();
                Ok(Some(decimals?))
            }
            None => Ok(None),
        }
    }
}