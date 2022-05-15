use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use error_chain::error_chain;
error_chain! {
    errors {
        EmptyDbError
        ApiKeyNotFoundError {
            description("No api key found in env variable. Please set it to BINANCE_API_KEY")
            display("No api key found in env variable. Please set it to BINANCE_API_KEY")
        }
        IntersectingTradeSlicesError(old_id: i64, new_id: i64) {
            description("Loaded trade data intersects with old trade data")
            display("Loaded trade data intersects with old trade data; old_id: '{}', new_id: '{}'", old_id, new_id)
        }
        BadStatusCodeError(code: reqwest::StatusCode, body: String, original_request: String) {
            description("Got bad code {code}, body {body} when doing request {original_request}")
            display("Got bad code {code}, body {body} when doing request {original_request}")
        }
    }
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        JsonDecodeError(serde_json::Error);
        MissingApiKeyInEnv(std::env::VarError);
    }
}

/*
    {
        "id": 340327051,
        "price": "0.06901500",
        "qty": "0.00160000",
        "quoteQty": "0.00011042",
        "time": 1652614347356,
        "isBuyerMaker": false,
        "isBestMatch": true
    },
*/
#[derive(Serialize, Deserialize, Clone)]
pub struct HistoricalTrade {
    #[serde(rename = "id")]
    pub trade_id: i64,
    #[serde(rename = "price")]
    pub price: String,
    #[serde(rename = "qty")]
    pub quantity: String,
    #[serde(rename = "quoteQty")]
    pub quote_quantity: String,
    #[serde(rename = "time")]
    pub time_milliseconds: i64,
    #[serde(rename = "isBuyerMaker")]
    pub is_buyer_maker: bool,
    #[serde(rename = "isBestMatch")]
    pub is_best_match: bool,
}

impl HistoricalTrade {
    pub fn get_price(&self) -> f64 {
        self.price.parse().unwrap()
    }
}

pub struct Db {
    data: Vec<HistoricalTrade>, // from most recent to least recent
}

impl Db {
    pub fn get_all_data_cloned(&self) -> Vec<HistoricalTrade> {
        self.data.clone()
    }
    pub fn get_data(&self, idx: usize) -> &HistoricalTrade {
        &self.data[self.data.len() - idx - 1] // inverse, because data is stored recent-to-latest
    }
    pub fn get_min_trade_id(&self) -> i64 {
        self.data.last().unwrap().trade_id
    }
    pub fn get_max_trade_id(&self) -> i64 {
        self.data[0].trade_id
    }
    pub fn get_min_time_milliseconds(&self) -> i64 {
        self.data.last().unwrap().time_milliseconds
    }
    pub fn get_data_len(&self) -> usize {
        self.data.len()
    }
    pub fn new<P: AsRef<Path>>(filename: &P) -> Result<Db> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut deserialized: Vec<HistoricalTrade> = serde_json::from_reader(reader)?;
        if deserialized.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        deserialized.sort_by(|a, b| b.trade_id.cmp(&a.trade_id));
        Ok(Db { data: deserialized })
    }
    pub fn from(data: Vec<HistoricalTrade>) -> Result<Db> {
        if data.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        Ok(Db { data: data })
    }
    pub async fn load_more_data(&mut self, symbol: &str) -> Result<()> {
        let limit = 1000;
        let from_id = self.get_min_trade_id() - limit;
        let query = format!("https://api.binance.com/api/v3/historicalTrades?symbol={symbol}&limit={limit}&fromId={from_id}");
        let client = reqwest::Client::new();
        let api_key = env::var("BINANCE_API_KEY").chain_err(|| ErrorKind::ApiKeyNotFoundError)?;
        let res = client
            .get(query.clone())
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
        let status = res.status();
        let data = res.text().await?;
        if !status.is_success() {
            error_chain::bail!(ErrorKind::BadStatusCodeError(status, data, query));
        }
        let mut new_data: Vec<HistoricalTrade> = serde_json::from_str(&data)
            .chain_err(|| format!("Got json decoder err when decoding text: {data}"))?;
        if new_data.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        if new_data[0].trade_id >= self.get_min_trade_id() {
            return Err(ErrorKind::IntersectingTradeSlicesError(
                self.get_min_trade_id(),
                new_data[0].trade_id,
            )
            .into());
        }
        new_data.sort_by(|a, b| b.trade_id.cmp(&a.trade_id));
        self.data.extend(new_data.drain(..));
        Ok(())
    }
    pub fn save<P: AsRef<Path>>(&self, filename: &P) -> Result<()> {
        let file = File::create(filename)?;
        serde_json::to_writer(BufWriter::new(file), &self.data)?;
        Ok(())
    }
}
